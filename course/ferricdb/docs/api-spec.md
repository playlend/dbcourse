# FerricDB — Public API Specification (single source of truth)

Milestone pages quote these signatures; the starter workspace stubs them.
If a milestone page and this file disagree, this file wins — fix the page.

Workspace: `course/ferricdb/` (cargo workspace, Rust 2021, rust-version 1.80).
Crate dirs under `crates/`; package names carry a `ferricdb-` prefix
(`crates/storage` = package `ferricdb-storage`). The binary crate is
`crates/ferricdb` = package `ferricdb`.

Dependency policy (from the course plan): implement core algorithms by
hand. Allowed external crates: `bytes`, `thiserror`, `tempfile` (binary temp
DBs + dev tests), `criterion` (dev, benches), `rustyline` (REPL), `tokio`
(server crate + binary `--serve` entrypoint), `sqllogictest` (dev, harness
only), `rand` (dev, property/fuzz tests only). `sqlparser` is permitted
solely as the Milestone 3 escape hatch and is NOT a default dependency.

Crate dependency graph (no cycles; `common` is the shared foundation —
`catalog` and `index` depend only on `common` + `storage` via `PageStore`):

```
common ← storage ← buffer ← wal
   ↑        ↑        ↑
   ├── sql  ├── catalog
   ├── txn  ├── index
   │        └──────────┐
   └──── exec ← {common, storage, index, sql, catalog, txn}
          ↑
   server ← {common, exec, catalog, txn}
   ferricdb(bin) ← everything
```

---

## crates/common — `ferricdb-common` (provided fully implemented)

```rust
pub const PAGE_SIZE: usize = 8192;          // matches PostgreSQL default block size
pub type PageId = u32;
pub const INVALID_PAGE_ID: PageId = u32::MAX;
pub type FrameId = usize;
pub type TxnId = u64;                        // 0 = frozen / none
pub type Lsn = u64;                          // 0 = no LSN yet; real WAL records >= 1
pub type Oid = u32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum DataType { Boolean, Integer, Float, Text }

#[derive(Clone, PartialEq, Debug)]
pub enum Value { Null, Boolean(bool), Integer(i64), Float(f64), Text(String) }
impl Value {
    pub fn data_type(&self) -> Option<DataType>;   // None for Null
    /// SQL comparison: returns None when either side is NULL (three-valued logic).
    pub fn sql_cmp(&self, other: &Value) -> Option<std::cmp::Ordering>;
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Column { pub name: String, pub data_type: DataType, pub nullable: bool }
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Schema { pub columns: Vec<Column> }
impl Schema {
    pub fn index_of(&self, name: &str) -> Option<usize>;
    pub fn column(&self, i: usize) -> &Column;
}

/// Tuple version metadata, present in every stored tuple header from
/// Milestone 1 (written as 0 until Milestone 8; xmin == 0 = frozen/bootstrap).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TupleMeta { pub xmin: TxnId, pub xmax: TxnId }

/// Physical tuple address: (page, slot) — Postgres calls this the ctid.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RecordId { pub page_id: PageId, pub slot: u16 }

#[derive(thiserror::Error, Debug)]
pub enum Error {
    Io(#[from] std::io::Error),
    PageFull, InvalidPage(PageId), InvalidSlot(RecordId),
    BufferPoolExhausted,
    Parse(String), Bind(String), Catalog(String), Execution(String),
    /// First-committer-wins conflict (Milestone 8): SQLSTATE 40001 analog.
    Serialization(String),
    Wal(String), Protocol(String),
}
pub type Result<T> = std::result::Result<T, Error>;
```

## crates/storage — `ferricdb-storage` (Milestone 1; MS8 compact; MS9 put_at)

