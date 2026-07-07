//! Provided Milestone 9 test suite.
//!
//! Run with `cargo test -p ferricdb-wal -- --include-ignored ms9_`.
//!
//! The sim-crash tests here "crash" by dropping the buffer pool without
//! flushing data pages (the WAL itself having been flushed per the rule) —
//! the in-process mini version of the crash-injection harness. The full
//! kill-the-process harness is your Milestone 9 exercise; keep the helpers
//! below reusable for it.

use std::path::Path;
use std::sync::Arc;

use ferricdb_buffer::{BufferPoolManager, ClockReplacer, LogFlusher};
use ferricdb_common::{PageId, RecordId, TupleMeta, TxnId};
use ferricdb_storage::{DiskManager, HeapFile, PageStore};
use ferricdb_wal::{recover, LogRecord, WalManager};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn rid(page_id: PageId, slot: u16) -> RecordId {
    RecordId { page_id, slot }
}

fn sample_records() -> Vec<LogRecord> {
    vec![
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
            redo: b"world".to_vec(),
            undo_next: 3,
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
        assert!(
            lsns.last().map_or(true, |&l| lsn > l),
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
}

#[test]
#[ignore = "milestone 9"]
fn ms9_flushed_lsn_advances_only_on_flush() {
    let dir = tempfile::tempdir().unwrap();
    let wal = WalManager::open(&dir.path().join("wal")).unwrap();

    let a = wal.append(0, &LogRecord::Begin { txn: 1 }).unwrap();
    let b = wal.append(a, &LogRecord::Commit { txn: 1 }).unwrap();
    assert!(
        wal.flushed_lsn() < a,
        "append alone does not make records durable"
    );

    wal.flush(a).unwrap();
    assert!(wal.flushed_lsn() >= a);
    assert!(wal.flushed_lsn() < b, "flush is up-to-LSN, not everything");

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
        // drop without further ceremony — the "crash"
    }
    let wal = WalManager::open(&path).unwrap();
    let got: Vec<_> = wal
        .iter_from(0)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].0, a);
    assert_eq!(got[1].0, b);
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
        "checkpoint flushes through itself"
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
/// tuples, everything logged by hand the way the executor will do it in
/// the full system.
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
    SimDb { wal, pool, heap }
}

/// Log + apply one insert for `txn`, returning (rid, lsn).
fn logged_insert(db: &SimDb, txn: TxnId, prev: u64, byte: u8) -> (RecordId, u64) {
    let meta = TupleMeta { xmin: txn, xmax: 0 };
    let r = db.heap.insert(meta, &[byte]).unwrap();
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
    (r, lsn)
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
        let db = open_sim2_reopen(dir.path(), 4, winner_rid, loser_rid);
        let report = recover(&db.0, &db.1).expect("recovery succeeds");
        assert!(report.loser_txns.contains(&2), "T2 is a loser");
        assert!(!report.loser_txns.contains(&1), "T1 committed");

        let heap = HeapFile::open(Arc::clone(&db.2) as Arc<dyn PageStore>, db.3);
        let winner = heap.get(winner_rid).unwrap();
        assert!(winner.is_some(), "committed insert survives the crash");
        assert_eq!(winner.unwrap().1, vec![0xAA]);
        assert!(
            heap.get(loser_rid).unwrap().is_none(),
            "uncommitted insert rolled back"
        );
    }
}

/// Reopen helper for the sim-crash tests: fresh WAL + pool over the same
/// files. Returns (wal, pool, store, heap_first_page).
fn open_sim2_reopen(
    dir: &Path,
    frames: usize,
    any_rid: RecordId,
    _other: RecordId,
) -> (
    Arc<WalManager>,
    Arc<BufferPoolManager>,
    Arc<BufferPoolManager>,
    PageId,
) {
    let disk = DiskManager::open(&dir.join("data.db")).unwrap();
    let wal = Arc::new(WalManager::open(&dir.join("wal")).unwrap());
    let pool = Arc::new(BufferPoolManager::new(
        frames,
        disk,
        Box::new(ClockReplacer::new(frames)),
    ));
    pool.set_log_flusher(Arc::clone(&wal) as Arc<dyn LogFlusher>);
    // heap first page: HeapFile::create allocated page 0 first in this
    // scripted scenario
    let _ = any_rid;
    let first = 0;
    (wal, Arc::clone(&pool), pool, first)
}

#[test]
#[ignore = "milestone 9"]
fn ms9_recovery_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let r;
    {
        let db = open_sim(dir.path(), 4);
        let b = db.wal.append(0, &LogRecord::Begin { txn: 1 }).unwrap();
        let (r1, l) = logged_insert(&db, 1, b, 0x11);
        r = r1;
        let c = db.wal.append(l, &LogRecord::Commit { txn: 1 }).unwrap();
        db.wal.flush(c).unwrap();
    }
    let disk = DiskManager::open(&dir.path().join("data.db")).unwrap();
    let wal = WalManager::open(&dir.path().join("wal")).unwrap();
    let pool = BufferPoolManager::new(4, disk, Box::new(ClockReplacer::new(4)));

    let first = recover(&wal, &pool).unwrap();
    let again = recover(&wal, &pool).unwrap();
    assert_eq!(
        again.redone, 0,
        "second recovery finds nothing to redo (pageLSN test)"
    );
    assert_eq!(again.undone, 0, "and nothing to undo");
    let _ = first;
    let _ = r;
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
                // dirty pages out — the WAL rule keeps this safe
                if rng.random_bool(0.3) {
                    db.pool.flush_all().unwrap();
                }
            }
            // CRASH here (drop without final flush)
        }
        let disk = DiskManager::open(&dir.path().join("data.db")).unwrap();
        let wal = WalManager::open(&dir.path().join("wal")).unwrap();
        let pool = Arc::new(BufferPoolManager::new(
            4,
            disk,
            Box::new(ClockReplacer::new(4)),
        ));
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
