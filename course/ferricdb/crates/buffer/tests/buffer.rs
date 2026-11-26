//! Provided Milestone 2 test suite (plus MS9 WAL-rule LogFlusher hook test).
//!
//! Run the 8 core tests with
//! `cargo test -p ferricdb-buffer -- --include-ignored ms2_`
//! (and the optional LRU-K stretch test with `--include-ignored stretch_lruk`).
//! `ms2_pagestore_impl_serves_heapfile` verifies that Milestone 1's
//! `HeapFile` runs unmodified over your `BufferPoolManager`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use ferricdb_buffer::{BufferPoolManager, ClockReplacer, LogFlusher, LruKReplacer, Replacer};
use ferricdb_common::{Error, Lsn, PageId, Result};
use ferricdb_storage::{DiskManager, Page, PageStore};

fn pool(dir: &tempfile::TempDir, frames: usize) -> BufferPoolManager {
    let disk = DiskManager::open(&dir.path().join("t.db")).unwrap();
    BufferPoolManager::new(frames, disk, Box::new(ClockReplacer::new(frames)))
}

/// Allocate `n` pages on a fresh pool, stamping page i's byte 100 with i.
fn seed_pages(pool: &BufferPoolManager, n: u32) -> Vec<PageId> {
    let mut ids = Vec::new();
    for i in 0..n {
        let (id, mut guard) = pool.new_page().expect("new_page");
        guard.data_mut()[100] = i as u8;
        ids.push(id);
        // guard drops here: page unpinned, dirty
    }
    ids
}

// ------------------------------------------------------------- replacer

/// REQUIRED NAME (api-spec): CLOCK evicts the first unpinned frame whose
/// reference bit is clear, giving recently-used frames a second chance.
#[test]
#[ignore = "milestone 2"]
fn ms2_clock_evicts_unpinned() {
    let mut clock = ClockReplacer::new(4);
    for f in 0..4usize {
        clock.record_access(f);
        clock.set_evictable(f, true);
    }
    assert_eq!(clock.evictable_count(), 4);

    // All ref bits are set: the hand clears 0,1,2,3 then takes frame 0.
    assert_eq!(clock.evict(), Some(0), "full sweep lands back on frame 0");

    // Re-access frame 1; 2 and 3 have clear bits now, hand is past 0.
    clock.record_access(1);
    assert_eq!(clock.evict(), Some(2), "frame 1's second chance skips it");

    // Pinned frames are invisible to the policy.
    clock.set_evictable(3, false);
    assert_eq!(clock.evict(), Some(1));
    assert_eq!(clock.evict(), None, "only a pinned frame remains");
    assert_eq!(clock.evictable_count(), 0);
}

#[test]
#[ignore = "milestone 2 (stretch)"]
fn stretch_lruk_backward_distance() {
    // LRU-2: A accessed at t1, t5; B at t2, t3; C once at t4; D once at t6.
    // Frames with < 2 accesses have infinite backward 2-distance and are
    // evicted first, ties broken FIFO by earliest access (C at t4 before D at
    // t6). Then A's 2nd-most-recent access (t1) is older than B's (t2).
    let mut lru = LruKReplacer::new(4, 2);
    for f in 0..4usize {
        lru.set_evictable(f, true);
    }
    lru.record_access(0); // A t1
    lru.record_access(1); // B t2
    lru.record_access(1); // B t3
    lru.record_access(2); // C t4 — single access
    lru.record_access(0); // A t5
    lru.record_access(3); // D t6 — single access (newer than C)

    assert_eq!(
        lru.evict(),
        Some(2),
        "single-access frames have infinite distance; oldest (C) goes first"
    );
    assert_eq!(lru.evict(), Some(3), "D is the remaining <k frame");
    assert_eq!(
        lru.evict(),
        Some(0),
        "A's 2nd-most-recent (t1) is older than B's (t2)"
    );
    assert_eq!(lru.evict(), Some(1));
}

// ------------------------------------------------------------- the pool

#[test]
#[ignore = "milestone 2"]
fn ms2_fetch_roundtrip_through_eviction() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pool(&dir, 3);
    let ids = seed_pages(&pool, 10); // far more pages than frames

    // every page's stamp must survive eviction + refetch
    for (i, id) in ids.iter().enumerate() {
        let guard = pool.fetch_page_read(*id).expect("fetch");
        assert_eq!(guard.data()[100], i as u8, "dirty victim was written back");
    }
}