```rust
pub const PAGE_TYPE_UNINIT: u32 = 0;
pub const PAGE_TYPE_HEAP: u32 = 1;
pub const PAGE_TYPE_BTREE_LEAF: u32 = 2;
pub const PAGE_TYPE_BTREE_INTERNAL: u32 = 3;

/// One 8 KB page in memory. First 16 bytes of every page are reserved:
/// pageLSN (u64) + page type tag (u32) + reserved (u32).
pub struct Page { /* [u8; PAGE_SIZE] */ }
impl Page {
    pub fn new() -> Self;
    pub fn lsn(&self) -> Lsn;
    pub fn set_lsn(&mut self, lsn: Lsn);
    pub fn page_type(&self) -> u32;
    pub fn set_page_type(&mut self, tag: u32);
    pub fn data(&self) -> &[u8; PAGE_SIZE];
    pub fn data_mut(&mut self) -> &mut [u8; PAGE_SIZE];
}

/// Read-only slotted-page view over `&Page` (use inside `PageStore::with_page`
/// so reads never dirty buffer-pool frames).
pub struct HeapPageRef<'a> { /* &'a Page */ }
impl<'a> HeapPageRef<'a> {
    pub fn is_initialized(page: &Page) -> bool;
    pub fn attach(page: &'a Page) -> Self;
    pub fn get(&self, slot: u16) -> Option<(TupleMeta, &[u8])>;
    pub fn slot_count(&self) -> u16;
    pub fn free_space(&self) -> usize;
    pub fn next_page(&self) -> PageId;
}

/// Mutable slotted-page view over `&mut Page`: 28-byte header (16B reserved +
/// 4B next_page + 2B lower + 2B upper + 2B slot_count + 2B flags),
/// line-pointer array growing forward, tuple data growing backward.
pub struct HeapPage<'a> { /* &'a mut Page */ }
impl<'a> HeapPage<'a> {
    pub fn init(page: &'a mut Page) -> Self;       // format an empty heap page
    pub fn attach(page: &'a mut Page) -> Self;     // view an already-formatted page
    pub fn as_ref(&self) -> HeapPageRef<'_>;
    pub fn insert(&mut self, meta: TupleMeta, tuple: &[u8]) -> Option<u16>; // slot
    pub fn get(&self, slot: u16) -> Option<(TupleMeta, &[u8])>;             // None if dead/absent
    pub fn set_meta(&mut self, slot: u16, meta: TupleMeta) -> Result<()>;   // MVCC updates xmin/xmax
    pub fn delete(&mut self, slot: u16) -> Result<()>;                      // mark dead (tombstone)
    pub fn slot_count(&self) -> u16;
    pub fn free_space(&self) -> usize;
    pub fn next_page(&self) -> PageId;
    pub fn set_next_page(&mut self, next: PageId);
    pub fn compact(&mut self) -> usize;                                     // MS8 VACUUM
    pub fn put_at(&mut self, slot: u16, meta: TupleMeta, tuple: &[u8]) -> Result<()>; // MS9 ARIES
}

pub mod tuple {
    /// Format: null bitmap (bit 1 = NULL), then fixed-width values in schema order;
    /// TEXT as u32 length prefix + UTF-8 bytes.
    pub fn serialize(schema: &Schema, values: &[Value]) -> Result<Vec<u8>>;
    pub fn deserialize(schema: &Schema, bytes: &[u8]) -> Result<Vec<Value>>;
}

/// Page-granular file I/O. One file holds all pages of one database.
pub struct DiskManager { /* file handle, page counter */ }
impl DiskManager {
    pub fn open(path: &std::path::Path) -> Result<Self>;
    pub fn read_page(&self, id: PageId, out: &mut Page) -> Result<()>;
    pub fn write_page(&self, id: PageId, page: &Page) -> Result<()>;
    pub fn allocate_page(&self) -> Result<PageId>;
    pub fn num_pages(&self) -> u32;
}

/// How higher layers reach pages. DiskManager implements it directly
/// (Milestone 1); BufferPoolManager supersedes it (Milestone 2).
pub trait PageStore: Send + Sync {
    /// Closures return nothing so the trait stays object-safe
    /// (the system holds Arc<dyn PageStore>); capture results through
    /// the closure environment.
    fn with_page(&self, id: PageId, f: &mut dyn FnMut(&Page)) -> Result<()>;
    fn with_page_mut(&self, id: PageId, f: &mut dyn FnMut(&mut Page)) -> Result<()>;
    fn allocate_page(&self) -> Result<PageId>;
    fn num_pages(&self) -> u32;
    fn flush_all(&self) -> Result<()>;
}

/// A linked/sequential collection of heap pages storing one table.
pub struct HeapFile { /* Arc<dyn PageStore>, first_page */ }
impl HeapFile {
    pub fn create(store: std::sync::Arc<dyn PageStore>) -> Result<Self>;
    pub fn open(store: std::sync::Arc<dyn PageStore>, first_page: PageId) -> Self;
    pub fn first_page(&self) -> PageId;
    pub fn insert(&self, meta: TupleMeta, tuple: &[u8]) -> Result<RecordId>;
    pub fn get(&self, rid: RecordId) -> Result<Option<(TupleMeta, Vec<u8>)>>;
    pub fn set_meta(&self, rid: RecordId, meta: TupleMeta) -> Result<()>;
    pub fn delete(&self, rid: RecordId) -> Result<()>;
    pub fn scan(&self) -> HeapScan;   // Iterator<Item = Result<(RecordId, TupleMeta, Vec<u8>)>>
}
pub struct HeapScan { /* page cursor + slot cursor */ }
```

