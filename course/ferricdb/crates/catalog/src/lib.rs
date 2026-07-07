//! # ferricdb-catalog — the system catalog
//!
//! What you build here, and when:
//!
//! - **Milestone 3**: [`Catalog::bootstrap`] plus table DDL metadata —
//!   [`Catalog::create_table`], [`Catalog::drop_table`], [`Catalog::table`],
//!   [`Catalog::tables`]. The catalog is not an in-memory map that gets lost
//!   on restart: it stores *itself* in heap pages through the same
//!   [`PageStore`] the rest of the database uses. Bootstrap uses fixed page
//!   ids for the system tables (`__tables` / `__columns`), so a reopened
//!   database can find and read its own catalog before it knows about any
//!   other table.
//! - **Milestone 5**: index metadata — [`IndexInfo`],
//!   [`Catalog::create_index`], [`Catalog::set_index_root`]. The B+Tree's
//!   root page moves when the root splits, so the catalog must be able to
//!   update a stored index's root pointer.
//! - **Milestone 7**: optimizer statistics — [`ColumnStats`], [`TableStats`],
//!   [`Catalog::set_stats`], [`Catalog::stats`], written by `ANALYZE` and
//!   read by the cost model in `ferricdb-exec`.
//!
//! Everything in this crate must survive a process restart: create a table,
//! drop the [`Catalog`], reopen the store, [`Catalog::bootstrap`] again —
//! the table (schema, heap location, indexes, stats) must still be there.
//! The provided integration tests in `tests/catalog.rs` check exactly that.

// Remove this once you start implementing.
#![allow(unused_variables, dead_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ferricdb_common::{Oid, PageId, Result, Schema, Value};
use ferricdb_storage::PageStore;

/// Metadata for one secondary index (Milestone 5).
///
/// Created by [`Catalog::create_index`] and carried inside
/// [`TableInfo::indexes`]. The executor uses `root_page` to
/// `BPlusTree::open` the index; the catalog persists updates to it via
/// [`Catalog::set_index_root`] because a B+Tree's root page id changes when
/// the root node splits.
#[derive(Clone, Debug)]
pub struct IndexInfo {
    /// Catalog-assigned object id of this index, unique across tables and
    /// indexes within one database.
    pub oid: Oid,
    /// Index name as given to `CREATE INDEX` (unique per database).
    pub name: String,
    /// Name of the table this index belongs to.
    pub table: String,
    /// Name of the single indexed column (FerricDB indexes are
    /// single-column).
    pub column: String,
    /// Page id of the B+Tree root node. Updated through
    /// [`Catalog::set_index_root`] when the root splits.
    pub root_page: PageId,
}

/// Metadata for one user table (Milestone 3).
///
/// Returned by [`Catalog::table`] / [`Catalog::tables`]. Holds everything
/// the binder and executor need: the schema for name/type resolution and
/// the heap file location (`first_page`) for scans
/// (`HeapFile::open(store, first_page)`).
#[derive(Clone, Debug)]
pub struct TableInfo {
    /// Catalog-assigned object id of this table, unique across tables and
    /// indexes within one database.
    pub oid: Oid,
    /// Table name as given to `CREATE TABLE` (unique per database).
    pub name: String,
    /// Column layout of the table's tuples.
    pub schema: Schema,
    /// First page of the table's heap file chain.
    pub first_page: PageId,
    /// Indexes defined on this table (empty until Milestone 5).
    pub indexes: Vec<IndexInfo>,
}

/// Per-column statistics gathered by `ANALYZE` (Milestone 7).
///
/// One entry per column of the table's schema, in schema order, inside
/// [`TableStats::columns`]. The cost model (`ferricdb-exec::cost`) uses
/// these for selectivity estimation, e.g. `distinct_estimate` for equality
/// predicates and `min`/`max` for range predicates.
#[derive(Clone, Debug, Default)]
pub struct ColumnStats {
    /// Smallest non-NULL value observed, or `None` if the column had no
    /// non-NULL values (or was never analyzed).
    pub min: Option<Value>,
    /// Largest non-NULL value observed, or `None` if the column had no
    /// non-NULL values (or was never analyzed).
    pub max: Option<Value>,
    /// Estimated number of distinct non-NULL values.
    pub distinct_estimate: u64,
    /// Number of NULLs observed in this column.
    pub null_count: u64,
}

/// Whole-table statistics gathered by `ANALYZE` (Milestone 7).
///
/// Stored by [`Catalog::set_stats`], retrieved by [`Catalog::stats`], and
/// persisted with the rest of the catalog so a reopened database still has
/// its statistics.
#[derive(Clone, Debug, Default)]
pub struct TableStats {
    /// Number of live rows at `ANALYZE` time.
    pub row_count: u64,
    /// Number of heap pages in the table's heap file at `ANALYZE` time.
    pub page_count: u32,
    /// Per-column statistics, in schema order (one entry per column).
    pub columns: Vec<ColumnStats>,
}

/// Private in-memory cache of catalog contents.
///
/// Suggested design (you may restructure freely — this type is private):
/// [`Catalog::bootstrap`] fills it by reading the on-disk system tables;
/// mutating operations update it *and* the on-disk representation, so
/// lookups never re-read pages. The `Mutex` is what lets every `Catalog`
/// method take `&self`.
#[derive(Default)]
struct CatalogCache {
    /// Table name → metadata (including that table's indexes).
    tables: HashMap<String, TableInfo>,
    /// Table name → statistics, only for tables that have been analyzed.
    stats: HashMap<String, TableStats>,
    /// Next object id to hand out to a new table or index.
    next_oid: Oid,
}

