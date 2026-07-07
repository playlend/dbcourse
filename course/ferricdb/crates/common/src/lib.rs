//! # ferricdb-common — shared foundation types
//!
//! This crate is the one crate in the FerricDB starter that is **provided
//! fully implemented**. It contains no learning content — just the shared
//! plumbing every other crate needs from Milestone 1 onward: page and
//! transaction identifier types, the SQL [`Value`] model with three-valued
//! comparison, [`Schema`]/[`Column`] descriptors, tuple metadata, and the
//! workspace-wide [`Error`]/[`Result`] types.
//!
//! You do not implement anything here. Read it once so you know what is
//! available, then move on to `ferricdb-storage` (Milestone 1).

// This crate ships fully implemented; the allow below matches the other
// starter crates for consistency and is safe to remove.
#![allow(unused_variables, dead_code)]

use std::cmp::Ordering;

/// Size of one on-disk page in bytes. Matches PostgreSQL's default block
/// size (8 KB). Every page-oriented structure in FerricDB — heap pages,
/// B+Tree nodes, catalog pages — occupies exactly this many bytes.
pub const PAGE_SIZE: usize = 8192;

/// Identifier of a page within the database file. Page N lives at byte
/// offset `N * PAGE_SIZE`.
pub type PageId = u32;

/// Sentinel meaning "no page" — used e.g. as the next-page link of the
/// last page in a heap file chain.
pub const INVALID_PAGE_ID: PageId = u32::MAX;

/// Index of a frame (in-memory page slot) inside the buffer pool
/// (Milestone 2).
pub type FrameId = usize;

/// Transaction identifier. `0` means "invalid/none" (e.g. a tuple whose
/// `xmax` is 0 has not been deleted by any transaction).
pub type TxnId = u64;

/// Log sequence number — the byte offset / ordinal position of a record
/// in the write-ahead log (Milestone 9). `0` means "no LSN yet".
pub type Lsn = u64;

/// Object identifier assigned by the catalog to tables and indexes
/// (Milestone 3), in the spirit of PostgreSQL's OIDs.
pub type Oid = u32;

/// The SQL data types FerricDB supports.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataType {
    /// SQL BOOLEAN.
    Boolean,
    /// SQL INTEGER / BIGINT — stored as `i64`.
    Integer,
    /// SQL FLOAT / DOUBLE PRECISION — stored as `f64`.
    Float,
    /// SQL TEXT / VARCHAR — stored as a UTF-8 `String`.
    Text,
}

/// A runtime SQL value: either NULL or a typed scalar.
///
/// `Value` is what executors pass between operators and what
/// `ferricdb-storage::tuple` serializes into heap tuples.
#[derive(Clone, PartialEq, Debug)]
pub enum Value {
    /// SQL NULL — the absence of a value, of no particular type.
    Null,
    /// A BOOLEAN value.
    Boolean(bool),
    /// An INTEGER value.
    Integer(i64),
    /// A FLOAT value.
    Float(f64),
    /// A TEXT value.
    Text(String),
}

impl Value {
    /// The [`DataType`] of this value, or `None` for [`Value::Null`]
    /// (NULL is untyped in this model).
    pub fn data_type(&self) -> Option<DataType> {
        match self {
            Value::Null => None,
            Value::Boolean(_) => Some(DataType::Boolean),
            Value::Integer(_) => Some(DataType::Integer),
            Value::Float(_) => Some(DataType::Float),
            Value::Text(_) => Some(DataType::Text),
        }
    }

    /// SQL comparison: returns `None` when either side is NULL
    /// (three-valued logic).
    ///
    /// Semantics:
    /// - NULL compared with anything (including NULL) is unknown → `None`.
    /// - `Integer` and `Float` cross-compare numerically (the integer is
    ///   widened to `f64`; fine for course-sized data).
    /// - `Text` compares lexicographically by Unicode code point.
    /// - `Boolean` orders `false < true`.
    /// - Comparing incompatible types (e.g. `Boolean` vs `Text`) returns
    ///   `None`; the binder (Milestone 3) rejects such comparisons before
    ///   execution, so executors treat `None` like NULL-unknown.
    /// - `Float` NaN compares as unknown (`None`), mirroring
    ///   `f64::partial_cmp`.
    pub fn sql_cmp(&self, other: &Value) -> Option<Ordering> {
        use Value::*;
        match (self, other) {
            (Null, _) | (_, Null) => None,
            (Boolean(a), Boolean(b)) => Some(a.cmp(b)),
            (Integer(a), Integer(b)) => Some(a.cmp(b)),
            (Float(a), Float(b)) => a.partial_cmp(b),
            (Integer(a), Float(b)) => (*a as f64).partial_cmp(b),
            (Float(a), Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Text(a), Text(b)) => Some(a.cmp(b)),
            _ => None, // incompatible types
        }
    }
}

