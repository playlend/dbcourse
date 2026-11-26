//! FerricDB storage layer — **you build this in Milestone 1**.
//!
//! Everything above this crate eventually turns into reads and writes of
//! fixed-size 8 KB pages ([`ferricdb_common::PAGE_SIZE`]). This crate owns:
//!
//! * [`Page`] — one raw page in memory, with the 16-byte reserved header
//!   (pageLSN + page type tag + reserved) every FerricDB page carries;
//! * [`HeapPage`] / [`HeapPageRef`] — mutable and read-only slotted-page
//!   views: line-pointer array growing forward, tuple data growing backward,
//!   tombstone deletes, plus `compact` (Milestone 8 VACUUM) and `put_at`
//!   (Milestone 9 ARIES redo/undo);
//! * [`mod@tuple`] — the serialized tuple format (null bitmap, fixed-width
//!   values, length-prefixed TEXT);
//! * [`DiskManager`] — page-granular file I/O;
//! * [`PageStore`] — how higher layers reach pages. [`DiskManager`]
//!   implements it directly (Milestone 1); the buffer pool supersedes it
//!   from Milestone 2 on;
//! * [`HeapFile`] — a chain of heap pages storing one table, addressed by
//!   [`RecordId`].
//!
//! Milestone page: `course/project/milestone-01/index.html`.
//! Concept module: `course/modules/m02-storage/index.html`.

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::path::Path;
use std::sync::Arc;

use ferricdb_common::{Lsn, PageId, RecordId, Result, Schema, TupleMeta, Value, PAGE_SIZE};

/// Page type tag for a freshly zeroed, unformatted page.
pub const PAGE_TYPE_UNINIT: u32 = 0;
/// Page type tag for a slotted heap page ([`HeapPage`]).
pub const PAGE_TYPE_HEAP: u32 = 1;
/// Page type tag for a B+Tree leaf node (Milestone 5).
pub const PAGE_TYPE_BTREE_LEAF: u32 = 2;
/// Page type tag for a B+Tree internal node (Milestone 5).
pub const PAGE_TYPE_BTREE_INTERNAL: u32 = 3;

/// One 8 KB page in memory.
///
/// The first 16 bytes of **every** FerricDB page are reserved:
///
/// | offset | size | field                                             |
/// |--------|------|---------------------------------------------------|
/// | 0      | 8    | pageLSN (u64, little-endian; 0 = never logged)    |
/// | 8      | 4    | page type tag (u32, little-endian; `PAGE_TYPE_*`) |
/// | 12     | 4    | reserved                                          |
///
/// The pageLSN is written as 0 until Milestone 9 starts stamping it; the
/// accessors exist from day one so the buffer pool and WAL can be wired
/// without changing this type.
pub struct Page {
    data: [u8; PAGE_SIZE],
}

impl Page {
    /// A fresh, zeroed page.
    pub fn new() -> Self {
        Page {
            data: [0u8; PAGE_SIZE],
        }
    }

    /// The pageLSN from the reserved header (bytes 0..8, little-endian).
    pub fn lsn(&self) -> Lsn {
        todo!("Milestone 1")
    }

    /// Stamp the pageLSN (bytes 0..8, little-endian).
    pub fn set_lsn(&mut self, lsn: Lsn) {
        todo!("Milestone 1")
    }

    /// The page type tag from the reserved header (bytes 8..12, little-endian).
    pub fn page_type(&self) -> u32 {
        u32::from_le_bytes(self.data[8..12].try_into().unwrap())
    }

    /// Set the page type tag (bytes 8..12, little-endian).
    pub fn set_page_type(&mut self, tag: u32) {
        self.data[8..12].copy_from_slice(&tag.to_le_bytes());
    }

    /// Raw read access to all 8192 bytes (header included).
    pub fn data(&self) -> &[u8; PAGE_SIZE] {
        &self.data
    }

    /// Raw write access to all 8192 bytes (header included).
    pub fn data_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        &mut self.data
    }
}

impl Default for Page {
    fn default() -> Self {
        Self::new()
    }
}

/// Read-only slotted-page view over `&Page`.
///
/// Use this inside [`PageStore::with_page`] (e.g. `HeapFile::get` and
/// `HeapScan::next`) so read paths never require `&mut Page` and therefore
/// never dirty buffer-pool frames in Milestone 2.
pub struct HeapPageRef<'a> {
    page: &'a Page,
}

impl<'a> HeapPageRef<'a> {
    /// Returns `true` if `page` carries [`PAGE_TYPE_HEAP`] in its header.
    pub fn is_initialized(page: &Page) -> bool {
        page.page_type() == PAGE_TYPE_HEAP
    }

