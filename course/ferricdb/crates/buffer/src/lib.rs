//! FerricDB buffer pool — **you build this in Milestone 2**.
//!
//! From this milestone on, every page access in FerricDB flows through
//! [`BufferPoolManager`]: it caches pages in fixed [`ferricdb_common::FrameId`]
//! frames, pins pages in use via RAII read/write latch guards
//! ([`ReadPageGuard`] and [`WritePageGuard`], modeled on CMU 15-445 BusTub
//! and PostgreSQL's shared/exclusive buffer content locks), writes dirty
//! victims back before reusing their frames, and picks victims through a
//! pluggable [`Replacer`] ([`ClockReplacer`] required, [`LruKReplacer`]
//! stretch).
//!
//! The [`LogFlusher`] hook is dormant until Milestone 9, when it enforces
//! the WAL rule: before a dirty page with `pageLSN = L` is written out
//! (whether by eviction, `flush_page`, or `flush_all`), `flush_until(L)`
//! must succeed.
//!
//! Milestone page: `course/project/milestone-02/index.html`.
//! Concept module: `course/modules/m03-buffer-pool/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

use ferricdb_common::{FrameId, Lsn, PageId, Result};
use ferricdb_storage::{DiskManager, Page, PageStore};

/// Victim-selection policy. The pool tells the replacer which frames are
/// candidates ([`Replacer::set_evictable`]) and reports accesses
/// ([`Replacer::record_access`]); the replacer answers [`Replacer::evict`].
///
/// A pinned frame must be marked non-evictable by the pool — the replacer
/// itself never sees pin counts.
pub trait Replacer: Send {
    /// A frame was accessed (fetch hit or fresh load).
    fn record_access(&mut self, frame: FrameId);

    /// Mark a frame evictable (pin count reached zero) or not (first pin).
    fn set_evictable(&mut self, frame: FrameId, evictable: bool);

    /// Choose a victim among the evictable frames, or `None` if there are
    /// none. The chosen frame stops being evictable.
    fn evict(&mut self) -> Option<FrameId>;

    /// How many frames are currently evictable.
    fn evictable_count(&self) -> usize;
}

/// Classic one-bit CLOCK (second chance): a reference bit per frame, set on
/// access; the hand sweeps the frames in a circle, clearing set bits, and
/// evicts at the first evictable frame whose bit is already clear.
pub struct ClockReplacer {/* ref bits + hand + evictable set — your fields here */}

impl ClockReplacer {
    pub fn new(num_frames: usize) -> Self {
        todo!("Milestone 2")
    }
}

impl Replacer for ClockReplacer {
    fn record_access(&mut self, frame: FrameId) {
        todo!("Milestone 2")
    }

    fn set_evictable(&mut self, frame: FrameId, evictable: bool) {
        todo!("Milestone 2")
    }

    fn evict(&mut self) -> Option<FrameId> {
        todo!("Milestone 2")
    }

    fn evictable_count(&self) -> usize {
        todo!("Milestone 2")
    }
}

/// LRU-K (stretch goal): evict the frame whose *backward k-distance* — time
/// since its k-th most recent access — is largest. Frames with fewer than
/// `k` recorded accesses have infinite distance and are evicted first, ties
/// broken by the oldest overall access.
pub struct LruKReplacer {/* history of last K access timestamps per frame — your fields here */}

impl LruKReplacer {
    pub fn new(num_frames: usize, k: usize) -> Self {
        todo!("Milestone 2 (stretch)")
    }
}

impl Replacer for LruKReplacer {
    fn record_access(&mut self, frame: FrameId) {
        todo!("Milestone 2 (stretch)")
    }

    fn set_evictable(&mut self, frame: FrameId, evictable: bool) {
        todo!("Milestone 2 (stretch)")
    }

    fn evict(&mut self) -> Option<FrameId> {
        todo!("Milestone 2 (stretch)")
    }

    fn evictable_count(&self) -> usize {
        todo!("Milestone 2 (stretch)")
    }
}

/// Log hook for the WAL rule (wired in Milestone 9): before a dirty page
/// with `pageLSN = L` is written to disk, `flush_until(L)` must succeed.
pub trait LogFlusher: Send + Sync {
    fn flush_until(&self, lsn: Lsn) -> Result<()>;
}

