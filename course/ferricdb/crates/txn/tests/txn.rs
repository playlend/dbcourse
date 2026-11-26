//! Provided Milestone 8 test suite (plus MS9 `restore` hook test).
//!
//! Run with `cargo test -p ferricdb-txn -- --include-ignored ms8_`.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::thread;

use ferricdb_common::{Error, RecordId, TupleMeta, TxnId};
use ferricdb_txn::{Snapshot, Transaction, TransactionManager};

/// The M09 module's scenario: we are transaction 100.
/// Snapshot owner=100, xmin=98, xmax=101, active={98} — T98 in flight,
/// T99 aborted, T95/T96 settled long ago, T100 is us.
fn snapshot_100() -> Snapshot {
    Snapshot {
        owner: 100,
        xmin: 98,
        xmax: 101,
        active: BTreeSet::from([98]),
    }
}

/// Commit-state oracle matching the scenario above.
fn oracle(xid: TxnId) -> bool {
    matches!(xid, 95 | 96)
}

/// Every one of the ten interdb §5.6 rules, table-driven.
/// Columns: name, xmin, xmax, expected-visible.
#[test]
#[ignore = "milestone 8"]
fn ms8_visibility_rules_table() {
    let snap = snapshot_100();
    let cases: &[(&str, TxnId, TxnId, bool)] = &[
        ("R1: aborted creator", 99, 0, false),
        ("R2: my own insert, not deleted", 100, 0, true),
        ("R3: my own insert, deleted by me", 100, 100, false),
        ("R4: other in-progress creator", 98, 0, false),
        ("R6: committed creator, no deleter", 95, 0, true),
        ("R6b: committed creator, aborted deleter", 95, 99, true),
        ("R7: committed creator, deleted by me", 95, 100, false),
        (
            "R8: committed creator, other in-progress deleter",
            95,
            98,
            true,
        ),
        (
            "R10: committed creator, deleter committed before snapshot",
            95,
            96,
            false,
        ),
    ];
    for (name, xmin, xmax, want) in cases {
        let meta = TupleMeta {
            xmin: *xmin,
            xmax: *xmax,
        };
        assert_eq!(
            snap.is_visible(&meta, &oracle),
            *want,
            "{name}: xmin={xmin} xmax={xmax}"
        );
    }
}

/// Frozen / bootstrap tuples (`xmin == 0`) behave as committed in the distant
/// past: visible unless deleted by a visible `xmax`.
#[test]
#[ignore = "milestone 8"]
fn ms8_frozen_tuple_xmin_zero_is_visible() {
    let snap = snapshot_100();
    assert!(snap.is_visible(&TupleMeta { xmin: 0, xmax: 0 }, &oracle));
    assert!(snap.is_visible(&TupleMeta { xmin: 0, xmax: 98 }, &oracle));
    assert!(snap.is_visible(&TupleMeta { xmin: 0, xmax: 99 }, &oracle));
    assert!(!snap.is_visible(&TupleMeta { xmin: 0, xmax: 96 }, &oracle));
    assert!(!snap.is_visible(&TupleMeta { xmin: 0, xmax: 100 }, &oracle));
}

/// Rules 5 and 9 need a creator/deleter that committed AFTER the snapshot:
/// T98 commits, but it stays in our active set until we retake.
#[test]
#[ignore = "milestone 8"]
fn ms8_visibility_active_in_snapshot_rules() {
    let snap = snapshot_100();
    let oracle_98_committed = |xid: TxnId| matches!(xid, 95 | 96 | 98);

    // R5: creator committed but active in snapshot → invisible
    let created_by_98 = TupleMeta { xmin: 98, xmax: 0 };
    assert!(!snap.is_visible(&created_by_98, &oracle_98_committed));

    // R9: deleter committed but active in snapshot → still visible
    let deleted_by_98 = TupleMeta { xmin: 95, xmax: 98 };
    assert!(snap.is_visible(&deleted_by_98, &oracle_98_committed));

    // ids >= snapshot xmax count as active too (the future)
    let created_by_105 = TupleMeta { xmin: 105, xmax: 0 };
    let all_committed = |_xid: TxnId| true;
    assert!(!snap.is_visible(&created_by_105, &all_committed));
}