    /// Attach a read-only view to an already-formatted heap page.
    pub fn attach(page: &'a Page) -> Self {
        HeapPageRef { page }
    }

    /// Read the tuple in `slot`. Returns `None` if the slot is out of range
    /// or tombstoned.
    pub fn get(&self, slot: u16) -> Option<(TupleMeta, &[u8])> {
        todo!("Milestone 1")
    }

    /// Number of line pointers ever allocated on this page (live + dead).
    pub fn slot_count(&self) -> u16 {
        todo!("Milestone 1")
    }

    /// Maximum payload byte length of a single new tuple that would currently
    /// fit on this page (after deducting the line pointer and 16-byte
    /// [`TupleMeta`] that a new insert requires). Returns `0` if even an
    /// empty tuple would not fit.
    pub fn free_space(&self) -> usize {
        todo!("Milestone 1")
    }

    /// The next page id in the heap file's linked chain, or
    /// [`ferricdb_common::INVALID_PAGE_ID`] if this is the tail page.
    pub fn next_page(&self) -> PageId {
        todo!("Milestone 1")
    }
}

/// Mutable slotted-page view over `&mut Page`.
///
/// Standard 28-byte header layout (course convention, shared with M02 and MS1):
///
/// | offset | size | field                                                  |
/// |--------|------|--------------------------------------------------------|
/// | 0      | 8    | `page_lsn` (`u64`, LE; 0 = never logged)               |
/// | 8      | 4    | `page_type` (`u32`, LE; [`PAGE_TYPE_HEAP`] = 1)        |
/// | 12     | 4    | `reserved` (`u32`, LE; 0)                              |
/// | 16     | 4    | `next_page` ([`PageId`], LE; `INVALID_PAGE_ID` = tail) |
/// | 20     | 2    | `lower` (`u16`, LE; byte offset of end of line ptrs)   |
/// | 22     | 2    | `upper` (`u16`, LE; byte offset of start of tuples)    |
/// | 24     | 2    | `slot_count` (`u16`, LE; live + dead line pointers)    |
/// | 26     | 2    | `flags` (`u16`, LE; 0)                                 |
///
/// After the 28-byte header:
///
/// * the **line-pointer array**, growing *forward* from byte 28 — one entry
///   per slot holding `(offset, length, flags/live-bit)`;
/// * contiguous **free space** between `lower` and `upper`;
/// * **tuple data**, growing *backward* from `PAGE_SIZE` (`8192`). Each
///   stored tuple is its 16-byte [`TupleMeta`] (`xmin`, `xmax` — written as
///   zeros until Milestone 8) followed by the serialized tuple bytes.
///
/// Slots are never reused by `delete`: `delete` tombstones the line pointer
/// (the slot number stays valid and dead), which is what MVCC needs in
/// Milestone 8. [`HeapPage::compact`] reclaims dead tuple bodies during
/// `VACUUM` while preserving every live slot number.
pub struct HeapPage<'a> {
    page: &'a mut Page,
}

impl<'a> HeapPage<'a> {
    /// Format `page` as an empty heap page (sets [`PAGE_TYPE_HEAP`],
    /// `next_page = INVALID_PAGE_ID`, `lower = 28`, `upper = PAGE_SIZE`,
    /// `slot_count = 0`, `flags = 0`) and return the view.
    pub fn init(page: &'a mut Page) -> Self {
        todo!("Milestone 1")
    }

    /// View an already-formatted heap page.
    pub fn attach(page: &'a mut Page) -> Self {
        todo!("Milestone 1")
    }