/// Hit/miss counters exposed by [`BufferPoolManager::stats`] for benchmarks
/// and diagnostics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BufferPoolStats {
    /// Fetches satisfied from a resident frame (zero disk reads).
    pub hits: u64,
    /// Fetches that missed the pool and read from [`DiskManager`].
    pub misses: u64,
}

impl BufferPoolStats {
    /// Fraction of fetches served from memory (`0.0` if no fetches yet).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Private pool metadata protected by one mutex (page table, free list,
/// pin counts, dirty flags, `rec_lsn`s, replacer, and hit/miss stats).
///
/// Keeping all metadata — including the [`Replacer`] — behind one `Mutex`
/// avoids lock-ordering bugs between the page table and replacer, while
/// the actual page bytes live in `frames: Vec<RwLock<Page>>` outside this
/// mutex so readers and writers of different pages run concurrently without
/// holding the pool latch during page work.
struct PoolMeta {
    replacer: Box<dyn Replacer>,
    /* page_table, free_list, frame_meta (page_id, pin_count, dirty, rec_lsn), stats —
    your fields here */
}

/// The buffer pool manager.
///
/// Owns `pool_size` frames (`Vec<RwLock<Page>>`), pool metadata behind a
/// single `Mutex`, the [`DiskManager`], and the optional [`LogFlusher`].
///
/// # Safe latch ordering (no `unsafe`, no deadlocks)
///
/// 1. Lock `meta` to look up / pin a frame (evicting a victim with
///    `pin_count == 0` if needed; wait until `meta` is dropped or lock the
///    unpinned victim frame briefly to read/write its bytes).
/// 2. **Release `meta`** before acquiring `self.frames[frame].read()` or
///    `.write()` for the returned guard!
/// 3. In `Drop` of [`ReadPageGuard`] / [`WritePageGuard`], **drop the
///    frame's `RwLock*Guard` FIRST** (via `Option::take()`) *before*
///    re-locking `meta` to decrement `pin_count`. (Rust drops struct fields
///    only *after* `Drop::drop` returns — releasing the frame lock inside
///    `Drop::drop` prevents an ABBA deadlock if another thread holds `meta`
///    while waiting on a frame latch.)
pub struct BufferPoolManager {
    pool_size: usize,
    disk: DiskManager,
    frames: Vec<RwLock<Page>>,
    meta: Mutex<PoolMeta>,
    log: Mutex<Option<Arc<dyn LogFlusher>>>,
}

impl BufferPoolManager {
    /// Create a pool with `pool_size` frames over `disk`.
    pub fn new(pool_size: usize, disk: DiskManager, replacer: Box<dyn Replacer>) -> Self {
        let frames = (0..pool_size).map(|_| RwLock::new(Page::new())).collect();
        BufferPoolManager {
            pool_size,
            disk,
            frames,
            meta: Mutex::new(PoolMeta { replacer }),
            log: Mutex::new(None),
        }
    }

    /// Install the WAL hook (Milestone 9). Until then the pool flushes
    /// dirty pages without a log check.
    pub fn set_log_flusher(&self, log: Arc<dyn LogFlusher>) {
        *self.log.lock().unwrap() = Some(log);
    }

    /// Pin page `id` and return a **shared (read-only)** guard for it.
    ///
    /// Multiple threads may hold `ReadPageGuard`s for the same page
    /// concurrently; reading never marks a frame dirty.
    ///
    /// Hit: pin, record access, increment hit counter, return.
    /// Miss: find a free frame or evict a victim (if dirty, call
    /// `flush_until(page.lsn())` when a log flusher is installed, then write
    /// to disk), read the page in, update the page table, pin, return.
    /// If every frame is pinned: [`ferricdb_common::Error::BufferPoolExhausted`].
    pub fn fetch_page_read(&self, id: PageId) -> Result<ReadPageGuard<'_>> {
        todo!("Milestone 2")
    }