// ---------------------------------------------------------------- manager

#[test]
#[ignore = "milestone 8"]
fn ms8_begin_assigns_increasing_ids_and_snapshots() {
    let mgr = TransactionManager::new();
    let t1 = mgr.begin();
    let t2 = mgr.begin();
    assert!(t1.id() >= 1, "0 is reserved for frozen/none");
    assert!(t2.id() > t1.id(), "ids increase");
    assert_eq!(t2.snapshot().owner, t2.id());
    assert!(
        !t2.snapshot().active.contains(&t2.id()),
        "active holds OTHER in-flight txns, not owner"
    );
    assert!(
        t2.snapshot().active.contains(&t1.id()),
        "a later snapshot sees an earlier active txn as active"
    );
    assert!(mgr.is_active(t1.id()), "in-flight txn is active");
    mgr.commit(&t1).unwrap();
    assert!(!mgr.is_active(t1.id()), "committed txn is no longer active");
    assert!(
        mgr.is_committed(t1.id()),
        "committed txn is marked committed"
    );
    let t3 = mgr.begin();
    assert!(
        !t3.snapshot().active.contains(&t1.id()),
        "committed txns leave new snapshots' active sets"
    );
}

#[test]
#[ignore = "milestone 8"]
fn ms8_first_committer_wins() {
    let mgr = TransactionManager::new();
    let rid = RecordId {
        page_id: 3,
        slot: 7,
    };

    let t1 = mgr.begin();
    let t2 = mgr.begin(); // concurrent: t1 is in t2's snapshot
    mgr.record_write(&t1, rid);
    mgr.record_write(&t2, rid);

    mgr.commit(&t1).expect("first committer wins");
    match mgr.commit(&t2) {
        Err(Error::Serialization(_)) => {}
        other => panic!("second committer must get a serialization error, got {other:?}"),
    }
    assert!(
        !mgr.is_committed(t2.id()),
        "conflicting second committer must be aborted"
    );
    assert!(
        mgr.oldest_visible_xid() > t2.id(),
        "aborted second committer leaves the active set so the horizon advances"
    );
}

#[test]
#[ignore = "milestone 8"]
fn ms8_no_conflict_when_not_concurrent() {
    let mgr = TransactionManager::new();
    let rid = RecordId {
        page_id: 1,
        slot: 1,
    };

    let t1 = mgr.begin();
    mgr.record_write(&t1, rid);
    assert!(mgr.is_active(t1.id()), "in-flight txn is active");
    mgr.commit(&t1).unwrap();
    assert!(!mgr.is_active(t1.id()), "committed txn is no longer active");
    assert!(
        mgr.is_committed(t1.id()),
        "committed txn is marked committed"
    );

    // t2 starts AFTER t1 committed: same rid, no conflict.
    let t2 = mgr.begin();
    mgr.record_write(&t2, rid);
    mgr.commit(&t2).expect("sequential writers never conflict");
}

#[test]
#[ignore = "milestone 8"]
fn ms8_abort_frees_conflicts() {
    let mgr = TransactionManager::new();
    let rid = RecordId {
        page_id: 2,
        slot: 2,
    };

    let t1 = mgr.begin();
    let t2 = mgr.begin();
    mgr.record_write(&t1, rid);
    mgr.abort(&t1);
    assert!(!mgr.is_committed(t1.id()));

    mgr.record_write(&t2, rid);
    mgr.commit(&t2)
        .expect("an aborted writer conflicts with nobody");
}