    /// Borrow a read-only [`HeapPageRef`] view of the underlying page.
    pub fn as_ref(&self) -> HeapPageRef<'_> {
        HeapPageRef::attach(self.page)
    }

    /// Insert a tuple; returns the slot number, or `None` if the page has
    /// insufficient free space (caller moves on to another page).
    pub fn insert(&mut self, meta: TupleMeta, tuple: &[u8]) -> Option<u16> {
        todo!("Milestone 1")
    }

    /// Read the tuple in `slot`. Returns `None` if the slot is out of range
    /// or tombstoned.
    pub fn get(&self, slot: u16) -> Option<(TupleMeta, &[u8])> {
        todo!("Milestone 1")
    }

    /// Overwrite the stored [`TupleMeta`] of a live tuple (MVCC stamps
    /// `xmin`/`xmax` through this in Milestone 8). The tuple bytes are
    /// untouched.
    ///
    /// Returns `Err(Error::InvalidSlot)` if `slot` is out of range or dead.
    pub fn set_meta(&mut self, slot: u16, meta: TupleMeta) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Tombstone `slot`: the line pointer is marked dead, `get` returns
    /// `None`, and `slot_count` is unchanged.
    ///
    /// Returns `Err(Error::InvalidSlot)` if `slot` is out of range or already
    /// dead.
    pub fn delete(&mut self, slot: u16) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Number of line pointers ever allocated on this page (live + dead).
    pub fn slot_count(&self) -> u16 {
        todo!("Milestone 1")
    }

    /// Maximum payload byte length of a single new tuple that fits right now
    /// (deducting the new line pointer and 16-byte [`TupleMeta`]).
    pub fn free_space(&self) -> usize {
        todo!("Milestone 1")
    }

    /// The next page id in the heap file's linked chain, or
    /// [`ferricdb_common::INVALID_PAGE_ID`] if this is the tail page.
    pub fn next_page(&self) -> PageId {
        todo!("Milestone 1")
    }

    /// Update the next-page link in the heap header.
    pub fn set_next_page(&mut self, next: PageId) {
        todo!("Milestone 1")
    }

    /// Reclaim dead (tombstoned) tuple bodies on this page by repacking all
    /// live tuples tightly toward `PAGE_SIZE` and advancing `upper`.
    ///
    /// **Invariants:** every live tuple keeps its exact slot number (existing
    /// `RecordId`s stay valid) and `slot_count()` is unchanged. Returns the
    /// number of bytes reclaimed (`new_upper - old_upper`).
    ///
    /// Implemented in Milestone 8 (used by `VACUUM`).
    pub fn compact(&mut self) -> usize {
        todo!("Milestone 8")
    }

    /// Idempotently place `(meta, tuple)` at the exact `slot` index
    /// (Milestone 9 — ARIES redo and undo/CLR primitive).
    ///
    /// * If `slot >= slot_count()`, extends the line-pointer array with dead
    ///   (tombstoned) entries up to `slot` and writes the new tuple there.
    /// * If `slot < slot_count()`, replaces or resurrects `slot` with
    ///   `(meta, tuple)` (allocating fresh space at `upper` — compacting
    ///   first if needed — if the new tuple does not fit in the old space).
    ///
    /// Returns `Err(Error::PageFull)` only if the page cannot hold the tuple
    /// even after compaction.
    pub fn put_at(&mut self, slot: u16, meta: TupleMeta, tuple: &[u8]) -> Result<()> {
        todo!("Milestone 9")
    }
}

/// The serialized tuple format.
///
/// Layout: a null bitmap (`ceil(n / 8)` bytes, one bit per column,
/// **bit `1` = NULL** — course convention, contrasting with PostgreSQL's
/// `1 = present`), then each non-NULL value in schema order:
///
/// * `Boolean` — 1 byte (`0` / `1`);
/// * `Integer` — 8 bytes, little-endian `i64`;
/// * `Float`   — 8 bytes, little-endian `f64::to_le_bytes()`;
/// * `Text`    — `u32` little-endian byte length, then that many UTF-8 bytes.
///
/// Values are written unaligned; readers must use byte copies (e.g.
/// `i64::from_le_bytes`), never pointer casts.
pub mod tuple {
    use super::*;

    /// Serialize `values` (must match `schema` in arity and types;
    /// `Value::Null` is allowed only for nullable columns — otherwise return
    /// `Err(Error::Execution)`).
    pub fn serialize(schema: &Schema, values: &[Value]) -> Result<Vec<u8>> {
        todo!("Milestone 1")
    }

    /// Inverse of [`serialize`]. Returns `Err(Error::Execution)` on truncated
    /// input or invalid UTF-8.
    pub fn deserialize(schema: &Schema, bytes: &[u8]) -> Result<Vec<Value>> {
        todo!("Milestone 1")
    }
}

/// Page-granular file I/O. One `DiskManager` owns one database file; page
/// `id` lives at byte offset `id * PAGE_SIZE`.
///
/// All methods take `&self`: the disk manager is shared behind an `Arc` and
/// must synchronize internally (positioned reads/writes or an internal
/// `Mutex<File>` — your choice; see the milestone guidance).
pub struct DiskManager {/* file handle, page counter — your fields here */}

impl DiskManager {
    /// Open (or create) the database file at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        todo!("Milestone 1")
    }

    /// Read page `id` into `out`. Reading a page beyond the end of the file
    /// (`id >= num_pages()`) returns `Err(Error::InvalidPage(id))`.
    pub fn read_page(&self, id: PageId, out: &mut Page) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Write `page` at page offset `id`. Writing to an unallocated page
    /// (`id >= num_pages()`) returns `Err(Error::InvalidPage(id))`.
    pub fn write_page(&self, id: PageId, page: &Page) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Extend the file by one zeroed page and return its id.
    pub fn allocate_page(&self) -> Result<PageId> {
        todo!("Milestone 1")
    }

    /// Number of pages currently in the file.
    pub fn num_pages(&self) -> u32 {
        todo!("Milestone 1")
    }
}

