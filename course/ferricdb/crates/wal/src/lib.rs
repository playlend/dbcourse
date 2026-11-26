//! FerricDB write-ahead log and recovery — **you build this in Milestone 9**.
//!
//! An append-only log of [`LogRecord`]s with monotonically increasing
//! [`Lsn`]s (`>= 1`; `0` is the "no LSN" sentinel); the WAL rule enforced
//! through the buffer pool's [`ferricdb_buffer::LogFlusher`] hook (flush the
//! log through a dirty page's `pageLSN` before that page may be written);
//! and [`recover`] — the ARIES three passes:
//!
//! 1. **Analysis** — rebuild ATT + DPT from the last checkpoint, plus the
//!    set of committed transaction ids so MVCC visibility survives restarts.
//! 2. **Redo** — *repeat history* from the minimum `recLSN`, applying every
//!    update/CLR/page-init whose page's `pageLSN < record.lsn` (honoring the
//!    DPT `recLSN` filter).
//! 3. **Undo** — roll back losers in reverse LSN order using `HeapPage::put_at`
//!    and `HeapPage::delete`, writing [`LogRecord::Clr`]s whose `undo_next`
//!    chains track rollback progress; CLRs are redo-only and never undone.
//!
//! Milestone page: `course/project/milestone-09/index.html`.
//! Concept module: `course/modules/m10-logging-recovery/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::path::Path;

use ferricdb_buffer::{BufferPoolManager, LogFlusher};
use ferricdb_common::{Lsn, PageId, RecordId, Result, TupleMeta, TxnId};
use ferricdb_storage::PageStore;

/// The compensating physical action recorded inside a [`LogRecord::Clr`].
///
/// Explicit variants avoid undocumented binary sub-encodings:
/// * undoing an `Insert` tombstones `rid` (`ClrAction::Tombstone`);
/// * undoing an `Update` or `Delete` restores the before-image `(meta, tuple)`
///   at `rid` via `HeapPage::put_at` (`ClrAction::Restore`).
#[derive(Clone, Debug, PartialEq)]
pub enum ClrAction {
    /// Undo of an `Insert`: mark `rid` dead via `HeapPage::delete`.
    Tombstone,
    /// Undo of an `Update` or `Delete`: restore `(meta, tuple)` at `rid`
    /// via `HeapPage::put_at`.
    Restore { meta: TupleMeta, tuple: Vec<u8> },
}

/// Everything FerricDB ever writes to its log.
///
/// Update-style records carry before-images (`old*`) for undo and
/// after-images for redo — physical logging at tuple granularity, the
/// cleanest shape that supports full ARIES.
#[derive(Clone, Debug, PartialEq)]
pub enum LogRecord {
    /// Formatting a brand-new heap page (`HeapPage::init`), so crash
    /// recovery can re-initialize an unflushed page before replaying
    /// tuple inserts onto it.
    PageInit {
        page_id: PageId,
    },
    Begin {
        txn: TxnId,
    },
    Commit {
        txn: TxnId,
    },
    Abort {
        txn: TxnId,
    },
    Insert {
        txn: TxnId,
        rid: RecordId,
        tuple: Vec<u8>,
        meta: TupleMeta,
    },
    Update {
        txn: TxnId,
        rid: RecordId,
        old_meta: TupleMeta,
        old: Vec<u8>,
        new_meta: TupleMeta,
        new: Vec<u8>,
    },
    Delete {
        txn: TxnId,
        rid: RecordId,
        old_meta: TupleMeta,
        old: Vec<u8>,
    },
    /// Compensation log record, written during undo. `action` is the
    /// compensating change (applied on redo, never undone); `undo_next`
    /// points at the next record of the same transaction still to undo —
    /// the rollback-progress cursor that makes recovery restartable.
    Clr {
        txn: TxnId,
        rid: RecordId,
        action: ClrAction,
        undo_next: Lsn,
    },
    CheckpointBegin,
    /// Fuzzy checkpoint payload: a photograph of the active-transaction
    /// table (`txn → lastLSN`) and the dirty page table (`page → recLSN`).
    /// Nothing is flushed or blocked to take one.
    CheckpointEnd {
        active_txns: Vec<(TxnId, Lsn)>,
        dirty_pages: Vec<(PageId, Lsn)>,
    },
}

/// The append-only log: an in-memory tail plus a file, with an explicit
/// flush boundary ([`WalManager::flushed_lsn`]).
///
/// All methods take `&self`; the manager is shared (the buffer pool holds
/// it as its [`LogFlusher`]) and synchronizes internally.
///
/// **LSN convention:** every appended record receives a strictly increasing
/// `Lsn >= 1` (`0` is reserved for "no LSN yet"). If you derive LSNs from
/// byte offsets, use `1 + start_offset` or `end_offset` so the first record
/// never gets `Lsn == 0`.
pub struct WalManager {/* file, in-memory tail, next/flushed LSNs — your fields here */}

impl WalManager {
    /// Open (or create) the log file at `path`. Existing durable records
    /// become iterable (any torn trailing bytes from a mid-write crash are
    /// truncated cleanly to the end of the last valid record); new appends
    /// continue with strictly higher LSNs.
    pub fn open(path: &Path) -> Result<Self> {
        todo!("Milestone 9")
    }

