//! # ferricdb-exec — the FerricDB query engine
//!
//! This crate is where SQL text turns into rows. You build it across four
//! milestones:
//!
//! | Milestone | What you implement here |
//! |-----------|-------------------------|
//! | 3 | [`binder`] — resolve table/column names and types against the catalog |
//! | 4 | [`Planner::plan`], [`Planner::create_executor`], [`execute`] — sequential scans, filters, projections, in-memory sort + limit, INSERT/UPDATE/DELETE |
//! | 5 | index scans become plannable ([`plan::LogicalPlan::IndexScan`]) |
//! | 6 | joins, GROUP BY aggregates, and external merge sort ([`sort`]) |
//! | 7 | [`Planner::optimize`] and [`cost`] — rewrite rules plus cost-based access-path and join-order choice |
//!
//! Every statement takes the same pipeline (Postgres in miniature):
//!
//! ```text
//! SQL text ── Parser ──▶ ast::Statement
//!          ── Binder ──▶ binder::BoundStatement      (names → (table_idx, col_idx))
//!          ── Planner::plan ──▶ plan::LogicalPlan    (SELECT/INSERT/UPDATE/DELETE)
//!          ── Planner::optimize ──▶ plan::LogicalPlan (rewritten, costed — MS7)
//!          ── Planner::create_executor ──▶ Box<dyn Executor>  (Volcano iterators)
//!          ── open()/next()/close() loop ──▶ QueryResult
//! ```
//!
//! DDL, TCL, and utility statements (`CREATE TABLE`, `DROP TABLE`,
//! `CREATE INDEX`, `BEGIN`, `COMMIT`, `ROLLBACK`, `ANALYZE`, `EXPLAIN`,
//! `VACUUM`) are dispatched directly by [`execute`] without going through
//! [`Planner::plan`].
//!
//! [`execute`] is the one-stop entry point used by the REPL, the provided
//! tests, and the server crate.

// Starter scaffolding: remove this once you start implementing.
#![allow(unused_variables, dead_code)]

pub mod binder;
pub mod cost;
pub mod plan;
pub mod sort;

use std::sync::{Arc, Mutex};

use ferricdb_catalog::Catalog;
use ferricdb_common::{Lsn, PageId, RecordId, Result, Schema, TupleMeta, TxnId, Value};
use ferricdb_storage::PageStore;
use ferricdb_txn::{Transaction, TransactionManager};

/// One row flowing between Volcano operators (modeled on BusTub's
/// `(Tuple, RID)` iterator output).
///
/// Base scans (`SeqScan`, `IndexScan`) attach `rid: Some(rid)`, and `Filter`
/// forwards it unchanged so `Update` and `Delete` can compose over any
/// child `Box<dyn Executor>` without smuggling `RecordId`s inside `Value`s.
/// Operators that synthesize rows (`Projection`, `Join`, `Aggregate`,
/// `Values`) set `rid: None`.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecRow {
    pub values: Vec<Value>,
    pub rid: Option<RecordId>,
}

impl ExecRow {
    pub fn new(values: Vec<Value>) -> Self {
        ExecRow { values, rid: None }
    }

    pub fn with_rid(values: Vec<Value>, rid: RecordId) -> Self {
        ExecRow {
            values,
            rid: Some(rid),
        }
    }
}

/// Volcano (iterator-model) executor contract. Every physical operator —
/// scan, filter, projection, join, aggregate, sort, limit, and the DML
/// operators — implements this trait, and operators compose by owning
/// their children as `Box<dyn Executor>`.
///
/// Protocol:
/// 1. [`Executor::open`] is called exactly once before the first `next`;
///    it acquires resources and opens the children.
/// 2. [`Executor::next`] returns `Ok(Some(row))` for each output row and
///    `Ok(None)` at end of stream. After `Ok(None)` it keeps returning
///    `Ok(None)`. DML operators (`Insert`, `Update`, `Delete`) perform their
///    writes and emit a single one-column row `[Value::Integer(affected)]`
///    (then `Ok(None)`), which [`execute`] extracts into
///    [`QueryResult::affected`].
/// 3. [`Executor::close`] releases resources (also closes the children).
///
/// Implemented in Milestone 4 (basic operators), extended in Milestones
/// 5–8 (index scans, joins/aggregates/external sort, and MVCC-visible
/// scans respectively).
pub trait Executor {
    /// Shape of the rows this operator emits. Valid to call at any time,
    /// including before `open`.
    fn schema(&self) -> &Schema;

