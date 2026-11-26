//! Provided Milestone 9 test suite.
//!
//! Run with `cargo test -p ferricdb-wal -- --include-ignored ms9_`.
//!
//! The sim-crash tests here "crash" by dropping the buffer pool without
//! flushing data pages (while the WAL itself obeys the WAL rule via
//! `set_log_flusher` and `page.set_lsn`) — the in-process mini version of
//! the crash-injection harness. The full kill-the-process harness is your
//! Milestone 9 exercise; keep the helpers below reusable for it.

use std::path::Path;
use std::sync::Arc;

use ferricdb_buffer::{BufferPoolManager, ClockReplacer, LogFlusher};
use ferricdb_common::{PageId, RecordId, TupleMeta, TxnId};
use ferricdb_storage::{DiskManager, HeapFile, HeapPage, PageStore};
use ferricdb_wal::{recover, ClrAction, LogRecord, WalManager};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn rid(page_id: PageId, slot: u16) -> RecordId {
    RecordId { page_id, slot }
}

fn sample_records() -> Vec<LogRecord> {
    vec![
        LogRecord::PageInit { page_id: 2 },
        LogRecord::Begin { txn: 1 },
        LogRecord::Insert {
            txn: 1,
            rid: rid(2, 0),
            tuple: b"hello".to_vec(),
            meta: TupleMeta { xmin: 1, xmax: 0 },
        },
        LogRecord::Update {
            txn: 1,
            rid: rid(2, 0),
            old_meta: TupleMeta { xmin: 1, xmax: 0 },
            old: b"hello".to_vec(),
            new_meta: TupleMeta { xmin: 1, xmax: 0 },
            new: b"world".to_vec(),
        },
        LogRecord::Delete {
            txn: 1,
            rid: rid(2, 0),
            old_meta: TupleMeta { xmin: 1, xmax: 0 },
            old: b"world".to_vec(),
        },
        LogRecord::Clr {
            txn: 1,
            rid: rid(2, 0),
            action: ClrAction::Restore {
                meta: TupleMeta { xmin: 1, xmax: 0 },
                tuple: b"world".to_vec(),
            },
            undo_next: 3,
        },
        LogRecord::Clr {
            txn: 2,
            rid: rid(2, 1),
            action: ClrAction::Tombstone,
            undo_next: 0,
        },
        LogRecord::CheckpointBegin,
        LogRecord::CheckpointEnd {
            active_txns: vec![(1, 40), (2, 55)],
            dirty_pages: vec![(2, 10), (7, 33)],
        },
        LogRecord::Commit { txn: 1 },
        LogRecord::Abort { txn: 2 },
    ]
}

// ---------------------------------------------------------------- the log

#[test]
#[ignore = "milestone 9"]
fn ms9_append_iter_roundtrip_every_variant() {
    let dir = tempfile::tempdir().unwrap();
    let wal = WalManager::open(&dir.path().join("wal")).unwrap();

    let records = sample_records();
    let mut lsns = Vec::new();
    let mut prev = 0;
    for rec in &records {
        let lsn = wal.append(prev, rec).unwrap();
        assert!(lsn >= 1, "real records must have LSN >= 1");
        assert!(
            lsns.last().is_none_or(|&l| lsn > l),
            "LSNs strictly increase"
        );
        lsns.push(lsn);
        prev = lsn;
    }
    wal.flush(*lsns.last().unwrap()).unwrap();

    let got: Vec<_> = wal
        .iter_from(0)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(got.len(), records.len());
    for (i, (lsn, prev_lsn, rec)) in got.iter().enumerate() {
        assert_eq!(*lsn, lsns[i]);
        assert_eq!(
            *prev_lsn,
            if i == 0 { 0 } else { lsns[i - 1] },
            "prevLSN chain"
        );
        assert_eq!(rec, &records[i], "serialization fidelity for {rec:?}");
    }

    // Mid-log iter_from starts at the requested LSN.
    let mid = lsns[4];
    let tail: Vec<_> = wal
        .iter_from(mid)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(tail.len(), records.len() - 4);
    assert_eq!(tail[0].0, mid);
}

#[test]
#[ignore = "milestone 9"]
fn ms9_flushed_lsn_advances_only_on_flush() {
    let dir = tempfile::tempdir().unwrap();
    let wal = WalManager::open(&dir.path().join("wal")).unwrap();

    let a = wal.append(0, &LogRecord::Begin { txn: 1 }).unwrap();
    assert!(
        wal.flushed_lsn() < a,
        "append alone does not make records durable"
    );

    wal.flush(a).unwrap();
    assert!(wal.flushed_lsn() >= a);

    // Appending a new record AFTER flush does not advance flushed_lsn until
    // flushed again (compatible with full-tail WAL buffer flushes).
    let b = wal.append(a, &LogRecord::Commit { txn: 1 }).unwrap();
    assert!(wal.flushed_lsn() < b, "new append is not durable yet");

    wal.flush(b).unwrap();
    assert!(wal.flushed_lsn() >= b);
    wal.flush(b).unwrap(); // idempotent
}

