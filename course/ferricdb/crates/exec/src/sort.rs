//! External merge sort — **you build this in Milestone 6**.
//!
//! `ORDER BY` must work on inputs larger than memory. The
//! [`ExternalSorter`] holds at most `memory_pages` worth of rows at a
//! time: it accumulates rows, sorts each full batch, spills it to temp
//! pages through the page store as a *sorted run* (using
//! `ferricdb_storage::tuple::serialize` with `schema`), and finally k-way
//! merges the runs (`k = max(2, memory_pages - 1)` so multi-pass merging
//! always reduces the run count even when `memory_pages` is tiny). When
//! the whole input fits in one batch, no run is spilled and this degrades
//! gracefully to the Milestone 4 in-memory sort.
//!
//! Comparison uses [`ferricdb_common::Value::sql_cmp`] per key, honoring
//! per-key direction; **NULLs order last in either direction** (course
//! convention, consistent across MS4 and MS6).

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::sync::Arc;

use ferricdb_common::{Result, Schema, Value};
use ferricdb_storage::PageStore;

/// Sorts arbitrarily large row streams within a bounded memory budget.
pub struct ExternalSorter {
    /* store, schema, memory budget, key extractors, run bookkeeping —
    your fields here */
}

impl ExternalSorter {
    /// Create a sorter over rows matching `schema`.
    ///
    /// `keys` gives, per sort key, the row offset to compare and whether
    /// the direction is descending (`true` = DESC). Temp pages for spilled
    /// runs are allocated from `store` and are throwaway (no WAL, no catalog
    /// entries). Clamp merge fan-in to at least 2 (multi-pass merge needs
    /// `memory_pages >= 3` for textbook `(B - 1)`-way merge).
    pub fn new(
        store: Arc<dyn PageStore>,
        schema: Schema,
        memory_pages: usize,
        keys: Vec<(usize, bool)>,
    ) -> Self {
        todo!("Milestone 6")
    }

    /// Feed one row. May spill a sorted run when the buffered batch
    /// reaches the memory budget.
    pub fn push(&mut self, row: Vec<Value>) -> Result<()> {
        todo!("Milestone 6")
    }

    /// Finish: sort the final batch, then k-way merge all runs. Returns
    /// the rows in order. (An iterator-returning variant is a fine
    /// refinement once the tests pass.)
    pub fn finish(self) -> Result<Vec<Vec<Value>>> {
        todo!("Milestone 6")
    }

    /// How many runs were spilled so far — the tests assert this is
    /// non-zero under a tiny budget (proving the spill path ran) and zero
    /// when everything fit.
    pub fn spilled_runs(&self) -> usize {
        todo!("Milestone 6")
    }
}