    /// Prepare for iteration (open children, build hash tables, start
    /// sorted runs, ...).
    fn open(&mut self) -> Result<()>;

    /// Produce the next output row (plus optional `RecordId`), or `Ok(None)`
    /// at end of stream.
    fn next(&mut self) -> Result<Option<ExecRow>>;

    /// Release everything acquired in `open`.
    fn close(&mut self) -> Result<()>;
}

/// Hook through which the execution layer emits write-ahead-log records
/// (Milestone 9).
///
/// `ferricdb-exec` sits *below* `ferricdb-wal` in the crate graph, so it
/// cannot name `WalManager` directly. Instead the `ferricdb` binary wraps
/// the `WalManager` in a small adapter (a newtype — required by the orphan
/// rule, since neither crate depends on the other) that implements this
/// trait, and installs it in [`ExecContext::wal`].
///
/// Until Milestone 9, [`ExecContext::wal`] is `None` and executors skip
/// logging entirely.
pub trait WalHook: Send + Sync {
    /// Log formatting a newly allocated heap page so crash recovery can
    /// re-initialize it before replaying tuple inserts.
    fn log_page_init(&self, page_id: PageId) -> Result<Lsn>;
    /// Log the start of transaction `txn`. Returns the record's LSN.
    fn log_begin(&self, txn: TxnId) -> Result<Lsn>;
    /// Log the commit of `txn`. The caller must `flush_until` the returned
    /// LSN before reporting success to the client (the durability point).
    fn log_commit(&self, txn: TxnId) -> Result<Lsn>;
    /// Log the abort of `txn` (transaction-end marker written once rollback finishes).
    fn log_abort(&self, txn: TxnId) -> Result<Lsn>;
    /// Roll back `txn` by walking its `prev_lsn` chain backward, applying
    /// inverse page mutations (`HeapPage::delete` / `HeapPage::put_at`) via
    /// `store`, appending a `Clr` per undone mutation (stamping `pageLSN`),
    /// and finishing with `LogRecord::Abort`. Reuses ARIES Pass 3 undo for
    /// live `ROLLBACK`.
    fn rollback(&self, txn: TxnId, store: &dyn PageStore) -> Result<()>;
    /// Log a tuple insertion (after-image only).
    fn log_insert(&self, txn: TxnId, rid: RecordId, meta: TupleMeta, tuple: &[u8]) -> Result<Lsn>;
    /// Log a tuple update (before- and after-image, so it can be both
    /// redone and undone).
    fn log_update(
        &self,
        txn: TxnId,
        rid: RecordId,
        old_meta: TupleMeta,
        old: &[u8],
        new_meta: TupleMeta,
        new: &[u8],
    ) -> Result<Lsn>;
    /// Log a tuple deletion (before-image, so it can be undone).
    fn log_delete(&self, txn: TxnId, rid: RecordId, old_meta: TupleMeta, old: &[u8])
        -> Result<Lsn>;
    /// Block until every log record with LSN ≤ `lsn` is durable on disk.
    fn flush_until(&self, lsn: Lsn) -> Result<()>;
}

/// Default sort/hash memory budget for a fresh [`ExecContext`], in buffer
/// pages (64 × 8 KB = 512 KB). Provided tests shrink
/// [`ExecContext::memory_pages`] to force external sorting.
pub const DEFAULT_MEMORY_PAGES: usize = 64;