#[test]
#[ignore = "milestone 9"]
fn ms9_flushed_records_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wal");
    let (a, b);
    {
        let wal = WalManager::open(&path).unwrap();
        a = wal.append(0, &LogRecord::Begin { txn: 9 }).unwrap();
        b = wal
            .append(
                a,
                &LogRecord::Insert {
                    txn: 9,
                    rid: rid(1, 1),
                    tuple: b"durable".to_vec(),
                    meta: TupleMeta { xmin: 9, xmax: 0 },
                },
            )
            .unwrap();
        wal.flush(b).unwrap();
        // Append one more record WITHOUT flushing: must vanish on crash.
        let _unflushed = wal.append(b, &LogRecord::Commit { txn: 9 }).unwrap();
        // drop without further ceremony — the "crash"
    }
    let wal = WalManager::open(&path).unwrap();
    let got: Vec<_> = wal
        .iter_from(0)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(got.len(), 2, "only the 2 flushed records survive reopen");
    assert_eq!(got[0].0, a);
    assert_eq!(got[1].0, b);

    // New appends after reopen must receive strictly larger LSNs.
    let c = wal.append(b, &LogRecord::Abort { txn: 9 }).unwrap();
    assert!(c > b, "LSN monotonicity continues after reopen");
}

#[test]
#[ignore = "milestone 9"]
fn ms9_checkpoint_carries_att_and_dpt() {
    let dir = tempfile::tempdir().unwrap();
    let wal = WalManager::open(&dir.path().join("wal")).unwrap();
    let att = vec![(3, 30), (5, 70)];
    let dpt = vec![(11, 20), (12, 60)];
    let end = wal.checkpoint(&att, &dpt).unwrap();
    assert!(
        wal.flushed_lsn() >= end,
        "checkpoint flushes through CheckpointEnd"
    );

    let recs: Vec<_> = wal
        .iter_from(0)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let found = recs.iter().any(|(_, _, r)| {
        matches!(r, LogRecord::CheckpointEnd { active_txns, dirty_pages }
            if active_txns == &att && dirty_pages == &dpt)
    });
    assert!(found, "CheckpointEnd carries the exact ATT and DPT");
}

// ------------------------------------------------------------ recovery

/// A tiny scripted database for sim-crash scenarios: heap of one-byte
/// tuples, logged in strict write-ahead order with `page.set_lsn(lsn)` —
/// exactly the protocol executors follow in the full system.
struct SimDb {
    wal: Arc<WalManager>,
    pool: Arc<BufferPoolManager>,
    heap: HeapFile,
}

fn open_sim(dir: &Path, frames: usize) -> SimDb {
    let disk = DiskManager::open(&dir.join("data.db")).unwrap();
    let wal = Arc::new(WalManager::open(&dir.join("wal")).unwrap());
    let pool = Arc::new(BufferPoolManager::new(
        frames,
        disk,
        Box::new(ClockReplacer::new(frames)),
    ));
    pool.set_log_flusher(Arc::clone(&wal) as Arc<dyn LogFlusher>);
    let store: Arc<dyn PageStore> = Arc::clone(&pool) as Arc<dyn PageStore>;
    let heap = HeapFile::create(store).unwrap();
    // Log and flush the initial heap page formatting so page 0 is valid on
    // disk (and recoverable via PageInit if ever needed).
    let init_lsn = wal
        .append(
            0,
            &LogRecord::PageInit {
                page_id: heap.first_page(),
            },
        )
        .unwrap();
    {
        let mut g = pool.fetch_page_write(heap.first_page()).unwrap();
        g.set_lsn(init_lsn);
    }
    wal.flush(init_lsn).unwrap();
    pool.flush_all().unwrap();
    SimDb { wal, pool, heap }
}

/// Apply one insert in proper write-ahead / pageLSN protocol, returning
/// `(rid, lsn)`.
fn logged_insert(db: &SimDb, txn: TxnId, prev: u64, byte: u8) -> (RecordId, u64) {
    let meta = TupleMeta { xmin: txn, xmax: 0 };
    // Hold the page write latch across insert + WAL append + page.set_lsn so
    // no steal flush can ever evict the dirty page before its pageLSN is set.
    let mut g = db.pool.fetch_page_write(db.heap.first_page()).unwrap();
    let slot = HeapPage::attach(&mut g)
        .insert(meta, &[byte])
        .expect("page 0 has room for tiny test tuples");
    let r = RecordId {
        page_id: db.heap.first_page(),
        slot,
    };
    let lsn = db
        .wal
        .append(
            prev,
            &LogRecord::Insert {
                txn,
                rid: r,
                tuple: vec![byte],
                meta,
            },
        )
        .unwrap();
    g.set_lsn(lsn);
    (r, lsn)
}

