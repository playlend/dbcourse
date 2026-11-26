//! Milestone 2 hit-rate benchmark: fetch throughput and hit rate on a skewed
//! workload whose working set exceeds the pool.
//!
//! Run with `cargo bench -p ferricdb-buffer` AFTER implementing the pool —
//! it panics on the `todo!()` stubs until then (benches are not run by
//! `cargo test` or CI). The Definition of Done for Milestone 2 asks you to
//! record the hit rate and throughput this reports.

use criterion::{criterion_group, criterion_main, Criterion};
use ferricdb_buffer::{BufferPoolManager, ClockReplacer};
use ferricdb_storage::DiskManager;

fn bench_hit_rate(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let disk = DiskManager::open(&dir.path().join("bench.db")).unwrap();
    let pool = BufferPoolManager::new(64, disk, Box::new(ClockReplacer::new(64)));

    // 256 pages, pool of 64: a fixed 1,120-fetch skewed access pattern where
    // 80% of fetches hit the 32 hot pages and 20% (224 fetches) walk all 224
    // cold pages (5 * 17 = 85 is coprime to 224, so every cold page is visited
    // once per pass and the working set is all 256 pages — 4x the pool size).
    let mut ids = Vec::new();
    for _ in 0..256 {
        let (id, _guard) = pool.new_page().expect("implement Milestone 2 first");
        ids.push(id);
    }
    let pattern: Vec<usize> = (0..1120)
        .map(|i| {
            if i % 5 == 0 {
                32 + (i * 17) % 224
            } else {
                (i * 13) % 32
            }
        })
        .collect();

    // Warm pass + print observed hit rate once for the learner's notes.
    let before = pool.stats();
    for &p in &pattern {
        let g = pool.fetch_page_read(ids[p]).unwrap();
        std::hint::black_box(g.data()[0]);
    }
    let after = pool.stats();
    let hits = after.hits - before.hits;
    let misses = after.misses - before.misses;
    eprintln!(
        "skewed 256p/64f pass: hits={hits} misses={misses} hit_rate={:.1}%",
        100.0 * hits as f64 / (hits + misses).max(1) as f64
    );

    c.bench_function("fetch_skewed_256p_64f", |b| {
        b.iter(|| {
            for &p in &pattern {
                let g = pool.fetch_page_read(ids[p]).unwrap();
                std::hint::black_box(g.data()[0]);
            }
        })
    });
}

criterion_group!(benches, bench_hit_rate);
criterion_main!(benches);
