//! FerricDB transaction layer — **you build this in Milestone 8**.
//!
//! MVCC snapshot isolation, PostgreSQL-style: every transaction gets a
//! [`Snapshot`] at BEGIN; every scan filters tuple versions through
//! [`Snapshot::is_visible`] (the ten interdb Ch. 5 rules); writes stamp
//! xmin/xmax on tuple versions; commits enforce first-committer-wins.
//!
//! Anomaly coverage (state it honestly, like toyDB does): dirty reads,
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
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Oldest transaction id that was active at capture time; everything
    /// below is settled (committed or aborted).
    pub xmin: TxnId,
    /// First transaction id NOT yet assigned at capture time; this id and
    /// above are "the future".
    pub xmax: TxnId,
    /// Transaction ids in `[xmin, xmax)` that were in flight at capture
    /// time. "Active in the snapshot" means: in this set, or `>= xmax`.
    pub active: BTreeSet<TxnId>,
}

impl Snapshot {
    /// MVCC visibility of one tuple version to this snapshot.
    ///
    /// `committed(xid)` is the commit-state oracle (true = committed,
    /// false = aborted or in progress — combine with `self.active` and the
    /// current txn id embedded in the snapshot capture to distinguish).
    ///
    /// Mirrors interdb Ch. 5 §5.6's ten rules exactly; the table-driven
    /// test in `tests/txn.rs` walks every one. The current transaction's id
    /// is `self.xmax - 1` by construction in this course (the id assigned
    /// at BEGIN, captured with the snapshot) — see the milestone page.
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
    /// This transaction's id (stamped into xmin/xmax of tuples it writes).
    pub fn id(&self) -> TxnId {
        todo!("Milestone 8")
    }

    /// The snapshot captured at BEGIN.
    pub fn snapshot(&self) -> &Snapshot {
        todo!("Milestone 8")
    }
}

/// Assigns transaction ids, tracks states and write sets, detects
/// first-committer-wins conflicts, and knows the VACUUM horizon.
///
/// All methods take `&self`; the manager is shared as
/// `Arc<TransactionManager>` and synchronizes internally. Keep the
/// commit-time conflict check inside the same critical section that flips
/// the state to `Committed` — see the milestone pitfalls.
pub struct TransactionManager {
    inner: Mutex<ManagerState>,
}

struct ManagerState {
    /* next txid, per-txn states, active set, committed write sets —
    your fields here */
}

impl TransactionManager {
    pub fn new() -> Self {
        TransactionManager {
            inner: Mutex::new(ManagerState {}),
        }
    }

    /// Assign the next transaction id and capture its snapshot — atomically,
    /// under one lock (the snapshot-race pitfall on the milestone page).
    pub fn begin(&self) -> Arc<Transaction> {
        todo!("Milestone 8")
    }

    /// Commit. First-committer-wins: if this transaction's write set
    /// intersects the write set of any transaction that committed after
    /// this one's snapshot was taken, abort with
    /// [`ferricdb_common::Error::Serialization`] instead.
    pub fn commit(&self, txn: &Transaction) -> Result<()> {
        todo!("Milestone 8")
    }

    /// Roll back: mark aborted. No undo work — visibility rule 1 hides
    /// everything this transaction wrote; VACUUM reclaims it later.
    pub fn abort(&self, txn: &Transaction) {
        todo!("Milestone 8")
    }

    /// Commit-state oracle used by visibility checks.
    pub fn is_committed(&self, id: TxnId) -> bool {
        todo!("Milestone 8")
    }

    /// Record that `txn` wrote (inserted a version at / stamped xmax on)
    /// `rid`. Feeds the commit-time conflict check.
    pub fn record_write(&self, txn: &Transaction, rid: RecordId) {
        todo!("Milestone 8")
    }

    /// Oldest xmin any active snapshot can still see — the VACUUM horizon:
    /// versions whose deleter committed below this are reclaimable.
    pub fn oldest_visible_xid(&self) -> TxnId {
        todo!("Milestone 8")
    }
}

impl Default for TransactionManager {
    fn default() -> Self {
        Self::new()
    }
}