## crates/buffer — `ferricdb-buffer` (Milestone 2)

```rust
pub trait Replacer: Send {
    fn record_access(&mut self, frame: FrameId);
    fn set_evictable(&mut self, frame: FrameId, evictable: bool);
    fn evict(&mut self) -> Option<FrameId>;
    fn evictable_count(&self) -> usize;
}
pub struct ClockReplacer { /* ref bits + hand */ }
impl ClockReplacer { pub fn new(num_frames: usize) -> Self; }
pub struct LruKReplacer { /* stretch: history of last K accesses */ }
impl LruKReplacer { pub fn new(num_frames: usize, k: usize) -> Self; }

/// Log hook for the WAL rule (Milestone 9): before a dirty page with
/// pageLSN = L is written out, flush_until(L) must succeed.
pub trait LogFlusher: Send + Sync {
    fn flush_until(&self, lsn: Lsn) -> Result<()>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferPoolStats { pub hits: u64, pub misses: u64 }
impl BufferPoolStats { pub fn hit_rate(&self) -> f64; }

pub struct BufferPoolManager { /* frames: Vec<RwLock<Page>>, meta: Mutex<PoolMeta>, disk, log */ }
impl BufferPoolManager {
    pub fn new(pool_size: usize, disk: DiskManager, replacer: Box<dyn Replacer>) -> Self;
    pub fn set_log_flusher(&self, log: std::sync::Arc<dyn LogFlusher>); // wired in MS9
    pub fn fetch_page_read(&self, id: PageId) -> Result<ReadPageGuard<'_>>;   // shared latch + pin
    pub fn fetch_page_write(&self, id: PageId) -> Result<WritePageGuard<'_>>; // exclusive latch + pin
    pub fn new_page(&self) -> Result<(PageId, WritePageGuard<'_>)>;           // allocate + pin
    pub fn flush_page(&self, id: PageId) -> Result<()>;
    pub fn flush_all(&self) -> Result<()>;
    pub fn dirty_pages(&self) -> Vec<(PageId, Lsn)>;                          // MS9 DPT snapshot
    pub fn stats(&self) -> BufferPoolStats;
    pub fn pool_size(&self) -> usize;
}
impl ferricdb_storage::PageStore for BufferPoolManager { /* via Read/WritePageGuard */ }

/// RAII shared pin + read latch. Deref -> &Page; Drop releases latch then unpins.
pub struct ReadPageGuard<'a> { /* pool ref, frame, Option<RwLockReadGuard<'a, Page>> */ }
/// RAII exclusive pin + write latch. Deref/DerefMut -> Page; DerefMut marks dirty;
/// Drop releases latch then unpins.
pub struct WritePageGuard<'a> { /* pool ref, frame, dirty, Option<RwLockWriteGuard<'a, Page>> */ }
```

## crates/index — `ferricdb-index` (Milestone 5)

```rust
/// Disk-resident B+Tree mapping a single-column key to RecordIds, ordered by
/// composite `(key, rid)` (PostgreSQL heap-TID tie-breaker design: supports
/// non-unique secondary indexes AND Milestone 8 MVCC out-of-place updates).
/// One node = one page. Rejects Value::Null, Float(NaN), and mismatched types.
pub struct BPlusTree { /* Arc<dyn PageStore>, root PageId, key type */ }
impl BPlusTree {
    pub fn create(store: std::sync::Arc<dyn PageStore>, key_type: DataType) -> Result<Self>;
    pub fn open(store: std::sync::Arc<dyn PageStore>, root: PageId, key_type: DataType) -> Self;
    pub fn root_page(&self) -> PageId;
    pub fn insert(&mut self, key: &Value, rid: RecordId) -> Result<()>;   // errors only on duplicate (key, rid)
    pub fn get(&self, key: &Value) -> Result<Vec<RecordId>>;              // all matching rids, sorted
    pub fn delete(&mut self, key: &Value, rid: RecordId) -> Result<bool>; // removes exact (key, rid)
    pub fn range(
        &self,
        lower: std::ops::Bound<Value>,
        upper: std::ops::Bound<Value>,
    ) -> Result<RangeIter>;   // Iterator<Item = Result<(Value, RecordId)>>, leaf-chain walk
    /// Test hook: verify sortedness, balance, leaf-chain, and key-range invariants.
    pub fn check_invariants(&self) -> Result<()>;
}
pub struct RangeIter { /* leaf cursor + upper bound */ }
```