    /// Append a record, chaining it to the same transaction's previous
    /// record via `prev_lsn` (`0` for a transaction's first record). Returns
    /// the record's LSN (`>= 1`). Appending does NOT flush.
    pub fn append(&self, prev_lsn: Lsn, rec: &LogRecord) -> Result<Lsn> {
        todo!("Milestone 9")
    }

    /// Make every record with LSN ≤ `up_to` durable (flushing the entire
    /// pending tail buffer whenever `up_to > flushed_lsn()` is standard and
    /// valid). Idempotent.
    pub fn flush(&self, up_to: Lsn) -> Result<()> {
        todo!("Milestone 9")
    }

    /// The largest LSN known durable (`0` on a brand-new empty log).
    pub fn flushed_lsn(&self) -> Lsn {
        todo!("Milestone 9")
    }

    /// Iterate durable records with LSN ≥ `lsn`, in order, yielding
    /// `(lsn, prev_lsn, record)`.
    pub fn iter_from(&self, lsn: Lsn) -> Result<WalIter> {
        todo!("Milestone 9")
    }

    /// Write a fuzzy checkpoint (`CheckpointBegin` + `CheckpointEnd` with
    /// the given tables), flush through it, and atomically persist the
    /// checkpoint metadata (last `CheckpointBegin` LSN plus the cumulative
    /// `committed_txns` and `max_txn_id` observed up to this checkpoint —
    /// PostgreSQL's `pg_control` + `pg_xact` in miniature) so [`recover`]
    /// stays bounded to the last checkpoint without losing pre-checkpoint
    /// MVCC commit history. Returns the `CheckpointEnd` LSN.
    pub fn checkpoint(&self, active: &[(TxnId, Lsn)], dirty: &[(PageId, Lsn)]) -> Result<Lsn> {
        todo!("Milestone 9")
    }
}

/// The WAL rule hook the buffer pool calls before writing a dirty page.
impl LogFlusher for WalManager {
    fn flush_until(&self, lsn: Lsn) -> Result<()> {
        self.flush(lsn)
    }
}

/// Iterator over log records from a starting LSN.
pub struct WalIter {/* your cursor state here */}

impl Iterator for WalIter {
    type Item = Result<(Lsn, Lsn, LogRecord)>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!("Milestone 9")
    }
}

/// What [`recover`] did — feeds both tests and post-crash
/// `TransactionManager::restore(report.max_txn_id, report.committed_txns)`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Update-style / PageInit / CLR records re-applied during redo.
    pub redone: usize,
    /// Loser updates rolled back during undo (== CLRs written).
    pub undone: usize,
    /// Transactions found in-flight at the crash and rolled back.
    pub loser_txns: Vec<TxnId>,
    /// Transactions whose `Commit` record reached the durable WAL.
    pub committed_txns: Vec<TxnId>,
    /// Highest `TxnId` observed anywhere in the WAL (so `TransactionManager`
    /// advances `next_txid` past it on restart).
    pub max_txn_id: TxnId,
}

/// ARIES restart recovery.
///
/// 1. **Analysis** — seed `committed_txns` and `max_txn_id` from the last
///    complete checkpoint's metadata (if any), then scan forward from that
///    checkpoint's `CheckpointBegin` (or the start of the log if none) to
///    rebuild the active-transaction table (ATT: `txn → lastLSN`), dirty page
///    table (DPT: `page → recLSN`), and any post-checkpoint commits / txn ids.
///    Whoever is left in the ATT is a loser.
/// 2. **Redo** — *repeat history*: from the minimum `recLSN` in the DPT,
///    re-apply every `PageInit`, `Insert`, `Update`, `Delete`, and `Clr`
///    whose target page is in the DPT with `recLSN <= record.lsn` and whose
///    on-page `page.lsn() < record.lsn`, stamping `page.set_lsn(record.lsn)`.
///    No logging during redo; the `pageLSN` test makes redo idempotent.
/// 3. **Undo** — roll back losers in reverse LSN order (single backward
///    sweep picking the max `next_to_undo` LSN across active losers),
///    applying the inverse via `HeapPage::delete` / `HeapPage::put_at`,
///    writing a [`LogRecord::Clr`] per undone update and stamping the page's
///    `pageLSN` with the CLR's LSN, and writing [`LogRecord::Abort`] when
///    a loser finishes. Flush the WAL and pool before returning.
pub fn recover(wal: &WalManager, pool: &BufferPoolManager) -> Result<RecoveryReport> {
    todo!("Milestone 9")
}

/// ARIES physical undo for a live `ROLLBACK` (Milestone 9).
///
/// Walks `txn`'s `prev_lsn` chain backward starting at `last_lsn` (including
/// any unflushed in-memory tail records in `wal`), applying inverse page
/// mutations via `store` (`HeapPage::delete` / `HeapPage::put_at`), appending
/// a [`LogRecord::Clr`] per undone mutation (stamping `pageLSN`), and
/// finishing with [`LogRecord::Abort`]. Shares its core undo routine with
/// [`recover`] Pass 3.
pub fn rollback_txn(
    wal: &WalManager,
    store: &dyn PageStore,
    txn: TxnId,
    last_lsn: Lsn,
) -> Result<()> {
    todo!("Milestone 9")
}
