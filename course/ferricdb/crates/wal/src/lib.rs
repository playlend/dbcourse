//! FerricDB write-ahead log and recovery — **you build this in Milestone 9**.
//!
//! An append-only log of [`LogRecord`]s with monotonically increasing
//! [`Lsn`]s; the WAL rule enforced through the buffer pool's
//! [`ferricdb_buffer::LogFlusher`] hook (flush the log through a dirty
//! page's pageLSN before that page may be written); and [`recover`] — the
//! ARIES three passes: analysis (rebuild ATT + DPT from the last
//! checkpoint), redo (*repeat history* from the minimum recLSN, applying a
//! record iff the page's pageLSN is less than the record's LSN), and undo
//! (roll back losers in reverse LSN order, writing [`LogRecord::Clr`]s
//! whose `undo_next` chains track rollback progress; CLRs are redo-only and
//! never undone).
//!
//! Milestone page: `course/project/milestone-09/index.html`.
//! Concept module: `course/modules/m10-logging-recovery/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::path::Path;

use ferricdb_buffer::{BufferPoolManager, LogFlusher};
use ferricdb_common::{Lsn, PageId, RecordId, Result, TupleMeta, TxnId};

/// Everything FerricDB ever writes to its log.
///
/// Update-style records carry before-images (`old*`) for undo and
/// after-images for redo — physical logging at tuple granularity, the
/// simplest shape that supports full ARIES.
#[derive(Clone, Debug, PartialEq)]
pub enum LogRecord {
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
    /// Compensation log record, written during undo. `redo` is the
    /// compensating change (applied on redo, never undone); `undo_next`
    /// points at the next record of the same transaction still to undo —
    /// the rollback-progress cursor that makes recovery restartable.
    Clr {
        txn: TxnId,
        rid: RecordId,
        redo: Vec<u8>,
        undo_next: Lsn,
    },
    CheckpointBegin,
    /// Fuzzy checkpoint payload: a photograph of the active-transaction
    /// table (txn → lastLSN) and the dirty page table (page → recLSN).
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
pub struct WalManager {/* file, in-memory tail, next/flushed LSNs — your fields here */}

impl WalManager {
    /// Open (or create) the log file at `path`. Existing records become
    /// iterable; new appends continue after them.
    pub fn open(path: &Path) -> Result<Self> {
        todo!("Milestone 9")
    }

    /// Append a record, chaining it to the same transaction's previous
    /// record via `prev_lsn` (0 for a transaction's first record). Returns
    /// the record's LSN. Appending does NOT flush.
    pub fn append(&self, prev_lsn: Lsn, rec: &LogRecord) -> Result<Lsn> {
        todo!("Milestone 9")
    }

    /// Make every record with LSN ≤ `up_to` durable. Idempotent.
    pub fn flush(&self, up_to: Lsn) -> Result<()> {
        todo!("Milestone 9")
    }

    /// The largest LSN known durable.
    pub fn flushed_lsn(&self) -> Lsn {
        todo!("Milestone 9")
    }

    /// Iterate records with LSN ≥ `lsn`, in order, yielding
    /// `(lsn, prev_lsn, record)`.
    pub fn iter_from(&self, lsn: Lsn) -> Result<WalIter> {
        todo!("Milestone 9")
    }

    /// Write a fuzzy checkpoint (Begin + End with the given tables) and
    /// flush through it. Returns the CheckpointEnd LSN.
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

/// What [`recover`] did — the crash-injection harness asserts on these.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Update-style records re-applied during redo.
    pub redone: usize,
    /// Loser updates rolled back during undo (== CLRs written).
    pub undone: usize,
    /// Transactions found in-flight at the crash and rolled back.
    pub loser_txns: Vec<TxnId>,
}

/// ARIES restart recovery.
///
/// 1. **Analysis** — from the most recent checkpoint, rebuild the
///    active-transaction table (with lastLSNs) and dirty page table (with
///    recLSNs); scan forward to the end of the log. Whoever is left in the
///    ATT is a loser.
/// 2. **Redo** — *repeat history*: from the minimum recLSN, re-apply every
///    update (winners' and losers' alike) whose page's pageLSN is less than
///    the record's LSN. No logging during redo; the pageLSN test is what
///    makes redo idempotent.
/// 3. **Undo** — roll back losers in reverse LSN order (single backward
///    sweep over max lastLSNs), writing a CLR per undone update; a crash
///    during undo resumes from the CLRs' `undo_next` chain.
pub fn recover(wal: &WalManager, pool: &BufferPoolManager) -> Result<RecoveryReport> {
    todo!("Milestone 9")
}