## crates/sql — `ferricdb-sql` (Milestone 3)

```rust
pub enum Keyword { Select, From, Where, Group, Order, By, Asc, Desc, Limit, Insert,
                   Into, Values, Update, Set, Delete, Create, Drop, Table, Index, On,
                   Primary, Key, As, Join, Inner, Cross, Begin, Commit, Rollback,
                   Analyze, Explain, Vacuum, Not, Null, And, Or, Is, Cast,
                   Boolean, Int, Integer, Float, Double, Text, Varchar }
pub enum Token { Keyword(Keyword), Ident(String), StringLit(String), IntegerLit(i64),
                 FloatLit(f64), BoolLit(bool), Eq, NotEq, Lt, LtEq, Gt, GtEq,
                 Plus, Minus, Star, Slash, Percent, LParen, RParen, Comma, Semicolon, Dot }
pub struct Lexer;
impl Lexer { pub fn tokenize(input: &str) -> Result<Vec<Token>>; }

pub mod ast {
    pub enum Statement {
        CreateTable { name: String, columns: Vec<ColumnDef> },
        DropTable { name: String },
        Insert { table: String, columns: Option<Vec<String>>, rows: Vec<Vec<Expr>> },
        Select(Box<Select>),
        Update { table: String, set: Vec<(String, Expr)>, filter: Option<Expr> },
        Delete { table: String, filter: Option<Expr> },
        Begin, Commit, Rollback,
        CreateIndex { name: String, table: String, column: String },       // MS5
        Analyze { table: Option<String> },                                  // MS7
        Explain { analyze: bool, inner: Box<Statement> },                   // MS7
        Vacuum,                                                             // MS8
    }
    pub struct ColumnDef { pub name: String, pub data_type: DataType,
                           pub nullable: bool, pub primary_key: bool }
    pub enum SelectItem { Wildcard, Expr { expr: Expr, alias: Option<String> } }
    pub struct Select {
        pub projection: Vec<SelectItem>,
        pub from: Vec<TableRef>,                // joins from MS6
        pub filter: Option<Expr>,
        pub group_by: Vec<Expr>,                // MS6
        pub order_by: Vec<(Expr, bool /*desc*/)>,
        pub limit: Option<u64>,
    }
    pub enum TableRef {
        Table { name: String, alias: Option<String> },
        Join { left: Box<TableRef>, right: Box<TableRef>,
               kind: JoinKind, on: Option<Expr> },                          // MS6
    }
    pub enum JoinKind { Inner, Cross }
    pub enum Expr {
        Literal(Value), Column(Option<String> /*qualifier*/, String),
        Unary(UnaryOp, Box<Expr>), Binary(Box<Expr>, BinaryOp, Box<Expr>),
        IsNull(Box<Expr>, bool /*negated*/), Cast(Box<Expr>, DataType),
        Function(String, Vec<Expr>),            // lowercase; COUNT(*) = Function("count", vec![])
    }
    pub enum UnaryOp { Not, Neg }
    pub enum BinaryOp { And, Or, Eq, NotEq, Lt, LtEq, Gt, GtEq,
                        Add, Sub, Mul, Div, Rem }
}

/// Recursive-descent statement parser + Pratt expression parser.
pub struct Parser;
impl Parser {
    pub fn parse(sql: &str) -> Result<Vec<ast::Statement>>;
    pub fn parse_expr(sql: &str) -> Result<ast::Expr>;   // test convenience
}
```

## crates/catalog — `ferricdb-catalog` (Milestone 3; indexes in MS5; stats in MS7)