/// The system catalog: the database's durable directory of tables, indexes,
/// and statistics (Milestone 3; indexes in 5, stats in 7).
///
/// The catalog is itself stored in heap pages on the given [`PageStore`],
/// using **fixed page ids** for its own system tables (`__tables`,
/// `__columns`) so it can find itself when the database is reopened — the
/// classic bootstrap trick. [`Catalog::bootstrap`] is create-or-open: on an
/// empty store it formats those pages; on an existing store it reads them
/// back into memory.
///
/// All methods take `&self`; interior mutability (the cached maps behind a
/// `Mutex`) keeps the catalog shareable via `Arc` between the binder,
/// planner, and executors.
pub struct Catalog {
    /// Backing page store — `DiskManager` directly in Milestone 3, the
    /// buffer pool from Milestone 4's full pipeline onward.
    store: Arc<dyn PageStore>,
    /// In-memory mirror of the on-disk catalog (see [`CatalogCache`]).
    cache: Mutex<CatalogCache>,
}

impl Catalog {
    /// Create-or-open the catalog on `store` (Milestone 3).
    ///
    /// - On an **empty** store (no pages yet): allocate and format the
    ///   system catalog pages at their fixed page ids, leaving a valid,
    ///   empty catalog.
    /// - On an **existing** store: read the system tables back and rebuild
    ///   the in-memory cache — tables, schemas, indexes (MS5), and stats
    ///   (MS7) must all reappear exactly as they were persisted.
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Io`] on page I/O failure;
    /// [`ferricdb_common::Error::Catalog`] if the stored catalog is
    /// malformed.
    pub fn bootstrap(store: Arc<dyn PageStore>) -> Result<Self> {
        todo!("Milestone 3")
    }

    /// Create a new table named `name` with the given `schema` and return
    /// its assigned [`Oid`] (Milestone 3).
    ///
    /// Allocates the table's heap file (its `first_page`), records the
    /// table and its columns in the system tables, and updates the cache.
    /// Column types and nullability must round-trip exactly through
    /// persistence.
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Catalog`] if a table named `name` already
    /// exists.
    pub fn create_table(&self, name: &str, schema: Schema) -> Result<Oid> {
        todo!("Milestone 3")
    }

    /// Remove the table named `name` from the catalog (Milestone 3).
    ///
    /// After this, [`Catalog::table`] for `name` errors and
    /// [`Catalog::tables`] no longer lists it — including after a reopen.
    /// (Reclaiming the table's heap pages is out of scope until VACUUM in
    /// Milestone 8; dropping the metadata is enough here.)
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Catalog`] if no such table exists.
    pub fn drop_table(&self, name: &str) -> Result<()> {
        todo!("Milestone 3")
    }

    /// Look up the table named `name` (Milestone 3).
    ///
    /// Returns a snapshot [`TableInfo`] (schema, heap `first_page`, and —
    /// from Milestone 5 — its indexes).
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Catalog`] if no such table exists.
    pub fn table(&self, name: &str) -> Result<TableInfo> {
        todo!("Milestone 3")
    }

    /// List all **user** tables (Milestone 3).
    ///
    /// The internal system tables (`__tables`, `__columns`, …) are an
    /// implementation detail and are *not* included; on a freshly
    /// bootstrapped empty store this returns an empty list. Order is
    /// unspecified.
    pub fn tables(&self) -> Result<Vec<TableInfo>> {
        todo!("Milestone 3")
    }

    /// Register a new single-column index and return its assigned [`Oid`]
    /// (Milestone 5).
    ///
    /// `root_page` is the root page id of the freshly created B+Tree
    /// (`BPlusTree::create` has already run; the catalog only records
    /// metadata). The index appears in [`TableInfo::indexes`] of `table`
    /// and persists across reopen.
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Catalog`] if `table` does not exist,
    /// `column` is not a column of `table`, or an index named `name`
    /// already exists.
    pub fn create_index(
        &self,
        name: &str,
        table: &str,
        column: &str,
        root_page: PageId,
    ) -> Result<Oid> {
        todo!("Milestone 5")
    }

    /// Update the recorded root page of the index with oid `index_oid`
    /// (Milestone 5).
    ///
    /// Called after a B+Tree root split changes the tree's root page id.
    /// The new root must be visible in [`TableInfo::indexes`] and must
    /// persist across reopen.
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Catalog`] if no index with that oid
    /// exists.
    pub fn set_index_root(&self, index_oid: Oid, root: PageId) -> Result<()> {
        todo!("Milestone 5")
    }

    /// Fetch the stored statistics for `table` (Milestone 7).
    ///
    /// Returns `Ok(None)` for a table that exists but has never been
    /// `ANALYZE`d — the cost model then falls back to default guesses.
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Catalog`] if no such table exists.
    pub fn stats(&self, table: &str) -> Result<Option<TableStats>> {
        todo!("Milestone 7")
    }

    /// Store (replace) the statistics for `table` (Milestone 7).
    ///
    /// Called by `ANALYZE`; the stats persist with the rest of the catalog
    /// and are returned by [`Catalog::stats`] until overwritten by the next
    /// `ANALYZE`.
    ///
    /// # Errors
    /// [`ferricdb_common::Error::Catalog`] if no such table exists.
    pub fn set_stats(&self, table: &str, stats: TableStats) -> Result<()> {
        todo!("Milestone 7")
    }
}
