//! FerricDB transaction layer — **you build this in Milestone 8**.
//!
//! MVCC snapshot isolation, PostgreSQL-style: every transaction gets a
//! [`Snapshot`] at BEGIN; every scan filters tuple versions through
//! [`Snapshot::is_visible`] (the ten interdb Ch. 5 rules); writes stamp
//! `xmin`/`xmax` on tuple versions; commits enforce first-committer-wins.
//!
//! Anomaly coverage (stated honestly, like toyDB does): dirty reads,
//! dirty writes, lost updates, fuzzy reads, and phantoms are prevented;
//! **write skew is not** — closing it is stretch milestone S2.
//!
//! Milestone page: `course/project/milestone-08/index.html`.
//! Concept module: `course/modules/m09-transactions-mvcc/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use ferricdb_common::{RecordId, Result, TupleMeta, TxnId};

/// A transaction's view of the world, captured atomically at BEGIN.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// The transaction id that owns this snapshot (sees its own inserts
    /// and deletes via rules 2, 3, and 7).
    pub owner: TxnId,
    /// Oldest transaction id that was active at capture time (including
    /// `owner`); everything below `xmin` is settled (committed or aborted).
    pub xmin: TxnId,
    /// First transaction id NOT yet assigned at capture time; this id and
    /// above are "the future".
    pub xmax: TxnId,
    /// **Other** transaction ids in `[xmin, xmax)` that were in flight at
    /// capture time (excluding `owner`). "Active in the snapshot" for
    /// another txn `x` means `self.active.contains(&x) || x >= self.xmax`.
    pub active: BTreeSet<TxnId>,
}

impl Snapshot {
    /// MVCC visibility of one tuple version to this snapshot.
    ///
    /// `committed(xid)` is the commit-state oracle (`true` = committed,
    /// `false` = aborted or in progress — combine with `self.owner`,
    /// `self.active`, and `self.xmax` to distinguish).
    ///
    /// Special case: `meta.xmin == 0` denotes a **frozen / bootstrap** tuple
    /// (written before Milestone 8 or during catalog bootstrap) and behaves as
    /// committed in the distant past — visible unless deleted by a visible
    /// `xmax`.
    ///
    /// Mirrors interdb Ch. 5 §5.6's ten rules cleanly; the table-driven test
    /// in `tests/txn.rs` walks every one.
    pub fn is_visible(&self, meta: &TupleMeta, committed: &dyn Fn(TxnId) -> bool) -> bool {
        todo!("Milestone 8")
    }
}

/// Lifecycle state of a transaction.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TxnState {
    Active,
    Committed,
    Aborted,
}

/// One transaction: an id, the snapshot captured at BEGIN, and (privately)
/// the write set used for first-committer-wins.
pub struct Transaction {/* id, snapshot, write set — your fields here */}

impl Transaction {
    /// This transaction's id (stamped into `xmin`/`xmax` of tuples it writes).
    pub fn id(&self) -> TxnId {
        todo!("Milestone 8")
    }

    /// The snapshot captured at BEGIN.
    pub fn snapshot(&self) -> &Snapshot {
        todo!("Milestone 8")
    }
}

/// Assigns transaction ids, tracks states and write sets, detects
/// first-committer-wins conflicts, knows the VACUUM horizon, and restores
/// committed state after WAL recovery (Milestone 9).
///
/// All methods take `&self`; the manager is shared as
/// `Arc<TransactionManager>` and synchronizes internally. Keep the
/// commit-time conflict check inside the same critical section that flips
/// the state to `Committed` — see the milestone pitfalls.
pub struct TransactionManager {
    inner: Mutex<ManagerState>,
}

struct ManagerState {
    /* next txid (starts at 1; 0 is reserved for frozen/none), per-txn states,
    active set, committed write sets — your fields here */
}

impl TransactionManager {
    pub fn new() -> Self {
        TransactionManager {
            inner: Mutex::new(ManagerState {}),
        }
    }

    /// Assign the next transaction id (`>= 1`) and capture its snapshot —
    /// atomically, under one lock (the snapshot-race pitfall on the
    /// milestone page).
    pub fn begin(&self) -> Arc<Transaction> {
        todo!("Milestone 8")
    }

    /// Commit. First-committer-wins: if this transaction's write set
    /// intersects the write set of any transaction that committed after
    /// this one's snapshot was taken, mark this transaction `Aborted`
    /// (removing it from the active set so the VACUUM horizon advances) and
    /// return `Err(Error::Serialization)`.
    pub fn commit(&self, txn: &Transaction) -> Result<()> {
        todo!("Milestone 8")
    }

    /// Roll back: mark aborted and remove from the active set. In Milestone 8
    /// visibility rule 1 hides everything this transaction wrote and rule 6b
    /// ignores its aborted `xmax`; in Milestone 9 the WAL layer also runs
    /// ARIES physical undo (writing CLRs) for full ACID durability.
    pub fn abort(&self, txn: &Transaction) {
        todo!("Milestone 8")
    }

    /// Commit-state oracle used by visibility checks.
    pub fn is_committed(&self, id: TxnId) -> bool {
        todo!("Milestone 8")
    }

    /// Whether transaction `id` is currently in flight (`TxnState::Active`).
    /// Used by Milestone 8's write-time first-updater-wins check to distinguish
    /// an active concurrent writer from an already-aborted one.
    pub fn is_active(&self, id: TxnId) -> bool {
        todo!("Milestone 8")
    }

    /// Record that `txn` wrote (inserted a version at / stamped `xmax` on)
    /// `rid`. Feeds the commit-time conflict check.
    pub fn record_write(&self, txn: &Transaction, rid: RecordId) {
        todo!("Milestone 8")
    }

    /// Oldest `xmin` any active snapshot can still see — the VACUUM horizon:
    /// versions whose deleter committed strictly below this are reclaimable.
    /// When no transactions are active, returns the next unassigned `TxnId`.
    pub fn oldest_visible_xid(&self) -> TxnId {
        todo!("Milestone 8")
    }

    /// Restore committed transaction state after crash recovery (Milestone 9).
    ///
    /// Marks every id in `committed` as [`TxnState::Committed`] and advances
    /// the next unassigned transaction id to at least `max_txn_id + 1`, so
    /// recovered tuples stay visible and new transactions never collide with
    /// pre-crash ids.
    pub fn restore(&self, max_txn_id: TxnId, committed: impl IntoIterator<Item = TxnId>) {
        todo!("Milestone 9")
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