```rust
pub struct IndexInfo { pub oid: Oid, pub name: String, pub table: String,
                       pub column: String, pub root_page: PageId }
pub struct TableInfo { pub oid: Oid, pub name: String, pub schema: Schema,
                       pub first_page: PageId, pub indexes: Vec<IndexInfo> }

/// Per-column statistics gathered by ANALYZE (Milestone 7).
#[derive(Clone, Debug, Default)]
pub struct ColumnStats { pub min: Option<Value>, pub max: Option<Value>,
                         pub distinct_estimate: u64, pub null_count: u64 }
#[derive(Clone, Debug, Default)]
pub struct TableStats { pub row_count: u64, pub page_count: u32,
                        pub columns: Vec<ColumnStats> }

/// System catalog, itself stored in heap pages (bootstrap: fixed page ids
/// for __tables / __columns; the catalog reads itself at open).
pub struct Catalog { /* Arc<dyn PageStore>, cached maps */ }
impl Catalog {
    pub fn bootstrap(store: std::sync::Arc<dyn PageStore>) -> Result<Self>; // create-or-open
    pub fn create_table(&self, name: &str, schema: Schema) -> Result<Oid>;
    pub fn drop_table(&self, name: &str) -> Result<()>;
    pub fn table(&self, name: &str) -> Result<TableInfo>;
    pub fn tables(&self) -> Result<Vec<TableInfo>>;
    pub fn create_index(&self, name: &str, table: &str, column: &str,
                        root_page: PageId) -> Result<Oid>;                  // MS5
    pub fn set_index_root(&self, index_oid: Oid, root: PageId) -> Result<()>;
    pub fn stats(&self, table: &str) -> Result<Option<TableStats>>;         // MS7
    pub fn set_stats(&self, table: &str, stats: TableStats) -> Result<()>;  // MS7
}
```

## crates/txn — `ferricdb-txn` (Milestone 8; MS9 restore)

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub owner: TxnId,
    pub xmin: TxnId,
    pub xmax: TxnId,
    /// Other in-flight transactions (excluding `owner`).
    pub active: std::collections::BTreeSet<TxnId>,
}
impl Snapshot {
    /// MVCC visibility of a tuple version to this snapshot, given the
    /// commit state oracle. `xmin == 0` is treated as frozen/committed.
    pub fn is_visible(&self, meta: &TupleMeta,
                      committed: &dyn Fn(TxnId) -> bool) -> bool;
}

pub enum TxnState { Active, Committed, Aborted }
pub struct Transaction { /* id, snapshot, write set */ }
impl Transaction {
    pub fn id(&self) -> TxnId;
    pub fn snapshot(&self) -> &Snapshot;
}

pub struct TransactionManager { /* next txid (>= 1), states, active set */ }
impl TransactionManager {
    pub fn new() -> Self;
    pub fn begin(&self) -> std::sync::Arc<Transaction>;
    /// First-committer-wins: aborts and returns Err(Error::Serialization) if
    /// the write set intersects a concurrently-committed transaction's writes.
    pub fn commit(&self, txn: &Transaction) -> Result<()>;
    pub fn abort(&self, txn: &Transaction);
    pub fn is_committed(&self, id: TxnId) -> bool;
    pub fn is_active(&self, id: TxnId) -> bool;
    pub fn record_write(&self, txn: &Transaction, rid: RecordId);
    /// Oldest xmin any active snapshot can still see (VACUUM horizon).
    pub fn oldest_visible_xid(&self) -> TxnId;
    /// Restore committed txns and advance next_txid after WAL recovery (MS9).
    pub fn restore(&self, max_txn_id: TxnId, committed: impl IntoIterator<Item = TxnId>);
}
```

## crates/wal — `ferricdb-wal` (Milestone 9)

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum ClrAction {
    Tombstone,
    Restore { meta: TupleMeta, tuple: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogRecord {
    PageInit { page_id: PageId },
    Begin  { txn: TxnId },
    Commit { txn: TxnId },
    Abort  { txn: TxnId },
    Insert { txn: TxnId, rid: RecordId, tuple: Vec<u8>, meta: TupleMeta },
    Update { txn: TxnId, rid: RecordId,
             old_meta: TupleMeta, old: Vec<u8>,
             new_meta: TupleMeta, new: Vec<u8> },
    Delete { txn: TxnId, rid: RecordId, old_meta: TupleMeta, old: Vec<u8> },
    Clr    { txn: TxnId, rid: RecordId, action: ClrAction, undo_next: Lsn },
    CheckpointBegin,
    CheckpointEnd { active_txns: Vec<(TxnId, Lsn /*lastLSN*/)>,
                    dirty_pages: Vec<(PageId, Lsn /*recLSN*/)> },
}

pub struct WalManager { /* append-only file, in-memory tail, flushed_lsn */ }
impl WalManager {
    pub fn open(path: &std::path::Path) -> Result<Self>;
    pub fn append(&self, prev_lsn: Lsn, rec: &LogRecord) -> Result<Lsn>; // returns Lsn >= 1
    pub fn flush(&self, up_to: Lsn) -> Result<()>;
    pub fn flushed_lsn(&self) -> Lsn;
    pub fn iter_from(&self, lsn: Lsn) -> Result<WalIter>; // Iterator<Item=Result<(Lsn, Lsn /*prev*/, LogRecord)>>
    pub fn checkpoint(&self, active: &[(TxnId, Lsn)],
                      dirty: &[(PageId, Lsn)]) -> Result<Lsn>; // returns CheckpointEnd LSN; persists checkpoint metadata
}
pub struct WalIter { /* cursor over durable WAL records */ }
impl ferricdb_buffer::LogFlusher for WalManager { /* flush_until = flush */ }

pub struct RecoveryReport {
    pub redone: usize,
    pub undone: usize,
    pub loser_txns: Vec<TxnId>,
    pub committed_txns: Vec<TxnId>,
    pub max_txn_id: TxnId,
}
/// ARIES: analysis (seed pre-checkpoint commits/max_txn_id from checkpoint metadata,
/// rebuild ATT+DPT+post-checkpoint commits from last CheckpointBegin),
/// redo (repeat history from min recLSN, honoring DPT & pageLSN),
/// undo (losers in reverse LSN order, writing CLRs + Abort).
pub fn recover(wal: &WalManager, pool: &BufferPoolManager) -> Result<RecoveryReport>;
/// ARIES physical undo for a live ROLLBACK (shared undo loop with recover() Pass 3).
pub fn rollback_txn(wal: &WalManager, store: &dyn ferricdb_storage::PageStore,
                    txn: TxnId, last_lsn: Lsn) -> Result<()>;
```