/// Everything an executor needs to run a statement: the shared services
/// of one open database plus per-session state. The REPL, tests, and
/// server each build one of these and pass it to [`execute`].
///
/// Fields are public so the binary crate and tests can wire one up
/// directly; [`ExecContext::new`] fills in defaults, and
/// [`ExecContext::new_session`] creates an isolated per-connection session
/// for `ferricdb-server` (sharing the database `Arc`s with a fresh
/// `current_txn` and `failed_txn`).
pub struct ExecContext {
    /// The system catalog: table/index definitions and (from Milestone 7)
    /// per-table statistics.
    pub catalog: Arc<Catalog>,
    /// Page access for heap files, B+Tree nodes, and temp pages —
    /// a `DiskManager` in Milestone 1 terms, the `BufferPoolManager`
    /// from Milestone 2 on.
    pub store: Arc<dyn PageStore>,
    /// Assigns transaction ids and snapshots, tracks commit state, and
    /// arbitrates first-committer-wins conflicts (Milestone 8).
    pub txn_manager: Arc<TransactionManager>,
    /// The session's currently open explicit transaction, if any.
    /// `BEGIN` fills it, `COMMIT`/`ROLLBACK` clear it; statements outside
    /// an explicit transaction run in their own single-statement
    /// (autocommit) transaction. Before Milestone 8 this stays `None`.
    pub current_txn: Mutex<Option<Arc<Transaction>>>,
    /// Set to `true` when a statement fails inside an explicit transaction
    /// (PostgreSQL `ReadyForQuery('E')` semantics: further commands are
    /// rejected until `ROLLBACK` clears both `current_txn` and `failed_txn`).
    pub failed_txn: Mutex<bool>,
    /// Write-ahead-log hook (Milestone 9). `None` until the binary wires
    /// in the WAL; executors must skip logging when absent.
    pub wal: Option<Arc<dyn WalHook>>,
    /// Memory budget, in 8 KB buffer pages, for memory-hungry operators
    /// (sorting and hash tables, Milestone 6). When a sort's input
    /// outgrows this budget it must spill sorted runs to temp pages via
    /// [`sort::ExternalSorter`] instead of buffering everything.
    pub memory_pages: usize,
}

impl ExecContext {
    /// Build a context over an open database with no WAL and the default
    /// memory budget ([`DEFAULT_MEMORY_PAGES`]). Provided fully
    /// implemented — it is plain wiring, not learning content.
    pub fn new(
        catalog: Arc<Catalog>,
        store: Arc<dyn PageStore>,
        txn_manager: Arc<TransactionManager>,
    ) -> Self {
        ExecContext {
            catalog,
            store,
            txn_manager,
            current_txn: Mutex::new(None),
            failed_txn: Mutex::new(false),
            wal: None,
            memory_pages: DEFAULT_MEMORY_PAGES,
        }
    }

    /// Create an independent per-connection session sharing this context's
    /// database services (`catalog`, `store`, `txn_manager`, `wal`,
    /// `memory_pages`) with its own clean `current_txn` and `failed_txn`
    /// slots. Call this once per accepted TCP connection in Milestone 10.
    pub fn new_session(&self) -> Self {
        ExecContext {
            catalog: Arc::clone(&self.catalog),
            store: Arc::clone(&self.store),
            txn_manager: Arc::clone(&self.txn_manager),
            current_txn: Mutex::new(None),
            failed_txn: Mutex::new(false),
            wal: self.wal.as_ref().map(Arc::clone),
            memory_pages: self.memory_pages,
        }
    }
}

/// Turns bound statements into logical plans, improves the plans, and
/// lowers them to executor trees.
pub struct Planner<'a> {
    /// The context whose catalog/store/txn services the produced
    /// executors will run against.
    ctx: &'a ExecContext,
}

impl<'a> Planner<'a> {
    /// Create a planner for one statement. Provided fully implemented —
    /// plain wiring.
    pub fn new(ctx: &'a ExecContext) -> Self {
        Planner { ctx }
    }

    /// Translate a plannable bound statement (`Select`, `Insert`, `Update`,
    /// `Delete`) into a [`plan::LogicalPlan`] tree (Milestone 4; joins and
    /// aggregates extend it in Milestone 6).
    ///
    /// DDL, TCL, and utility statements (`CreateTable`, `DropTable`,
    /// `CreateIndex`, `Begin`, `Commit`, `Rollback`, `Analyze`, `Explain`,
    /// `Vacuum`) are dispatched directly by [`execute`] before calling
    /// `plan()`.
    ///
    /// Typical shapes: `SELECT` becomes
    /// `Projection(Limit?(Sort?(Aggregate?(Filter?(scan/join ...)))))` —
    /// built inside-out; `INSERT` becomes `Insert { input: Values }`;
    /// `UPDATE` and `DELETE` wrap a filtered scan of the target table.
    pub fn plan(&self, stmt: binder::BoundStatement) -> Result<plan::LogicalPlan> {
        todo!("Milestone 4")
    }

