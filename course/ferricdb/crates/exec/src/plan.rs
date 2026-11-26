//! Logical plans — the tree the planner builds and the optimizer rewrites.
//!
//! **Row-layout convention** (referenced throughout the crate): an
//! operator's output row is a flat `Vec<Value>` (wrapped in
//! [`crate::ExecRow`] with an optional `RecordId` for DML). A scan yields its
//! table's columns in schema order tagged with its `table_idx`; a join yields
//! the left input's columns followed by the right's. `create_executor`
//! (Milestone 4) resolves [`BoundExpr::Column`]'s `(table, column)` pairs
//! into flat offsets using the subtree's `table_idx` layout, and
//! [`BoundExpr::InputRef`] directly indexes the child operator's output row.

#![allow(dead_code)]

use std::ops::Bound;

use ferricdb_catalog::{IndexInfo, TableInfo};
use ferricdb_common::{Schema, Value};

use crate::binder::{AggExpr, BoundExpr};

/// One node of a logical plan tree.
///
/// Milestone 4 builds and executes `SeqScan`, `Filter`, `Projection`,
/// `Sort`, `Limit`, `Insert`, `Update`, `Delete`, and `Values`;
/// Milestone 5 adds `IndexScan`; Milestone 6 the joins and `Aggregate`.
/// The optimizer (Milestone 7) rewrites trees of these nodes and chooses
/// between `SeqScan`/`IndexScan` and among join operators by cost.
#[derive(Clone, Debug)]
pub enum LogicalPlan {
    /// Full-table scan, with an optional fused filter (predicate pushdown
    /// lands here).
    SeqScan {
        /// Ordinal in the bound `FROM` list (`0` for single-table queries);
        /// lets self-joins, predicate pushdown, and join reordering know
        /// which `BoundExpr::Column { table, .. }` belongs to this scan.
        table_idx: usize,
        table: TableInfo,
        filter: Option<BoundExpr>,
    },
    /// B+Tree range/point scan (Milestone 5).
    IndexScan {
        /// Ordinal in the bound `FROM` list (`0` for single-table queries).
        table_idx: usize,
        table: TableInfo,
        index: IndexInfo,
        lower: Bound<Value>,
        upper: Bound<Value>,
    },
    Filter {
        input: Box<LogicalPlan>,
        predicate: BoundExpr,
    },
    Projection {
        input: Box<LogicalPlan>,
        exprs: Vec<BoundExpr>,
        /// Output column names (aliases preserved) and types, carried from
        /// `BoundSelect::schema` so `Executor::schema()` stays exact.
        schema: Schema,
    },
    /// Milestone 6. Works for any predicate; the fallback join.
    NestedLoopJoin {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        on: Option<BoundExpr>,
    },
    /// Milestone 6. Equi-joins only: build hash table on the **left** input
    /// using `left_keys`, probe with the **right** input using `right_keys`.
    /// NULL keys never match (SQL equality).
    HashJoin {
        left: Box<LogicalPlan>,
        right: Box<LogicalPlan>,
        left_keys: Vec<BoundExpr>,
        right_keys: Vec<BoundExpr>,
    },
    /// Milestone 6. Hash aggregation; output rows are `[group_by...,
    /// aggregates...]`. Ancestor `Projection`/`Sort` nodes reference these
    /// columns via [`BoundExpr::InputRef`].
    Aggregate {
        input: Box<LogicalPlan>,
        group_by: Vec<BoundExpr>,
        aggregates: Vec<AggExpr>,
    },
    /// Blocking sort. In-memory in Milestone 4; spills through
    /// [`crate::sort::ExternalSorter`] when the input exceeds the memory
    /// budget from Milestone 6 on. `true` = descending; **NULLs order last
    /// in either direction** (course convention).
    Sort {
        input: Box<LogicalPlan>,
        keys: Vec<(BoundExpr, bool)>,
    },
    Limit {
        input: Box<LogicalPlan>,
        limit: u64,
    },
    Insert {
        table: TableInfo,
        input: Box<LogicalPlan>,
    },
    Update {
        table: TableInfo,
        /// `(column ordinal, new value)` pairs.
        set: Vec<(usize, BoundExpr)>,
        input: Box<LogicalPlan>,
    },
    Delete {
        table: TableInfo,
        input: Box<LogicalPlan>,
    },
    /// Literal rows (INSERT's usual child; also table-less SELECTs).
    Values {
        rows: Vec<Vec<BoundExpr>>,
        schema: Schema,
    },
}