## crates/exec — `ferricdb-exec` (Milestones 4, 6, 7; binder in MS3)

```rust
pub mod binder {
    pub enum BoundExpr {
        Literal(Value),
        Column { table: usize, column: usize, data_type: DataType },
        /// Flat offset into immediate child's output (e.g. post-Aggregate [group_by..., aggs...]).
        InputRef { index: usize, data_type: DataType },
        Unary(UnaryOp, Box<BoundExpr>),
        Binary(Box<BoundExpr>, BinaryOp, Box<BoundExpr>),
        IsNull(Box<BoundExpr>, bool),
        Cast(Box<BoundExpr>, DataType),
        Function(String, Vec<BoundExpr>),
    }
    pub enum AggFunc { Count, Sum, Avg, Min, Max }
    pub struct AggExpr { pub func: AggFunc, pub arg: Option<BoundExpr> }
    pub enum BoundTableRef { Table(TableInfo), Join { left: Box<BoundTableRef>, right: Box<BoundTableRef>, kind: JoinKind, on: Option<BoundExpr> } }
    pub struct BoundSelect { pub from: Vec<BoundTableRef>, pub projection: Vec<(BoundExpr, String)>,
                             pub filter: Option<BoundExpr>, pub group_by: Vec<BoundExpr>,
                             pub aggregates: Vec<AggExpr>, pub order_by: Vec<(BoundExpr, bool)>,
                             pub limit: Option<u64>, pub schema: Schema }
    pub enum BoundStatement { CreateTable{..}, DropTable{..}, Insert{..}, Select(Box<BoundSelect>),
                              Update{..}, Delete{..}, Begin, Commit, Rollback,
                              CreateIndex{..}, Analyze{..}, Explain{..}, Vacuum }
    pub struct Binder<'a> { /* &'a Catalog */ }
    impl<'a> Binder<'a> {
        pub fn new(catalog: &'a Catalog) -> Self;
        pub fn bind(&self, stmt: ast::Statement) -> Result<BoundStatement>;
    }
}

pub mod plan {
    pub enum LogicalPlan {
        SeqScan { table_idx: usize, table: TableInfo, filter: Option<BoundExpr> },
        IndexScan { table_idx: usize, table: TableInfo, index: IndexInfo,
                    lower: Bound<Value>, upper: Bound<Value> },             // MS5
        Filter { input: Box<LogicalPlan>, predicate: BoundExpr },
        Projection { input: Box<LogicalPlan>, exprs: Vec<BoundExpr>, schema: Schema },
        NestedLoopJoin { left: Box<LogicalPlan>, right: Box<LogicalPlan>,
                         on: Option<BoundExpr> },                            // MS6
        HashJoin { left: Box<LogicalPlan>, right: Box<LogicalPlan>,
                   left_keys: Vec<BoundExpr>, right_keys: Vec<BoundExpr> },  // MS6 (build left, probe right)
        Aggregate { input: Box<LogicalPlan>, group_by: Vec<BoundExpr>,
                    aggregates: Vec<AggExpr> },                              // MS6
        Sort { input: Box<LogicalPlan>, keys: Vec<(BoundExpr, bool)> },      // NULLs LAST in ASC & DESC
        Limit { input: Box<LogicalPlan>, limit: u64 },
        Insert { table: TableInfo, input: Box<LogicalPlan> },
        Update { table: TableInfo, set: Vec<(usize, BoundExpr)>, input: Box<LogicalPlan> },
        Delete { table: TableInfo, input: Box<LogicalPlan> },
        Values { rows: Vec<Vec<BoundExpr>>, schema: Schema },
    }
}

/// One row flowing between Volcano operators, carrying an optional RecordId
/// from base scans/filters so Update and Delete compose cleanly.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecRow { pub values: Vec<Value>, pub rid: Option<RecordId> }
impl ExecRow {
    pub fn new(values: Vec<Value>) -> Self;
    pub fn with_rid(values: Vec<Value>, rid: RecordId) -> Self;
}

/// Volcano iterator contract. next() returns Ok(None) at end of stream.
pub trait Executor {
    fn schema(&self) -> &Schema;
    fn open(&mut self) -> Result<()>;
    fn next(&mut self) -> Result<Option<ExecRow>>;
    fn close(&mut self) -> Result<()>;
}

/// WAL adapter hook wired by `crates/ferricdb` in Milestone 9.
pub trait WalHook: Send + Sync {
    fn log_page_init(&self, page_id: PageId) -> Result<Lsn>;
    fn log_begin(&self, txn: TxnId) -> Result<Lsn>;
    fn log_commit(&self, txn: TxnId) -> Result<Lsn>;
    fn log_abort(&self, txn: TxnId) -> Result<Lsn>;
    fn rollback(&self, txn: TxnId, store: &dyn PageStore) -> Result<()>;
    fn log_insert(&self, txn: TxnId, rid: RecordId, meta: TupleMeta, tuple: &[u8]) -> Result<Lsn>;
    fn log_update(&self, txn: TxnId, rid: RecordId, old_meta: TupleMeta, old: &[u8],
                  new_meta: TupleMeta, new: &[u8]) -> Result<Lsn>;
    fn log_delete(&self, txn: TxnId, rid: RecordId, old_meta: TupleMeta, old: &[u8]) -> Result<Lsn>;
    fn flush_until(&self, lsn: Lsn) -> Result<()>;
}

pub const DEFAULT_MEMORY_PAGES: usize = 64;
pub struct ExecContext {
    pub catalog: Arc<Catalog>,
    pub store: Arc<dyn PageStore>,
    pub txn_manager: Arc<TransactionManager>,
    pub current_txn: Mutex<Option<Arc<Transaction>>>,
    pub failed_txn: Mutex<bool>,
    pub wal: Option<Arc<dyn WalHook>>,
    pub memory_pages: usize,
}
impl ExecContext {
    pub fn new(catalog: Arc<Catalog>, store: Arc<dyn PageStore>,
               txn_manager: Arc<TransactionManager>) -> Self;
    pub fn new_session(&self) -> Self; // isolated per-connection session for Server
}

pub struct Planner<'a> { /* &'a ExecContext */ }
impl<'a> Planner<'a> {
    pub fn new(ctx: &'a ExecContext) -> Self;
    pub fn plan(&self, stmt: binder::BoundStatement) -> Result<plan::LogicalPlan>;
    /// MS4: identity; MS7: rewrite rules + cost-based access path & join order.
    pub fn optimize(&self, plan: plan::LogicalPlan) -> Result<plan::LogicalPlan>;
    pub fn create_executor(&self, plan: &plan::LogicalPlan) -> Result<Box<dyn Executor>>;
}

pub struct QueryResult { pub schema: Schema, pub rows: Vec<Vec<Value>>,
                         pub affected: u64, pub tag: String, pub explain: Option<String> }
/// One-stop entry used by REPL, tests, and server.
pub fn execute(sql: &str, ctx: &ExecContext) -> Result<Vec<QueryResult>>;

pub mod cost {                                                 // MS7
    pub const SEQ_PAGE_COST: f64 = 1.0;      // matches Postgres defaults
    pub const RANDOM_PAGE_COST: f64 = 4.0;
    pub const CPU_TUPLE_COST: f64 = 0.01;
    pub struct CostModel<'a> { /* &'a Catalog */ }
    impl<'a> CostModel<'a> {
        pub fn new(catalog: &'a Catalog) -> Self;
        pub fn plan_cost(&self, plan: &LogicalPlan) -> f64;
        pub fn selectivity(&self, table: &TableInfo, pred: &BoundExpr) -> f64;
    }
}

pub mod sort {                                                 // MS6
    /// External merge sort: sorted runs spilled via the page store's temp
    /// pages (serialized with `schema`), k-way merge (`k = max(2, memory_pages - 1)`).
    pub struct ExternalSorter { /* ... */ }
    impl ExternalSorter {
        pub fn new(store: Arc<dyn PageStore>, schema: Schema,
                   memory_pages: usize, keys: Vec<(usize, bool)>) -> Self;
        pub fn push(&mut self, row: Vec<Value>) -> Result<()>;
        pub fn finish(self) -> Result<Vec<Vec<Value>>>;
        pub fn spilled_runs(&self) -> usize;
    }
}
```