fn reopen_sim(dir: &Path, frames: usize) -> (Arc<WalManager>, Arc<BufferPoolManager>) {
    let disk = DiskManager::open(&dir.join("data.db")).unwrap();
    let wal = Arc::new(WalManager::open(&dir.join("wal")).unwrap());
    let pool = Arc::new(BufferPoolManager::new(
        frames,
        disk,
        Box::new(ClockReplacer::new(frames)),
    ));
    pool.set_log_flusher(Arc::clone(&wal) as Arc<dyn LogFlusher>);
    (wal, pool)
}

#[test]
#[ignore = "milestone 9"]
fn ms9_sim_crash_committed_survives_uncommitted_vanishes() {
    let dir = tempfile::tempdir().unwrap();
    let (winner_rid, loser_rid);
    {
        let db = open_sim(dir.path(), 4);
        // T1 inserts A and commits (commit forces a log flush)
        let b1 = db.wal.append(0, &LogRecord::Begin { txn: 1 }).unwrap();
        let (r1, l1) = logged_insert(&db, 1, b1, 0xAA);
        winner_rid = r1;
        let c1 = db.wal.append(l1, &LogRecord::Commit { txn: 1 }).unwrap();
        db.wal.flush(c1).unwrap();

        // T2 inserts B and does NOT commit (its log records are flushed —
        // the WAL rule allows that; commitment is what's missing)
        let b2 = db.wal.append(0, &LogRecord::Begin { txn: 2 }).unwrap();
        let (r2, l2) = logged_insert(&db, 2, b2, 0xBB);
        loser_rid = r2;
        db.wal.flush(l2).unwrap();

        // CRASH: drop everything without pool.flush_all()
    }
    {
        let (wal, pool) = reopen_sim(dir.path(), 4);
        let report = recover(&wal, &pool).expect("recovery succeeds");
        assert!(report.loser_txns.contains(&2), "T2 is a loser");
        assert!(!report.loser_txns.contains(&1), "T1 committed");
        assert!(report.committed_txns.contains(&1), "T1 reported committed");
        assert!(report.max_txn_id >= 2);
        assert!(
            report.redone >= 2,
            "both inserts redone during repeat-history"
        );
        assert!(report.undone >= 1, "T2's insert undone with a CLR");

        let heap = HeapFile::open(Arc::clone(&pool) as Arc<dyn PageStore>, 0);
        let winner = heap.get(winner_rid).unwrap();
        assert!(winner.is_some(), "committed insert survives the crash");
        assert_eq!(winner.unwrap().1, vec![0xAA]);
        assert!(
            heap.get(loser_rid).unwrap().is_none(),
            "uncommitted insert rolled back"
        );
    }
}

#[test]
#[ignore = "milestone 9"]
fn ms9_update_delete_and_checkpoint_recovery() {
    let dir = tempfile::tempdir().unwrap();
    let (r_keep, r_undo_del);
    {
        let db = open_sim(dir.path(), 4);

        // T1 inserts two rows and commits.
        let b1 = db.wal.append(0, &LogRecord::Begin { txn: 1 }).unwrap();
        let (r1, l1) = logged_insert(&db, 1, b1, 0x11);
        let (r2, l2) = logged_insert(&db, 1, l1, 0x22);
        let c1 = db.wal.append(l2, &LogRecord::Commit { txn: 1 }).unwrap();
        db.wal.flush(c1).unwrap();
        r_keep = r1;
        r_undo_del = r2;

        // Take a fuzzy checkpoint mid-flight while T2 is active.
        let b2 = db.wal.append(0, &LogRecord::Begin { txn: 2 }).unwrap();
        let dirty = db.pool.dirty_pages();
        db.wal.checkpoint(&[(2, b2)], &dirty).unwrap();

        // T2 updates r1 and deletes r2, then crashes without committing.
        let mut prev2 = b2;
        {
            let mut g = db.pool.fetch_page_write(r1.page_id).unwrap();
            let old_meta = TupleMeta { xmin: 1, xmax: 0 };
            let new_meta = TupleMeta { xmin: 2, xmax: 0 };
            HeapPage::attach(&mut g)
                .put_at(r1.slot, new_meta, &[0x99])
                .unwrap();
            let u_lsn = db
                .wal
                .append(
                    prev2,
                    &LogRecord::Update {
                        txn: 2,
                        rid: r1,
                        old_meta,
                        old: vec![0x11],
                        new_meta,
                        new: vec![0x99],
                    },
                )
                .unwrap();
            g.set_lsn(u_lsn);
            prev2 = u_lsn;

            HeapPage::attach(&mut g).delete(r2.slot).unwrap();
            let d_lsn = db
                .wal
                .append(
                    prev2,
                    &LogRecord::Delete {
                        txn: 2,
                        rid: r2,
                        old_meta,
                        old: vec![0x22],
                    },
                )
                .unwrap();
            g.set_lsn(d_lsn);
            db.wal.flush(d_lsn).unwrap();
        }
        // Flush dirty page (steal!) so uncommitted mutations hit disk.
        db.pool.flush_all().unwrap();
    }

    let (wal, pool) = reopen_sim(dir.path(), 4);
    let report = recover(&wal, &pool).expect("recovery succeeds");
    assert!(report.loser_txns.contains(&2));
    assert!(report.undone >= 2, "both Update and Delete undone");

    let heap = HeapFile::open(Arc::clone(&pool) as Arc<dyn PageStore>, 0);
    assert_eq!(
        heap.get(r_keep).unwrap().expect("r1 restored").1,
        vec![0x11],
        "uncommitted Update rolled back to before-image"
    );
    assert_eq!(
        heap.get(r_undo_del).unwrap().expect("r2 resurrected").1,
        vec![0x22],
        "uncommitted Delete rolled back and un-tombstoned"
    );
}