/// How higher layers reach pages.
///
/// [`DiskManager`] implements it directly in Milestone 1; from Milestone 2
/// on, `BufferPoolManager` implements it and *everything* goes through the
/// pool. The closure-based shape (rather than returning references) is what
/// lets the pool hand out access to frames it still owns and pin-counts.
pub trait PageStore: Send + Sync {
    /// Run `f` with read access to page `id`. Does **not** mark the page
    /// dirty in a buffer pool.
    ///
    /// The closure returns nothing so the trait stays object-safe
    /// (`Arc<dyn PageStore>` is how the whole system holds its store);
    /// capture results through the closure's environment:
    ///
    /// ```ignore
    /// let mut lsn = 0;
    /// store.with_page(id, &mut |p| lsn = p.lsn())?;
    /// ```
    fn with_page(&self, id: PageId, f: &mut dyn FnMut(&Page)) -> Result<()>;

    /// Run `f` with write access to page `id`. Pool implementations mark
    /// the frame dirty. Capture results via the closure environment, as
    /// with [`PageStore::with_page`].
    fn with_page_mut(&self, id: PageId, f: &mut dyn FnMut(&mut Page)) -> Result<()>;

    /// Allocate a fresh page and return its id.
    fn allocate_page(&self) -> Result<PageId>;

    /// Total pages in the underlying file.
    fn num_pages(&self) -> u32;

    /// Force everything durable (`sync_data` for `DiskManager`; flushes all
    /// dirty frames for `BufferPoolManager`).
    fn flush_all(&self) -> Result<()>;
}

impl PageStore for DiskManager {
    fn with_page(&self, id: PageId, f: &mut dyn FnMut(&Page)) -> Result<()> {
        todo!("Milestone 1")
    }

    fn with_page_mut(&self, id: PageId, f: &mut dyn FnMut(&mut Page)) -> Result<()> {
        todo!("Milestone 1")
    }

    fn allocate_page(&self) -> Result<PageId> {
        DiskManager::allocate_page(self)
    }

    fn num_pages(&self) -> u32 {
        DiskManager::num_pages(self)
    }

    fn flush_all(&self) -> Result<()> {
        todo!("Milestone 1")
    }
}

/// A sequential collection of heap pages storing one table.
///
/// Pages are chained (each heap page header stores the next page id, or
/// [`ferricdb_common::INVALID_PAGE_ID`] at the tail). `first_page` is the
/// stable identity the catalog stores in `TableInfo::first_page`.
pub struct HeapFile {
    store: Arc<dyn PageStore>,
    first_page: PageId,
}

impl HeapFile {
    /// Allocate the first page of a new heap and format it with
    /// [`HeapPage::init`].
    pub fn create(store: Arc<dyn PageStore>) -> Result<Self> {
        todo!("Milestone 1")
    }

    /// Reopen an existing heap by its first page id.
    pub fn open(store: Arc<dyn PageStore>, first_page: PageId) -> Self {
        HeapFile { store, first_page }
    }

    /// The first page id (persist this in the catalog).
    pub fn first_page(&self) -> PageId {
        self.first_page
    }

    /// Insert a tuple somewhere with room (walking the chain, extending it
    /// when full) and return its address. Returns `Err(Error::PageFull)` if
    /// the tuple is too large to fit even in an empty heap page.
    pub fn insert(&self, meta: TupleMeta, tuple: &[u8]) -> Result<RecordId> {
        todo!("Milestone 1")
    }

    /// Read the tuple at `rid` (using [`PageStore::with_page`] +
    /// [`HeapPageRef`] so reads never dirty frames). Returns `Ok(None)` if
    /// the slot is tombstoned or out of range.
    pub fn get(&self, rid: RecordId) -> Result<Option<(TupleMeta, Vec<u8>)>> {
        todo!("Milestone 1")
    }

    /// Overwrite the [`TupleMeta`] at `rid` (MVCC stamping, Milestone 8).
    pub fn set_meta(&self, rid: RecordId, meta: TupleMeta) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Tombstone the tuple at `rid`.
    pub fn delete(&self, rid: RecordId) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Iterate every live tuple in the heap, in `(page, slot)` order along
    /// the page chain.
    pub fn scan(&self) -> HeapScan {
        todo!("Milestone 1")
    }
}

/// Iterator over a heap file's live tuples.
pub struct HeapScan {/* your cursor state here */}

impl Iterator for HeapScan {
    type Item = Result<(RecordId, TupleMeta, Vec<u8>)>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!("Milestone 1")
    }
}
