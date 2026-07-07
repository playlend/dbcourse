//! FerricDB buffer pool — **you build this in Milestone 2**.
//!
//! From this milestone on, every page access in FerricDB flows through
//! [`BufferPoolManager`]: it caches pages in fixed [`ferricdb_common::FrameId`]
//! frames, pins pages in use via the RAII [`PageGuard`], writes dirty
//! victims back before reusing their frames, and picks victims through a
//! pluggable [`Replacer`] ([`ClockReplacer`] required, [`LruKReplacer`]
//! stretch).
//!
//! The [`LogFlusher`] hook is dormant until Milestone 9, when it enforces
//! the WAL rule: before a dirty page with pageLSN = L is written out,
//! `flush_until(L)` must succeed.
//!
//! Milestone page: `course/project/milestone-02/index.html`.
//! Concept module: `course/modules/m03-buffer-pool/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

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
/// with pageLSN = L is written to disk, `flush_until(L)` must succeed.
pub trait LogFlusher: Send + Sync {
    fn flush_until(&self, lsn: Lsn) -> Result<()>;
}

/// The buffer pool manager.
///
/// Owns `pool_size` frames, a page table (PageId → FrameId), per-frame pin
/// counts and dirty bits, the [`Replacer`], and the [`DiskManager`] it
/// caches. All methods take `&self` — the pool is shared as
/// `Arc<BufferPoolManager>` and synchronizes internally (a single pool-wide
/// `Mutex` is a fine first design; see the milestone page before getting
/// cleverer).
pub struct BufferPoolManager {
    pool_size: usize,
    disk: DiskManager,
    replacer: Mutex<Box<dyn Replacer>>,
    log: Mutex<Option<Arc<dyn LogFlusher>>>,
    /* frames, page table, pin counts, dirty bits — your fields here */
}

impl BufferPoolManager {
    /// Create a pool with `pool_size` frames over `disk`.
    pub fn new(pool_size: usize, disk: DiskManager, replacer: Box<dyn Replacer>) -> Self {
        BufferPoolManager {
            pool_size,
            disk,
            replacer: Mutex::new(replacer),
            log: Mutex::new(None),
        }
    }

    /// Install the WAL hook (Milestone 9). Until then the pool flushes
    /// dirty pages without a log check.
    pub fn set_log_flusher(&self, log: Arc<dyn LogFlusher>) {
        *self.log.lock().unwrap() = Some(log);
    }

    /// Pin page `id` and return a guard for it.
    ///
    /// Hit: pin, record access, return. Miss: find a free frame or evict a
    /// victim (writing it back first if dirty — after `flush_until(pageLSN)`
    /// once the log hook is set), read the page in, update the page table
    /// on both sides, pin, return. If every frame is pinned:
    /// [`ferricdb_common::Error::BufferPoolExhausted`].
    pub fn fetch_page(&self, id: PageId) -> Result<PageGuard<'_>> {
        todo!("Milestone 2")
    }

    /// Allocate a brand-new page on disk and pin it (zeroed) in a frame.
    pub fn new_page(&self) -> Result<(PageId, PageGuard<'_>)> {
        todo!("Milestone 2")
    }

    /// Write page `id` through to disk if resident and dirty. Not an evict.
    pub fn flush_page(&self, id: PageId) -> Result<()> {
        todo!("Milestone 2")
    }

    /// Flush every dirty frame. Called on clean shutdown (and by tests).
    pub fn flush_all(&self) -> Result<()> {
        todo!("Milestone 2")
    }

    /// Number of frames.
    pub fn pool_size(&self) -> usize {
        self.pool_size
    }
}

/// RAII pin on one page.
///
/// `Deref` gives read access to the [`Page`]; `DerefMut` marks the frame
/// dirty and gives write access; `Drop` unpins (returning the frame to the
/// replacer's evictable set when the pin count reaches zero).
pub struct PageGuard<'a> {
    pool: &'a BufferPoolManager,
    frame: FrameId,
    /* pointer to the frame's Page — your field here */
}

impl<'a> Deref for PageGuard<'a> {
    type Target = Page;

    fn deref(&self) -> &Page {
        todo!("Milestone 2")
    }
}

impl<'a> DerefMut for PageGuard<'a> {
    fn deref_mut(&mut self) -> &mut Page {
        // remember: this is where the dirty bit gets set
        todo!("Milestone 2")
    }
}

impl<'a> Drop for PageGuard<'a> {
    fn drop(&mut self) {
        // Unpin; flip evictability when the pin count reaches zero.
        // Implement in Milestone 2. (Deliberately not todo!(): a panic in
        // Drop during an unwinding test would abort the whole test run.)
    }
}

/// The pool is a drop-in [`PageStore`]: from Milestone 2 on, `HeapFile`,
/// the catalog, and the B+Tree all take `Arc<BufferPoolManager>`.
impl PageStore for BufferPoolManager {
    fn with_page(&self, id: PageId, f: &mut dyn FnMut(&Page)) -> Result<()> {
        todo!("Milestone 2")
    }

    fn with_page_mut(&self, id: PageId, f: &mut dyn FnMut(&mut Page)) -> Result<()> {
        todo!("Milestone 2")
    }

    fn allocate_page(&self) -> Result<PageId> {
        todo!("Milestone 2")
    }

    fn num_pages(&self) -> u32 {
        todo!("Milestone 2")
    }

    fn flush_all(&self) -> Result<()> {
        BufferPoolManager::flush_all(self)
    }
}