#[test]
#[ignore = "milestone 9"]
fn ms9_recovery_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    {
        let db = open_sim(dir.path(), 4);
        let b = db.wal.append(0, &LogRecord::Begin { txn: 1 }).unwrap();
        let (_r1, l) = logged_insert(&db, 1, b, 0x11);
        let c = db.wal.append(l, &LogRecord::Commit { txn: 1 }).unwrap();
        db.wal.flush(c).unwrap();
    }
    let (wal, pool) = reopen_sim(dir.path(), 4);

    let first = recover(&wal, &pool).unwrap();
    assert!(
        first.redone >= 1,
        "first recovery must actually redo the unflushed committed insert"
    );
    let again = recover(&wal, &pool).unwrap();
    assert_eq!(
        again.redone, 0,
        "second recovery finds nothing to redo (pageLSN test)"
    );
    assert_eq!(again.undone, 0, "and nothing to undo");
}

/// REQUIRED NAME (api-spec): the seeded mini crash-injection loop. A random
/// prefix of a scripted workload is made durable, the rest is lost with the
/// "crash"; after recovery, every committed transaction's insert must be
/// present and every uncommitted one absent — for 100 seeded crash points.
#[test]
#[ignore = "milestone 9"]
fn ms9_crash_injection_no_lost_commits() {
    let mut rng = StdRng::seed_from_u64(0xA51E_5001);
    for round in 0..100 {
        let dir = tempfile::tempdir().unwrap();
        let mut committed: Vec<(RecordId, u8)> = Vec::new();
        let mut uncommitted: Vec<RecordId> = Vec::new();
        {
            let db = open_sim(dir.path(), 4);
            let txns = rng.random_range(1..6u64);
            for t in 1..=txns {
                let b = db.wal.append(0, &LogRecord::Begin { txn: t }).unwrap();
                let byte = rng.random::<u8>();
                let (r, l) = logged_insert(&db, t, b, byte);
                let commits = rng.random_bool(0.6);
                if commits {
                    let c = db.wal.append(l, &LogRecord::Commit { txn: t }).unwrap();
                    db.wal.flush(c).unwrap();
                    committed.push((r, byte));
                } else {
                    // maybe the log made it out, maybe not — both are legal
                    if rng.random_bool(0.5) {
                        db.wal.flush(l).unwrap();
                    }
                    uncommitted.push(r);
                }
                // random page flushing pressure (steal): sometimes push
                // dirty pages out — because `logged_insert` stamped pageLSN,
                // the WAL rule forces the WAL flush first so recovery can
                // always undo any stolen uncommitted page.
                if rng.random_bool(0.3) {
                    db.pool.flush_all().unwrap();
                }
            }
            // CRASH here (drop without final flush)
        }
        let (wal, pool) = reopen_sim(dir.path(), 4);
        recover(&wal, &pool).unwrap_or_else(|e| panic!("round {round}: recovery failed: {e:?}"));

        let store: Arc<dyn PageStore> = Arc::clone(&pool) as Arc<dyn PageStore>;
        let heap = HeapFile::open(store, 0);
        for (r, byte) in &committed {
            let got = heap.get(*r).unwrap();
            assert!(
                got.is_some(),
                "round {round}: committed tuple lost at {r:?}"
            );
            assert_eq!(
                got.unwrap().1,
                vec![*byte],
                "round {round}: committed bytes intact"
            );
        }
        for r in &uncommitted {
            assert!(
                heap.get(*r).unwrap().is_none(),
                "round {round}: uncommitted tuple resurrected at {r:?}"
            );
        }
    }
}