    /// Pin page `id` and return an **exclusive (read/write)** guard for it.
    ///
    /// Calling `DerefMut` on the returned guard marks the frame dirty (and
    /// records its `rec_lsn` once it transitions clean → dirty in Milestone 9).
    pub fn fetch_page_write(&self, id: PageId) -> Result<WritePageGuard<'_>> {
        todo!("Milestone 2")
    }

    /// Allocate a brand-new zeroed page on disk and return it pinned with an
    /// exclusive [`WritePageGuard`].
    pub fn new_page(&self) -> Result<(PageId, WritePageGuard<'_>)> {
        todo!("Milestone 2")
    }

    /// Write page `id` through to disk if resident and dirty. Not an evict.
    /// Enforces `LogFlusher::flush_until(page.lsn())` before writing when a
    /// log flusher is installed (Milestone 9).
    pub fn flush_page(&self, id: PageId) -> Result<()> {
        todo!("Milestone 2")
    }

    /// Flush every dirty frame (enforcing `LogFlusher::flush_until(page.lsn())`
    /// per dirty frame when installed). Called on clean shutdown and by tests.
    pub fn flush_all(&self) -> Result<()> {
        todo!("Milestone 2")
    }

    /// Snapshot of currently dirty resident pages as `(page_id, rec_lsn)`
    /// pairs, feeding ARIES fuzzy checkpoints in Milestone 9.
    pub fn dirty_pages(&self) -> Vec<(PageId, Lsn)> {
        todo!("Milestone 9")
    }

    /// Snapshot of hit/miss counters since pool creation.
    pub fn stats(&self) -> BufferPoolStats {
        todo!("Milestone 2")
    }

    /// Number of frames.
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }
}

/// RAII shared pin + read latch on one page.
///
/// `Deref` gives `&Page` without dirtying the frame. `Drop` releases the
/// frame read latch first, then unpins the frame (marking it evictable when
/// its pin count reaches zero).
pub struct ReadPageGuard<'a> {
    pool: &'a BufferPoolManager,
    frame: FrameId,
    guard: Option<RwLockReadGuard<'a, Page>>,
}

impl<'a> Deref for ReadPageGuard<'a> {
    type Target = Page;

    fn deref(&self) -> &Page {
        self.guard
            .as_deref()
            .expect("ReadPageGuard accessed after drop")
    }
}

impl<'a> Drop for ReadPageGuard<'a> {
    fn drop(&mut self) {
        // Release the frame latch BEFORE locking pool metadata to unpin!
        // (Deliberately not todo!(): a panic in Drop during an unwinding
        // test would abort the whole test run.)
        drop(self.guard.take());
        // TODO (Milestone 2): unpin `self.frame` in `self.pool`.
    }
}

/// RAII exclusive pin + write latch on one page.
///
/// `Deref` gives `&Page`; `DerefMut` marks the frame dirty and gives
/// `&mut Page`. `Drop` releases the frame write latch first, then unpins.
pub struct WritePageGuard<'a> {
    pool: &'a BufferPoolManager,
    frame: FrameId,
    dirty: bool,
    guard: Option<RwLockWriteGuard<'a, Page>>,
}

impl<'a> Deref for WritePageGuard<'a> {
    type Target = Page;

    fn deref(&self) -> &Page {
        self.guard
            .as_deref()
            .expect("WritePageGuard accessed after drop")
    }
}

impl<'a> DerefMut for WritePageGuard<'a> {
    fn deref_mut(&mut self) -> &mut Page {
        self.dirty = true;
        self.guard
            .as_deref_mut()
            .expect("WritePageGuard accessed after drop")
    }
}

impl<'a> Drop for WritePageGuard<'a> {
    fn drop(&mut self) {
        // Record pageLSN if dirty, release the frame write latch BEFORE
        // locking pool metadata, then propagate dirty/rec_lsn and unpin.
        // (Deliberately not todo!(): never panic in Drop.)
        drop(self.guard.take());
        // TODO (Milestone 2): update dirty flag (and rec_lsn in MS9) and
        // unpin `self.frame` in `self.pool`.
    }
}

/// The pool is a drop-in [`PageStore`]: from Milestone 2 on, `HeapFile`,
/// the catalog, and the B+Tree all take `Arc<BufferPoolManager>`.
impl PageStore for BufferPoolManager {
    fn with_page(&self, id: PageId, f: &mut dyn FnMut(&Page)) -> Result<()> {
        let guard = self.fetch_page_read(id)?;
        f(&guard);
        Ok(())
    }

    fn with_page_mut(&self, id: PageId, f: &mut dyn FnMut(&mut Page)) -> Result<()> {
        let mut guard = self.fetch_page_write(id)?;
        f(&mut guard);
        Ok(())
    }

    fn allocate_page(&self) -> Result<PageId> {
        let (id, _guard) = self.new_page()?;
        Ok(id)
    }

    fn num_pages(&self) -> u32 {
        self.disk.num_pages()
    }

    fn flush_all(&self) -> Result<()> {
        BufferPoolManager::flush_all(self)
    }
}