## crates/server — `ferricdb-server` (Milestone 10)

```rust
/// PostgreSQL frontend/backend protocol v3, hand-rolled incremental framing.
pub mod protocol {
    pub const PROTOCOL_VERSION: u32 = 196608; // 3.0
    pub const SSL_REQUEST_CODE: u32 = 80877103;

    pub enum FrontendMessage {
        SslRequest,
        StartupMessage { params: Vec<(String, String)> },
        Query { sql: String },
        Parse { statement: String, sql: String, param_types: Vec<u32> },
        Bind { portal: String, statement: String, params: Vec<Option<Vec<u8>>> },
        Execute { portal: String, max_rows: i32 },
        Describe { kind: u8, name: String },
        Sync, Terminate,
        PasswordMessage { password: String },
    }
    pub enum BackendMessage {
        AuthenticationOk, AuthenticationCleartextPassword,
        ParameterStatus { name: String, value: String },
        BackendKeyData { process_id: u32, secret_key: u32 },
        ReadyForQuery { txn_status: u8 },
        RowDescription { fields: Vec<FieldDescription> },
        DataRow { values: Vec<Option<Vec<u8>>> },
        CommandComplete { tag: String },
        ErrorResponse { severity: String, code: String, message: String },
        EmptyQueryResponse, ParseComplete, BindComplete,
    }
    pub struct FieldDescription { pub name: String, pub type_oid: u32, pub type_len: i16 }

    // Incremental non-blocking wire codecs (Ok(None) = need more bytes)
    pub fn read_message(buf: &mut bytes::BytesMut, startup: bool) -> Result<Option<FrontendMessage>>;
    pub fn write_message(buf: &mut bytes::BytesMut, msg: &BackendMessage) -> Result<()>;
    pub fn write_frontend_message(buf: &mut bytes::BytesMut, msg: &FrontendMessage) -> Result<()>;
    pub fn read_backend_message(buf: &mut bytes::BytesMut) -> Result<Option<BackendMessage>>;
}

pub struct ServerConfig { pub addr: String /* default 127.0.0.1:5433 */,
                          pub password: Option<String> }
pub struct Server { /* config, Arc<ExecContext> */ }
impl Server {
    pub fn new(config: ServerConfig, ctx: std::sync::Arc<ExecContext>) -> Self;
    pub async fn run(self) -> Result<()>;                                 // bind config.addr + serve
    pub async fn serve(self, listener: tokio::net::TcpListener) -> Result<()>;
}
```