/// One column of a table schema: its name, type, and nullability.
#[derive(Clone, Debug)]
pub struct Column {
    /// Column name as declared in `CREATE TABLE` (stored lowercase by the
    /// binder's conventions; `Schema::index_of` matches exactly).
    pub name: String,
    /// Declared SQL type of the column.
    pub data_type: DataType,
    /// Whether NULL is a legal value for this column.
    pub nullable: bool,
}

/// An ordered list of [`Column`]s describing the shape of a tuple —
/// a table's row layout or an executor's output row layout.
#[derive(Clone, Debug, Default)]
pub struct Schema {
    /// Columns in tuple order.
    pub columns: Vec<Column>,
}

impl Schema {
    /// Position of the column named `name` (exact match), or `None` if no
    /// such column exists. First match wins if names are duplicated.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }

    /// The column at position `i`.
    ///
    /// # Panics
    /// Panics if `i` is out of bounds — callers index with positions
    /// produced by [`Schema::index_of`] or the binder.
    pub fn column(&self, i: usize) -> &Column {
        &self.columns[i]
    }
}

/// Tuple version metadata, present in every stored tuple header from
/// Milestone 1 (written as 0 until Milestone 8 starts using it).
///
/// `xmin` is the transaction that created this tuple version; `xmax` is
/// the transaction that deleted it (0 = not deleted). These drive MVCC
/// visibility (`ferricdb-txn::Snapshot::is_visible`, Milestone 8).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TupleMeta {
    /// Id of the transaction that inserted this tuple version (0 = none).
    pub xmin: TxnId,
    /// Id of the transaction that deleted this tuple version (0 = live).
    pub xmax: TxnId,
}

/// Physical tuple address: (page, slot) — Postgres calls this the ctid.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct RecordId {
    /// Page the tuple lives on.
    pub page_id: PageId,
    /// Slot number within that page's line-pointer array.
    pub slot: u16,
}

/// The single error type shared by every FerricDB crate.
///
/// Each variant maps to one layer of the system; the rustdoc on each
/// variant names the milestone that first produces it.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Underlying file-system failure (Milestone 1: disk manager).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A tuple did not fit in the target page (Milestone 1: slotted pages).
    #[error("page is full")]
    PageFull,

    /// A page id was out of range or otherwise unusable (Milestone 1).
    #[error("invalid page id {0}")]
    InvalidPage(PageId),

    /// A record id addressed a missing or dead slot (Milestone 1).
    #[error("invalid slot {slot} on page {page_id}", page_id = .0.page_id, slot = .0.slot)]
    InvalidSlot(RecordId),

    /// Every buffer pool frame is pinned; nothing can be evicted
    /// (Milestone 2).
    #[error("buffer pool exhausted: all frames are pinned")]
    BufferPoolExhausted,

    /// SQL text could not be tokenized or parsed (Milestone 3).
    #[error("parse error: {0}")]
    Parse(String),

    /// Name/type resolution against the catalog failed (Milestone 3).
    #[error("bind error: {0}")]
    Bind(String),

    /// Catalog operation failed, e.g. duplicate or unknown table
    /// (Milestone 3).
    #[error("catalog error: {0}")]
    Catalog(String),

    /// Runtime failure inside an executor (Milestone 4).
    #[error("execution error: {0}")]
    Execution(String),

    /// First-committer-wins conflict (Milestone 8): SQLSTATE 40001 analog.
    #[error("serialization failure: {0}")]
    Serialization(String),

    /// Write-ahead log corruption or append/flush failure (Milestone 9).
    #[error("wal error: {0}")]
    Wal(String),

    /// Malformed or unexpected wire-protocol traffic (Milestone 10).
    #[error("protocol error: {0}")]
    Protocol(String),
}

