//! The cost model — **you build this in Milestone 7**.
//!
//! Costs are dimensionless, measured relative to one sequential page read,
//! exactly like PostgreSQL's (whose default constants these are). The
//! selectivity estimators follow Selinger et al. 1979 where statistics
//! exist and fall back to the paper's honest guesses where they don't.
//!
//! Documented assumptions (state them in your rustdoc as you implement):
//! values are uniformly distributed over the distinct count, and ANDed
//! predicates are independent — both fail on skewed/correlated data, which
//! is what `EXPLAIN ANALYZE`'s estimated-vs-actual gap is for.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use ferricdb_catalog::{Catalog, TableInfo};
use ferricdb_common::Result;

use crate::binder::BoundExpr;
use crate::plan::LogicalPlan;

/// Cost of reading one page sequentially (the unit).
pub const SEQ_PAGE_COST: f64 = 1.0;
/// Cost of reading one page non-sequentially (Postgres default: 4×).
pub const RANDOM_PAGE_COST: f64 = 4.0;
/// CPU cost of processing one tuple.
pub const CPU_TUPLE_COST: f64 = 0.01;

/// Prices plans against catalog statistics (gathered by `ANALYZE`,
/// Milestone 7; [`Catalog::stats`] returns `None` for never-analyzed
/// tables — fall back to pessimistic defaults and document them).
pub struct CostModel<'a> {
    catalog: &'a Catalog,
}

impl<'a> CostModel<'a> {
    pub fn new(catalog: &'a Catalog) -> Self {
        CostModel { catalog }
    }

    /// Estimated total cost of `plan`, recursively.
    ///
    /// Reference shapes: `SeqScan` = `SEQ_PAGE_COST · pages +
    /// CPU_TUPLE_COST · rows`; `IndexScan` = `RANDOM_PAGE_COST · (height +
    /// matching leaf pages + matching heap fetches) + cpu` (state your
    /// clustering assumption); joins per the Module 7 formulas over
    /// estimated input cardinalities.
    pub fn plan_cost(&self, plan: &LogicalPlan) -> f64 {
        todo!("Milestone 7")
    }

    /// Estimated fraction of `table`'s rows satisfying `pred`, clamped to
    /// `[0, 1]`.
    ///
    /// Selinger defaults: equality = `1 / distinct` (1/10 without stats);
    /// range = linear interpolation between the column's min and max (1/3
    /// when not interpolatable); `AND` multiplies, `OR` adds minus the
    /// product, `NOT` complements. Guard the degenerate cases: empty
    /// tables, `min == max`.
    pub fn selectivity(&self, table: &TableInfo, pred: &BoundExpr) -> f64 {
        todo!("Milestone 7")
    }

    fn _uses(&self) -> Result<()> {
        Ok(())
    }
}