## crates/ferricdb — `ferricdb` binary (Milestone 0, grows every milestone)

```rust
// REPL (rustyline): reads SQL lines / meta-commands, prints result tables.
// Meta-commands: .help  .exit  .tables  .schema <table>  .ast <sql>  .open <path>
// Flags: ferricdb [--db <path>] [--serve [addr]]   (--serve from MS10)
pub const META_COMMANDS: &str = ...;
pub struct Repl { /* ExecContext + rustyline editor */ }
impl Repl { pub fn new(db_path: Option<&Path>) -> Result<Self>;
            pub fn run(&mut self) -> Result<()>; }
```

## Test skeleton naming (starter workspace)

Every provided test is `#[ignore = "milestone N"]` and named
`msN_<what>`, e.g. `ms1_slotted_page_roundtrip`, `ms2_clock_evicts_unpinned`,
`ms5_btree_invariants_fuzz`, `ms8_write_skew_is_documented`,
`ms9_crash_injection_no_lost_commits`. Learners opt in per milestone:
`cargo test -p ferricdb-storage -- --include-ignored ms1_`.
sqllogictest files live in `crates/ferricdb/tests/slt/msN_*.slt`, run via
`ms4_slt_crud` (Milestone 4), `ms6_slt_joins_agg_sort` (Milestone 6), or
`slt_all` (from Milestone 6 onward).