/// Workspace-wide result alias over [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    // ---- Value::sql_cmp: three-valued logic ------------------------------

    #[test]
    fn sql_cmp_null_vs_anything_is_none() {
        let vals = [
            Value::Null,
            Value::Boolean(true),
            Value::Integer(7),
            Value::Float(1.5),
            Value::Text("x".into()),
        ];
        for v in &vals {
            assert_eq!(Value::Null.sql_cmp(v), None, "NULL cmp {v:?}");
            assert_eq!(v.sql_cmp(&Value::Null), None, "{v:?} cmp NULL");
        }
    }

    #[test]
    fn sql_cmp_integers() {
        assert_eq!(
            Value::Integer(1).sql_cmp(&Value::Integer(2)),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::Integer(5).sql_cmp(&Value::Integer(5)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Value::Integer(-1).sql_cmp(&Value::Integer(-2)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn sql_cmp_cross_numeric() {
        assert_eq!(
            Value::Integer(2).sql_cmp(&Value::Float(2.5)),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::Float(2.5).sql_cmp(&Value::Integer(2)),
            Some(Ordering::Greater)
        );
        assert_eq!(
            Value::Integer(3).sql_cmp(&Value::Float(3.0)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Value::Float(3.0).sql_cmp(&Value::Integer(3)),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn sql_cmp_floats_and_nan() {
        assert_eq!(
            Value::Float(1.0).sql_cmp(&Value::Float(2.0)),
            Some(Ordering::Less)
        );
        // NaN is unknown-comparable, like NULL.
        assert_eq!(Value::Float(f64::NAN).sql_cmp(&Value::Float(1.0)), None);
        assert_eq!(Value::Integer(1).sql_cmp(&Value::Float(f64::NAN)), None);
    }

    #[test]
    fn sql_cmp_text_lexicographic() {
        assert_eq!(
            Value::Text("apple".into()).sql_cmp(&Value::Text("banana".into())),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::Text("pear".into()).sql_cmp(&Value::Text("pear".into())),
            Some(Ordering::Equal)
        );
        // Lexicographic, not numeric: "10" < "9".
        assert_eq!(
            Value::Text("10".into()).sql_cmp(&Value::Text("9".into())),
            Some(Ordering::Less)
        );
    }

    #[test]
    fn sql_cmp_boolean_false_lt_true() {
        assert_eq!(
            Value::Boolean(false).sql_cmp(&Value::Boolean(true)),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::Boolean(true).sql_cmp(&Value::Boolean(true)),
            Some(Ordering::Equal)
        );
        assert_eq!(
            Value::Boolean(true).sql_cmp(&Value::Boolean(false)),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn sql_cmp_incompatible_types_is_none() {
        assert_eq!(Value::Boolean(true).sql_cmp(&Value::Integer(1)), None);
        assert_eq!(Value::Text("1".into()).sql_cmp(&Value::Integer(1)), None);
        assert_eq!(Value::Float(0.0).sql_cmp(&Value::Text("0.0".into())), None);
        assert_eq!(
            Value::Boolean(false).sql_cmp(&Value::Text("false".into())),
            None
        );
    }

    // ---- Value::data_type -------------------------------------------------

    #[test]
    fn value_data_type() {
        assert_eq!(Value::Null.data_type(), None);
        assert_eq!(Value::Boolean(true).data_type(), Some(DataType::Boolean));
        assert_eq!(Value::Integer(0).data_type(), Some(DataType::Integer));
        assert_eq!(Value::Float(0.0).data_type(), Some(DataType::Float));
        assert_eq!(Value::Text(String::new()).data_type(), Some(DataType::Text));
    }

    // ---- Schema -----------------------------------------------------------

    fn sample_schema() -> Schema {
        Schema {
            columns: vec![
                Column {
                    name: "id".into(),
                    data_type: DataType::Integer,
                    nullable: false,
                },
                Column {
                    name: "name".into(),
                    data_type: DataType::Text,
                    nullable: true,
                },
            ],
        }
    }

    #[test]
    fn schema_index_of_hit_and_miss() {
        let schema = sample_schema();
        assert_eq!(schema.index_of("id"), Some(0));
        assert_eq!(schema.index_of("name"), Some(1));
        assert_eq!(schema.index_of("missing"), None);
        // Exact match only — no case folding here.
        assert_eq!(schema.index_of("ID"), None);
    }

    #[test]
    fn schema_column_accessor() {
        let schema = sample_schema();
        assert_eq!(schema.column(1).name, "name");
        assert_eq!(schema.column(0).data_type, DataType::Integer);
    }

    // ---- Error Display ------------------------------------------------------

    #[test]
    fn error_display_messages() {
        assert_eq!(Error::PageFull.to_string(), "page is full");
        assert_eq!(Error::InvalidPage(42).to_string(), "invalid page id 42");
        assert_eq!(
            Error::InvalidSlot(RecordId {
                page_id: 3,
                slot: 9
            })
            .to_string(),
            "invalid slot 9 on page 3"
        );
        assert_eq!(
            Error::Parse("unexpected token `)`".into()).to_string(),
            "parse error: unexpected token `)`"
        );
        assert_eq!(
            Error::Serialization("write-write conflict on rid (1, 2)".into()).to_string(),
            "serialization failure: write-write conflict on rid (1, 2)"
        );
    }

    #[test]
    fn error_from_io() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        let err: Error = io.into();
        assert!(matches!(err, Error::Io(_)));
        assert_eq!(err.to_string(), "io error: gone");
    }
}
