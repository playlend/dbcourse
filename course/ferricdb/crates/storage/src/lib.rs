//! FerricDB storage layer — **you build this in Milestone 1**.
//!
//! Everything above this crate eventually turns into reads and writes of
//! fixed-size 8 KB pages ([`ferricdb_common::PAGE_SIZE`]). This crate owns:
//!
//! * [`Page`] — one raw page in memory, with the 16-byte reserved header
//!   (pageLSN + type tag) every FerricDB page carries;
//! * [`HeapPage`] — a slotted-page view: line-pointer array growing forward,
//!   tuple data growing backward, tombstone deletes;
//! * [`tuple`] — the serialized tuple format (null bitmap, fixed-width
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

/// One 8 KB page in memory.
///
/// The first 16 bytes of **every** FerricDB page are reserved:
///
/// | offset | size | field                                             |
/// |--------|------|---------------------------------------------------|
/// | 0      | 8    | pageLSN (u64, little-endian; 0 = never logged)    |
/// | 8      | 4    | page type tag (u32; heap/btree/catalog…)          |
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

/// Slotted-page view over a [`Page`].
///
/// Layout (after the 16-byte reserved page header):
///
/// * a heap-page header (slot count, free-space pointer);
/// * the **line-pointer array**, growing *forward* — one entry per slot
///   holding (offset, length, live/dead flag);
/// * free space;
/// * **tuple data**, growing *backward* from the end of the page. Each
///   stored tuple is its [`TupleMeta`] (xmin, xmax — written as zeros until
///   Milestone 8) followed by the serialized tuple bytes.
///
/// Slots are never reused within this milestone: `delete` tombstones the
/// line pointer (the slot number stays valid and dead), which is exactly
/// what MVCC needs later. See the milestone page for the free-space
/// arithmetic pitfalls.
pub struct HeapPage<'a> {
    page: &'a mut Page,
}

impl<'a> HeapPage<'a> {
    /// Format `page` as an empty heap page (writes the heap header) and
    /// return the view.
    pub fn init(page: &'a mut Page) -> Self {
        todo!("Milestone 1")
    }

    /// View an already-formatted heap page. Does not validate exhaustively;
    /// callers own page-type discipline (the type tag helps debugging).
    pub fn attach(page: &'a mut Page) -> Self {
        todo!("Milestone 1")
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
    /// xmin/xmax through this in Milestone 8). The tuple bytes are untouched.
    pub fn set_meta(&mut self, slot: u16, meta: TupleMeta) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Tombstone `slot`: the line pointer is marked dead, `get` returns
    /// `None`, and `slot_count` is unchanged. Space is reclaimed later
    /// (VACUUM, Milestone 8), not here.
    pub fn delete(&mut self, slot: u16) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Number of line pointers ever allocated on this page (live + dead).
    pub fn slot_count(&self) -> u16 {
        todo!("Milestone 1")
    }

    /// Bytes still available between the line-pointer array and the tuple
    /// data region (accounting for the line pointer a new insert needs).
    pub fn free_space(&self) -> usize {
        todo!("Milestone 1")
    }
}

/// The serialized tuple format.
///
/// Layout: a null bitmap (one bit per column, `ceil(n/8)` bytes, bit set =
/// NULL), then each non-NULL value in schema order:
///
/// * `Boolean` — 1 byte (0/1);
/// * `Integer` — 8 bytes, little-endian i64;
/// * `Float`   — 8 bytes, little-endian f64 bits;
/// * `Text`    — u32 little-endian byte length, then that many UTF-8 bytes.
///
/// Values are written unaligned; readers must use byte copies (e.g.
/// `i64::from_le_bytes`), never pointer casts.
pub mod tuple {
    use super::*;

    /// Serialize `values` (which must match `schema` in arity and types;
    /// `Value::Null` allowed only for nullable columns).
    pub fn serialize(schema: &Schema, values: &[Value]) -> Result<Vec<u8>> {
        todo!("Milestone 1")
    }

    /// Inverse of [`serialize`].
    pub fn deserialize(schema: &Schema, bytes: &[u8]) -> Result<Vec<Value>> {
        todo!("Milestone 1")
    }
}

/// Page-granular file I/O. One `DiskManager` owns one database file; page
/// `id` lives at byte offset `id * PAGE_SIZE`.
///
/// All methods take `&self`: the disk manager is shared behind an `Arc` and
/// must synchronize internally (positioned reads/writes or an internal
/// lock — your choice; see the milestone guidance).
pub struct DiskManager {/* file handle, page counter — your fields here */}

impl DiskManager {
    /// Open (or create) the database file at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        todo!("Milestone 1")
    }

    /// Read page `id` into `out`. Reading a page beyond the end of the file
    /// is an error ([`ferricdb_common::Error::InvalidPage`]).
    pub fn read_page(&self, id: PageId, out: &mut Page) -> Result<()> {
        todo!("Milestone 1")
    }

    /// Write `page` at page offset `id`.
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
    /// Run `f` with read access to page `id`.
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

    /// Force everything durable (no-op for the raw disk manager, which
    /// writes through; the buffer pool flushes dirty frames).
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
    /// Allocate the first page of a new heap and format it.
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
    /// when full) and return its address.
    pub fn insert(&self, meta: TupleMeta, tuple: &[u8]) -> Result<RecordId> {
        todo!("Milestone 1")
    }

    /// Read the tuple at `rid`. `Ok(None)` if the slot is tombstoned.
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

    /// Iterate every live tuple in the heap, in (page, slot) order.
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