    /// Improve a plan. In Milestone 4, implement this as the identity
    /// function (return the plan unchanged). Milestone 7 turns it into a
    /// real optimizer: rewrite rules first (constant folding, predicate
    /// pushdown, redundant-projection cleanup), then cost-based decisions using
    /// [`cost::CostModel`] — access path (SeqScan vs IndexScan) and join
    /// strategy/order.
    ///
    /// Contracts the provided tests rely on (Milestone 7):
    /// - Constant subexpressions fold to [`binder::BoundExpr::Literal`]
    ///   (NULL operands propagate to a NULL literal), and folding
    ///   preserves the surrounding node — a `Projection` stays a
    ///   `Projection`.
    /// - A `Filter` whose predicate touches only one side of a join is
    ///   pushed below the join: it ends up either fused into that side's
    ///   `SeqScan::filter` or as a `Filter` directly above that scan.
    pub fn optimize(&self, plan: plan::LogicalPlan) -> Result<plan::LogicalPlan> {
        todo!("Milestone 4")
    }

    /// Lower a logical plan to a tree of physical operators (Milestone 4;
    /// new operators arrive with Milestones 5–6).
    ///
    /// This is also where `(table_idx, col_idx)` column references get
    /// resolved to flat offsets in each operator's input rows — see the
    /// row-layout convention in the [`plan`] module docs.
    pub fn create_executor(&self, plan: &plan::LogicalPlan) -> Result<Box<dyn Executor>> {
        todo!("Milestone 4")
    }
}

/// The result of one executed statement.
#[derive(Clone, Debug, Default)]
pub struct QueryResult {
    /// Names and types of the output columns. For queries, the projection
    /// list (aliases win; computed expressions get a synthesized name).
    /// For DML/DDL the schema is empty.
    pub schema: Schema,
    /// The materialized output rows. Empty for DML/DDL and for `EXPLAIN`.
    pub rows: Vec<Vec<Value>>,
    /// Rows inserted/updated/deleted, for DML statements; 0 otherwise.
    pub affected: u64,
    /// PostgreSQL `CommandComplete` tag (e.g. `"SELECT 3"`, `"INSERT 0 2"`,
    /// `"UPDATE 1"`, `"DELETE 2"`, `"CREATE TABLE"`, `"BEGIN"`, `"COMMIT"`,
    /// `"ROLLBACK"`, `"ANALYZE"`, `"VACUUM"`). Used directly by
    /// `ferricdb-server` in Milestone 10.
    pub tag: String,
    /// Rendered plan for `EXPLAIN` (and `EXPLAIN ANALYZE`) statements,
    /// `None` otherwise. Format contract the provided tests grep for:
    /// one plan node per line, indented by depth, each line starting
    /// (after indentation) with the [`plan::LogicalPlan`] variant name
    /// verbatim — e.g. `SeqScan`, `IndexScan`, `HashJoin`.
    pub explain: Option<String>,
}

/// Parse, bind, plan, optimize, and run every statement in `sql`,
/// returning one [`QueryResult`] per top-level statement (statements are
/// separated by `;`). This is the one-stop entry used by the REPL, the
/// provided tests, and the server crate. Implemented in Milestone 4.
///
/// Transaction semantics:
/// - Outside an explicit transaction, each statement runs in its own
///   autocommit transaction (before Milestone 8 there is effectively a
///   single implicit transaction and `TupleMeta` stays zeroed).
/// - `BEGIN` stores a new transaction in [`ExecContext::current_txn`];
///   `COMMIT`/`ROLLBACK` end it via the [`TransactionManager`]. A
///   serialization failure surfaces as `Err(Error::Serialization)`
///   (Milestone 8).
/// - `EXPLAIN <stmt>` does not run `<stmt>`; it returns the rendered plan
///   in [`QueryResult::explain`]. `EXPLAIN ANALYZE` runs it and may
///   append runtime counters (Milestone 7).
pub fn execute(sql: &str, ctx: &ExecContext) -> Result<Vec<QueryResult>> {
    todo!("Milestone 4")
}
