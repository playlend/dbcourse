//! Name and type resolution — **you build this in Milestone 3**.
//!
//! The binder walks an [`ast::Statement`] with the catalog in hand and
//! settles everything the parser deferred: table names become
//! [`TableInfo`]s, column names become `(table_idx, column_idx)` pairs,
//! expressions get types checked bottom-up, and statement-shape rules
//! (INSERT arity, SET column existence) are enforced. Errors are
//! [`Error::Bind`](ferricdb_common::Error::Bind) and must name the
//! offender.
//!
//! The output types here are the planner's input vocabulary; the plan
//! module and the provided tests depend on their exact shape.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use ferricdb_catalog::{Catalog, TableInfo};
use ferricdb_common::{DataType, Result, Schema, Value};
use ferricdb_sql::ast::{self, BinaryOp, JoinKind, UnaryOp};

/// An expression with every column reference resolved and typed.
///
/// `Column { table, column }` indexes into the statement's FROM list
/// (`table`) and that table's schema (`column`). During executor creation
/// (Milestone 4) these pairs are flattened to offsets in the operator's
/// input row — left join input's columns first, then the right's.
#[derive(Clone, Debug, PartialEq)]
pub enum BoundExpr {
    Literal(Value),
    Column {
        /// Index into the bound FROM list.
        table: usize,
        /// Column ordinal within that table's schema.
        column: usize,
        /// The column's declared type (carried so later phases never need
        /// the catalog to type an expression).
        data_type: DataType,
    },
    /// Positional reference into the immediate child operator's output row
    /// (Milestone 6). Used in post-`Aggregate` projections and `ORDER BY`
    /// clauses where `LogicalPlan::Aggregate` emits
    /// `[group_by_0, ..., group_by_{g-1}, agg_0, ..., agg_{a-1}]`, so
    /// referencing aggregate `k` is `InputRef { index: g + k, .. }`.
    InputRef {
        /// Flat offset in the immediate child operator's output row.
        index: usize,
        /// Inferred result type of that output column.
        data_type: DataType,
    },
    Unary(UnaryOp, Box<BoundExpr>),
    Binary(Box<BoundExpr>, BinaryOp, Box<BoundExpr>),
    IsNull(Box<BoundExpr>, bool),
    Cast(Box<BoundExpr>, DataType),
    /// Non-aggregate function call (none in the core grammar; aggregates
    /// are separated out into [`AggExpr`] during binding).
    Function(String, Vec<BoundExpr>),
}

/// Aggregate functions (Milestone 6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AggFunc {
    Count,
    Sum,
    Avg,
    Min,
    Max,
}

/// One aggregate in a `SELECT` / `GROUP BY` query: `SUM(o_total)` is
/// `AggExpr { func: Sum, arg: Some(column) }`; `COUNT(*)` has `arg: None`.
#[derive(Clone, Debug, PartialEq)]
pub struct AggExpr {
    pub func: AggFunc,
    pub arg: Option<BoundExpr>,
}

/// A bound FROM source (joins arrive in Milestone 6).
#[derive(Clone, Debug)]
pub enum BoundTableRef {
    /// A base table; its position in the flattened FROM list is the
    /// `table` index [`BoundExpr::Column`] refers to.
    Table(TableInfo),
    /// An explicit join of two sources.
    Join {
        left: Box<BoundTableRef>,
        right: Box<BoundTableRef>,
        kind: JoinKind,
        on: Option<BoundExpr>,
    },
}

/// The bound form of a SELECT.
#[derive(Clone, Debug)]
pub struct BoundSelect {
    pub from: Vec<BoundTableRef>,
    /// Output expressions with their result names (alias, column name, or
    /// a synthesized name for computed expressions).
    pub projection: Vec<(BoundExpr, String)>,
    pub filter: Option<BoundExpr>,
    /// Milestone 6: grouping keys and the aggregates over each group.
    pub group_by: Vec<BoundExpr>,
    pub aggregates: Vec<AggExpr>,
    pub order_by: Vec<(BoundExpr, bool)>,
    pub limit: Option<u64>,
    /// The output schema (names + inferred types), ready for
    /// [`crate::QueryResult::schema`].
    pub schema: Schema,
}

/// A statement with all names resolved against the catalog.
#[derive(Clone, Debug)]
pub enum BoundStatement {
    CreateTable {
        name: String,
        schema: Schema,
        /// Ordinal of the PRIMARY KEY column, if declared.
        primary_key: Option<usize>,
    },
    DropTable {
        name: String,
    },
    Insert {
        table: TableInfo,
        /// Rows re-ordered to full schema order, with `NULL` literals
        /// filled in for omitted (nullable) columns.
        rows: Vec<Vec<BoundExpr>>,
    },
    Select(Box<BoundSelect>),
    Update {
        table: TableInfo,
        /// `(column ordinal, new value expression)` pairs.
        set: Vec<(usize, BoundExpr)>,
        filter: Option<BoundExpr>,
    },
    Delete {
        table: TableInfo,
        filter: Option<BoundExpr>,
    },
    Begin,
    Commit,
    Rollback,
    CreateIndex {
        name: String,
        table: TableInfo,
        /// Ordinal of the indexed column.
        column: usize,
    },
    Analyze {
        /// `None` = analyze every table.
        table: Option<TableInfo>,
    },
    Explain {
        analyze: bool,
        inner: Box<BoundStatement>,
    },
    Vacuum,
}

/// Resolves names and types against the catalog; produces bound statements
/// whose column references are `(table index, column index)` pairs.
pub struct Binder<'a> {
    catalog: &'a Catalog,
}

impl<'a> Binder<'a> {
    pub fn new(catalog: &'a Catalog) -> Self {
        Binder { catalog }
    }

    /// Bind one parsed statement.
    ///
    /// Must reject, with an error naming the offender: unknown tables and
    /// columns; ambiguous unqualified columns (two FROM tables share the
    /// name); type errors (`age + name`, a non-boolean WHERE); INSERT
    /// arity/type mismatches; UPDATEs of unknown columns; aggregates in
    /// Milestone-3-era queries (they bind from Milestone 6 on).
    ///
    /// **Implicit numeric widening:** an `Integer` expression is implicitly
    /// widened to `Float` (e.g. via `BoundExpr::Cast(..., DataType::Float)`
    /// or numeric promotion) when inserted into a `Float` column, assigned in
    /// `UPDATE` to a `Float` column, or combined with a `Float` in arithmetic
    /// or comparisons (`o_total > 100` where `o_total` is `FLOAT`).
    pub fn bind(&self, stmt: ast::Statement) -> Result<BoundStatement> {
        todo!("Milestone 3")
    }
}