#[test]
#[ignore = "milestone 2"]
fn ms2_pinned_pages_never_evicted() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pool(&dir, 3);
    let ids = seed_pages(&pool, 4);

    let _g0 = pool.fetch_page_read(ids[0]).unwrap();
    let _g1 = pool.fetch_page_read(ids[1]).unwrap();
    let _g2 = pool.fetch_page_read(ids[2]).unwrap();

    // all three frames pinned: the fourth page cannot enter
    match pool.fetch_page_read(ids[3]) {
        Err(Error::BufferPoolExhausted) => {}
        Ok(_) => panic!("expected BufferPoolExhausted, got a page guard"),
        Err(e) => panic!("expected BufferPoolExhausted, got {e:?}"),
    }

    drop(_g1); // one unpin frees exactly one frame
    pool.fetch_page_read(ids[3])
        .expect("fetch succeeds after an unpin");
}

#[test]
#[ignore = "milestone 2"]
fn ms2_concurrent_readers_share_page() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pool(&dir, 4);
    let ids = seed_pages(&pool, 1);

    // Two simultaneous read guards on the SAME page must not deadlock.
    let r1 = pool.fetch_page_read(ids[0]).unwrap();
    let r2 = pool.fetch_page_read(ids[0]).unwrap();
    assert_eq!(r1.data()[100], 0);
    assert_eq!(r2.data()[100], 0);
}

#[test]
#[ignore = "milestone 2"]
fn ms2_resident_fetch_does_no_io() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let pool = BufferPoolManager::new(
        4,
        DiskManager::open(&path).unwrap(),
        Box::new(ClockReplacer::new(4)),
    );
    let ids = seed_pages(&pool, 2);

    // Write through a guard WITHOUT flushing, then fetch again: the change
    // must be served from the frame, and disk must not have been written yet
    // (write-back, not write-through).
    {
        let mut g = pool.fetch_page_write(ids[0]).unwrap();
        g.data_mut()[200] = 0x5A;
    }
    let stats_before = pool.stats();
    let g = pool.fetch_page_read(ids[0]).unwrap();
    assert_eq!(g.data()[200], 0x5A, "resident fetch must not hit disk");
    assert_eq!(
        pool.stats().hits,
        stats_before.hits + 1,
        "resident fetch increments hit counter"
    );

    let disk_check = DiskManager::open(&path).unwrap();
    let mut on_disk = Page::new();
    disk_check.read_page(ids[0], &mut on_disk).unwrap();
    assert_ne!(
        on_disk.data()[200],
        0x5A,
        "dirty page stays buffered until flush or eviction"
    );
}

#[test]
#[ignore = "milestone 2"]
fn ms2_flush_makes_dirty_pages_durable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let (id1, id2);
    {
        let disk = DiskManager::open(&path).unwrap();
        let pool = BufferPoolManager::new(4, disk, Box::new(ClockReplacer::new(4)));
        let (p1, mut g1) = pool.new_page().unwrap();
        id1 = p1;
        g1.data_mut()[300] = 0x77;
        drop(g1);
        pool.flush_page(id1).expect("flush_page");

        let (p2, mut g2) = pool.new_page().unwrap();
        id2 = p2;
        g2.data_mut()[300] = 0x88;
        drop(g2);
        pool.flush_all().expect("flush_all");
    }
    // a completely fresh disk manager sees both flushed bytes
    let disk = DiskManager::open(&path).unwrap();
    let mut page = Page::new();
    disk.read_page(id1, &mut page).unwrap();
    assert_eq!(page.data()[300], 0x77);
    disk.read_page(id2, &mut page).unwrap();
    assert_eq!(page.data()[300], 0x88);
}

