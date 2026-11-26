//! # ferricdb-index — disk-resident B+Tree (Milestone 5)
//!
//! In this crate you build FerricDB's ordered index: a **B+Tree** that maps
//! a single-column key ([`Value`]) to one or more tuple addresses
//! ([`RecordId`]), with every tree node stored in exactly one 8 KB page
//! reached through the [`PageStore`] abstraction (the buffer pool in
//! practice, Milestone 2).
//!
//! # Composite `(key, rid)` entries — PostgreSQL style
//!
//! Real relational databases must support both non-unique secondary indexes
//! *and* MVCC updates (Milestone 8), where two live versions of the same row
//! temporarily share an indexed key while pointing at different `RecordId`s.
//! Following PostgreSQL (v12+ heap-TID tie-breaking), FerricDB's B+Tree
//! orders every entry by the **composite pair `(key, rid)`**:
//!
//! * Multiple distinct `RecordId`s may be stored under the same `key`.
//! * Inserting the exact same `(key, rid)` pair twice is rejected with
//!   [`Error::Execution`](ferricdb_common::Error::Execution).
//! * [`BPlusTree::get`] returns all matching `RecordId`s in ascending
//!   `RecordId` order (`Vec<RecordId>`).
//! * [`BPlusTree::delete`] deletes a specific `(key, rid)` pair so MVCC
//!   `VACUUM` can remove a dead tuple version without disturbing a newer
//!   version that shares the same key.
//!
//! What you implement here (all of it in **Milestone 5**):
//!
//! - **Node layout**: leaf and internal node formats inside a
//!   `ferricdb_storage::Page`. Remember the first 16 bytes of every page are
//!   reserved (`pageLSN` + `page_type` + `reserved`) — use
//!   `PAGE_TYPE_BTREE_LEAF` (2) and `PAGE_TYPE_BTREE_INTERNAL` (3) to
//!   distinguish leaf from internal nodes.
//! - **Point operations**: [`BPlusTree::insert`] (with node splits all the
//!   way up to a root split), [`BPlusTree::get`], and [`BPlusTree::delete`]
//!   (lazy deletion — removing the entry from its leaf without rebalancing —
//!   is acceptable).
//! - **Range scans**: [`BPlusTree::range`] returns a [`RangeIter`] that
//!   finds the starting leaf, then walks the leaf sibling chain until the
//!   upper bound is passed.
//! - **Self-checking**: [`BPlusTree::check_invariants`] — the test hook the
//!   provided fuzz test leans on.
//!
//! Run the provided tests as you go:
//!
//! ```text
//! cargo test -p ferricdb-index -- --include-ignored ms5_
//! ```

// Remove this once you start implementing.
#![allow(unused_variables, dead_code)]

use std::ops::Bound;
use std::sync::Arc;

use ferricdb_common::{DataType, PageId, RecordId, Result, Value};
use ferricdb_storage::PageStore;

/// Disk-resident B+Tree mapping a single-column key to [`RecordId`]s,
/// ordered by composite `(key, rid)`. One node = one page.
///
/// # Key contract
///
/// - Entries are unique on **`(key, rid)`**, allowing duplicate `key` values
///   with distinct `RecordId`s (required for non-unique secondary indexes
///   and Milestone 8 MVCC out-of-place updates).
/// - Keys must be **non-NULL**, **non-NaN** (for `DataType::Float`), and
///   match the tree's declared [`DataType`]. If handed [`Value::Null`],
///   `Value::Float(f)` where `f.is_nan()`, or a mismatched type, methods
///   return [`Error::Execution`](ferricdb_common::Error::Execution) rather
///   than panicking.
/// - With `Null` and `NaN` rejected, [`Value::sql_cmp`] is a strict total
///   order on keys, and `(key, rid)` is a strict total order on entries
///   (`RecordId` implements `Ord`).
///
/// # Persistence
///
/// The tree owns no file of its own: all nodes live in pages of the shared
/// [`PageStore`]. A tree is identified on disk purely by its root page id —
/// the catalog stores it in `IndexInfo::root_page` (Milestone 5) and the
/// tree is reattached with [`BPlusTree::open`]. Because a root split
/// changes the root page id, callers must re-read [`BPlusTree::root_page`]
/// after mutations and persist it (`Catalog::set_index_root`).
pub struct BPlusTree {
    /// Page store all tree nodes are read and written through.
    store: Arc<dyn PageStore>,
    /// Page id of the current root node. Changes when the root splits.
    root: PageId,
    /// Declared type of the indexed key column.
    key_type: DataType,
}

impl BPlusTree {
    /// Creates a new, empty B+Tree: allocates one page from `store`,
    /// formats it as an empty leaf (`PAGE_TYPE_BTREE_LEAF`), and makes it
    /// the root.
    ///
    /// Implemented in Milestone 5.
    pub fn create(store: Arc<dyn PageStore>, key_type: DataType) -> Result<Self> {
        todo!("Milestone 5")
    }

    /// Reattaches an existing B+Tree whose root node lives at page `root`
    /// (as recorded in the catalog). Performs no I/O by itself; the root is
    /// read lazily on first use.
    pub fn open(store: Arc<dyn PageStore>, root: PageId, key_type: DataType) -> Self {
        BPlusTree {
            store,
            root,
            key_type,
        }
    }