/// Fuzzy-read prevention at the visibility level: the same snapshot gives
/// the same answer before and after a concurrent commit.
#[test]
#[ignore = "milestone 8"]
fn ms8_fuzzy_read_prevented() {
    let mgr = TransactionManager::new();
    // Seed a real committed row before reader & writer begin.
    let setup = mgr.begin();
    mgr.commit(&setup).unwrap();

    let reader = mgr.begin();
    let writer = mgr.begin();

    let row_v1 = TupleMeta {
        xmin: setup.id(),
        xmax: writer.id(),
    }; // writer deletes v1
    let row_v2 = TupleMeta {
        xmin: writer.id(),
        xmax: 0,
    }; // and writes v2
    let snap = reader.snapshot().clone();
    let o = |xid: TxnId| mgr.is_committed(xid);

    // before the writer commits
    assert!(snap.is_visible(&row_v1, &o));
    assert!(!snap.is_visible(&row_v2, &o));

    mgr.commit(&writer).unwrap();

    // after: the reader's snapshot answers identically (rules 9 and 5)
    assert!(snap.is_visible(&row_v1, &o), "old version stays visible");
    assert!(!snap.is_visible(&row_v2, &o), "new version stays hidden");
}

/// REQUIRED NAME (api-spec). Write skew SUCCEEDS under snapshot isolation —
/// this test documents the anomaly rather than preventing it. The two
/// transactions read overlapping data but write DISJOINT rows, so
/// first-committer-wins finds no intersection and both commit. Flipping
/// this to `Err(Serialization)` is stretch milestone S2 (SSI).
#[test]
#[ignore = "milestone 8"]
fn ms8_write_skew_is_documented() {
    let mgr = TransactionManager::new();
    let alice = RecordId {
        page_id: 1,
        slot: 0,
    }; // doctor Alice's row
    let bob = RecordId {
        page_id: 1,
        slot: 1,
    }; // doctor Bob's row

    // Both transactions read both rows ("two doctors on call"), then each
    // takes a different doctor off call.
    let t1 = mgr.begin();
    let t2 = mgr.begin();
    mgr.record_write(&t1, alice);
    mgr.record_write(&t2, bob);

    mgr.commit(&t1).expect("t1 commits");
    mgr.commit(&t2)
        .expect("t2 ALSO commits: write sets are disjoint — this is write skew, permitted by SI");
}

#[test]
#[ignore = "milestone 8"]
fn ms8_oldest_visible_xid_tracks_active_snapshots() {
    let mgr = TransactionManager::new();
    let old = mgr.begin();
    let old_id = old.id();

    let t = mgr.begin();
    mgr.commit(&t).unwrap();

    assert!(
        mgr.oldest_visible_xid() <= old_id,
        "the horizon cannot pass a live snapshot"
    );
    mgr.commit(&old).unwrap();
    assert!(
        mgr.oldest_visible_xid() > old_id,
        "the horizon advances once the old snapshot is gone"
    );
}

#[test]
#[ignore = "milestone 8"]
fn ms8_concurrent_begins_are_atomic() {
    let mgr = Arc::new(TransactionManager::new());
    let mut handles = Vec::new();
    for _ in 0..8 {
        let mgr = Arc::clone(&mgr);
        handles.push(thread::spawn(move || {
            let mut ids = Vec::new();
            for _ in 0..100 {
                let t: Arc<Transaction> = mgr.begin();
                assert_eq!(t.snapshot().owner, t.id());
                assert!(t.id() < t.snapshot().xmax);
                assert!(!t.snapshot().active.contains(&t.id()));
                ids.push(t.id());
                mgr.commit(&t).unwrap();
            }
            ids
        }));
    }
    let mut all: Vec<TxnId> = handles
        .into_iter()
        .flat_map(|h| h.join().unwrap())
        .collect();
    let n = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(
        all.len(),
        n,
        "txn ids are globally unique under concurrency"
    );
}

#[test]
#[ignore = "milestone 9"]
fn ms9_restore_committed_txns_after_restart() {
    let mgr = TransactionManager::new();
    mgr.restore(15, [3, 7, 12]);
    assert!(mgr.is_committed(3));
    assert!(mgr.is_committed(7));
    assert!(mgr.is_committed(12));
    assert!(!mgr.is_committed(8));

    let fresh = mgr.begin();
    assert!(
        fresh.id() > 15,
        "new transactions after recovery must not reuse pre-crash ids"
    );
    let snap = fresh.snapshot();
    assert!(snap.is_visible(&TupleMeta { xmin: 7, xmax: 0 }, &|x| mgr.is_committed(x)));
    assert!(!snap.is_visible(&TupleMeta { xmin: 8, xmax: 0 }, &|x| mgr.is_committed(x)));
}