#[test]
#[ignore = "milestone 2"]
fn ms2_stress_multithreaded() {
    let dir = tempfile::tempdir().unwrap();
    let disk = DiskManager::open(&dir.path().join("t.db")).unwrap();
    let pool = Arc::new(BufferPoolManager::new(
        8,
        disk,
        Box::new(ClockReplacer::new(8)),
    ));
    let ids = seed_pages(&pool, 32);

    // Each thread owns byte offset 400+t of every page and increments it
    // under an exclusive WritePageGuard; any lost update, torn frame, or
    // eviction of a pinned page shows up as a wrong final count.
    const THREADS: usize = 8;
    const ROUNDS: usize = 50;
    let mut handles = Vec::new();
    for t in 0..THREADS {
        let pool = Arc::clone(&pool);
        let ids = ids.clone();
        handles.push(thread::spawn(move || {
            for r in 0..ROUNDS {
                for &id in &ids {
                    let mut g = pool.fetch_page_write(id).expect("fetch under contention");
                    let off = 400 + t;
                    let v = g.data()[off];
                    assert_eq!(v as usize, r, "thread {t} sees its own prior writes");
                    g.data_mut()[off] = v + 1;
                }
            }
        }));
    }
    for h in handles {
        h.join().expect("no thread panicked");
    }
    for &id in &ids {
        let g = pool.fetch_page_read(id).unwrap();
        for t in 0..THREADS {
            assert_eq!(g.data()[400 + t] as usize, ROUNDS, "no lost updates");
        }
    }
}

#[test]
#[ignore = "milestone 2"]
fn ms2_pagestore_impl_serves_heapfile() {
    use ferricdb_common::TupleMeta;
    use ferricdb_storage::HeapFile;

    let dir = tempfile::tempdir().unwrap();
    let disk = DiskManager::open(&dir.path().join("t.db")).unwrap();
    let pool: Arc<dyn PageStore> = Arc::new(BufferPoolManager::new(
        4,
        disk,
        Box::new(ClockReplacer::new(4)),
    ));

    // the Milestone 1 heap runs unmodified over the pool
    let heap = HeapFile::create(Arc::clone(&pool)).unwrap();
    let mut rids = Vec::new();
    for i in 0..200u32 {
        rids.push(heap.insert(TupleMeta::default(), &i.to_le_bytes()).unwrap());
    }
    for (i, rid) in rids.iter().enumerate() {
        let (_, bytes) = heap.get(*rid).unwrap().expect("live");
        assert_eq!(bytes, (i as u32).to_le_bytes());
    }
    for rid in rids.iter().step_by(5) {
        heap.delete(*rid).unwrap();
    }
    let live = heap.scan().collect::<Result<Vec<_>>>().unwrap();
    assert_eq!(live.len(), 200 - 40);
}

// -------------------------------------------------- Milestone 9 WAL rule

struct MockFlusher {
    flushed_up_to: AtomicU64,
}

impl LogFlusher for MockFlusher {
    fn flush_until(&self, lsn: Lsn) -> Result<()> {
        self.flushed_up_to.fetch_max(lsn, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
#[ignore = "milestone 9"]
fn ms9_log_flusher_enforced_before_dirty_write() {
    let dir = tempfile::tempdir().unwrap();
    let pool = pool(&dir, 2);
    let flusher = Arc::new(MockFlusher {
        flushed_up_to: AtomicU64::new(0),
    });
    pool.set_log_flusher(Arc::clone(&flusher) as Arc<dyn LogFlusher>);

    let (p0, mut g0) = pool.new_page().unwrap();
    g0.set_lsn(42);
    g0.data_mut()[100] = 0xAA;
    drop(g0);

    // Flushing p0 must call flush_until(>= 42) before writing.
    pool.flush_page(p0).unwrap();
    assert!(
        flusher.flushed_up_to.load(Ordering::SeqCst) >= 42,
        "flush_page must enforce the WAL rule"
    );

    // Dirtying p0 to LSN 99 and forcing eviction via two more pages must
    // call flush_until(>= 99) before evicting p0.
    {
        let mut g0 = pool.fetch_page_write(p0).unwrap();
        g0.set_lsn(99);
        g0.data_mut()[100] = 0xBB;
    }
    let (_p1, g1) = pool.new_page().unwrap();
    drop(g1);
    let (_p2, g2) = pool.new_page().unwrap();
    drop(g2);
    let (_p3, g3) = pool.new_page().unwrap();
    drop(g3);
    assert!(
        flusher.flushed_up_to.load(Ordering::SeqCst) >= 99,
        "evicting a dirty victim must enforce the WAL rule"
    );
}