    /// Page id of the current root node.
    ///
    /// This is the tree's persistent identity: store it (via the catalog)
    /// and pass it back to [`BPlusTree::open`] to reattach the tree later.
    /// It changes whenever [`BPlusTree::insert`] splits the root.
    pub fn root_page(&self) -> PageId {
        self.root
    }

    /// Inserts `(key, rid)`, splitting leaves/internal nodes (and possibly
    /// the root) as needed to make room.
    ///
    /// # Duplicate entries & invalid keys
    ///
    /// - Inserting the same `key` with a **different** `rid` succeeds (both
    ///   entries live in ascending `(key, rid)` order).
    /// - Inserting an exact `(key, rid)` pair that is already present returns
    ///   `Err(`[`Error::Execution`](ferricdb_common::Error::Execution)`)`
    ///   and leaves the tree unchanged.
    /// - Passing `Value::Null`, `Value::Float(NaN)`, or a `Value` whose
    ///   type does not match `key_type` returns `Err(Error::Execution)`.
    ///
    /// Implemented in Milestone 5.
    pub fn insert(&mut self, key: &Value, rid: RecordId) -> Result<()> {
        todo!("Milestone 5")
    }

    /// Point lookup: all [`RecordId`]s stored under `key`, in ascending
    /// `RecordId` order (empty `Vec` if `key` is not present).
    ///
    /// Implemented in Milestone 5.
    pub fn get(&self, key: &Value) -> Result<Vec<RecordId>> {
        todo!("Milestone 5")
    }

    /// Removes the exact `(key, rid)` entry from the tree. Returns
    /// `Ok(true)` if the entry was present (and is now gone), `Ok(false)`
    /// if it was not present.
    ///
    /// Lazy deletion is acceptable: remove the entry from its leaf without
    /// merging or redistributing underfull nodes.
    ///
    /// Implemented in Milestone 5.
    pub fn delete(&mut self, key: &Value, rid: RecordId) -> Result<bool> {
        todo!("Milestone 5")
    }

    /// Ordered scan of every `(key, rid)` pair whose `key` falls between
    /// `lower` and `upper` per the given [`Bound`]s (any combination of
    /// `Included` / `Excluded` / `Unbounded` on either side).
    ///
    /// Descends once to the first qualifying leaf, then the returned
    /// [`RangeIter`] walks the leaf sibling chain, yielding pairs in
    /// ascending `(key, rid)` order and stopping as soon as a key exceeds
    /// `upper`. An empty or inverted range (e.g. `lower > upper`) yields no
    /// items — it is not an error.
    ///
    /// Implemented in Milestone 5.
    pub fn range(&self, lower: Bound<Value>, upper: Bound<Value>) -> Result<RangeIter> {
        todo!("Milestone 5")
    }

    /// Test hook: verifies the structural invariants of the tree and
    /// returns `Err` (`Error::Execution` with a descriptive message) on the
    /// first violation found.
    ///
    /// Check at least:
    ///
    /// - **Sortedness**: `(key, rid)` pairs strictly ascending within every
    ///   node, and across the whole leaf chain (no duplicate `(key, rid)`).
    /// - **Balance**: every leaf is at the same depth.
    /// - **Key ranges**: each child's `(key, rid)` entries fall within the
    ///   bounds implied by the separator entries of its parent.
    /// - **Leaf chain**: following sibling links visits exactly the leaves
    ///   reachable from the root, in order.
    ///
    /// Minimum-occupancy checks are optional because deletion is lazy.
    /// The provided `ms5_btree_invariants_fuzz` test calls this after every
    /// 100 random operations — an early, precise failure here saves hours.
    ///
    /// Implemented in Milestone 5.
    pub fn check_invariants(&self) -> Result<()> {
        todo!("Milestone 5")
    }
}

/// Ordered iterator over `(key, rid)` pairs produced by
/// [`BPlusTree::range`]. Walks the leaf sibling chain; yields items in
/// ascending `(key, rid)` order and ends once the upper bound is passed.
///
/// Page-read failures surface as `Some(Err(_))`; after the first error the
/// iterator should fuse (keep returning `None`).
///
/// The fields below are a suggested shape — restructure freely; only the
/// public API (`Iterator<Item = Result<(Value, RecordId)>>`) is fixed.
pub struct RangeIter {
    /// Page store leaf pages are read through.
    store: Arc<dyn PageStore>,
    /// Key type, needed to decode keys out of leaf pages.
    key_type: DataType,
    /// Leaf page currently being walked
    /// (`ferricdb_common::INVALID_PAGE_ID` once exhausted).
    current_page: PageId,
    /// Next entry index to yield within `current_page`.
    current_slot: u16,
    /// Upper bound; iteration stops when a key passes it.
    upper: Bound<Value>,
}

impl Iterator for RangeIter {
    type Item = Result<(Value, RecordId)>;

    /// Yields the next `(key, rid)` pair in ascending `(key, rid)` order,
    /// stepping to the next sibling leaf when the current one is exhausted
    /// (skipping over any empty leaves produced by lazy deletion).
    ///
    /// Implemented in Milestone 5.
    fn next(&mut self) -> Option<Self::Item> {
        todo!("Milestone 5")
    }
}
