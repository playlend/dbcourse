//! Milestone 2 hit-rate benchmark: fetch throughput on a skewed workload
//! whose working set exceeds the pool.
//!
//! Run with `cargo bench -p ferricdb-buffer` AFTER implementing the pool —
//! it panics on the `todo!()` stubs until then (benches are not run by
//! `cargo test` or CI). The Definition of Done for Milestone 2 asks you to
//! record the number this reports.

use criterion::{criterion_group, criterion_main, Criterion};
use ferricdb_buffer::{BufferPoolManager, ClockReplacer};
use ferricdb_storage::DiskManager;

fn bench_hit_rate(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let disk = DiskManager::open(&dir.path().join("bench.db")).unwrap();
    let pool = BufferPoolManager::new(64, disk, Box::new(ClockReplacer::new(64)));

    // 256 pages, pool of 64: a fixed skewed access pattern (80% of fetches
    // hit the first 32 pages) — deterministic, no RNG at bench time.
    let mut ids = Vec::new();
    for _ in 0..256 {
        let (id, _guard) = pool.new_page().expect("implement Milestone 2 first");
        ids.push(id);
    }
    let pattern: Vec<usize> = (0..1024)
        .map(|i| {
            if i % 5 == 0 {
                32 + (i * 7) % 224
            } else {
                (i * 13) % 32
            }
        })
        .collect();

    c.bench_function("fetch_skewed_256p_64f", |b| {
        b.iter(|| {
            for &p in &pattern {
                let g = pool.fetch_page(ids[p]).unwrap();
                std::hint::black_box(g.data()[0]);
            }
        })
    });
}

criterion_group!(benches, bench_hit_rate);
criterion_main!(benches);
