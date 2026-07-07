//! # ferricdb-index — disk-resident B+Tree (Milestone 5)
//!
//! In this crate you build FerricDB's ordered index: a **B+Tree** that maps
//! a single-column key ([`Value`]) to a tuple address ([`RecordId`]), with
//! every tree node stored in exactly one 8 KB page reached through the
//! [`PageStore`] abstraction (the buffer pool in practice, Milestone 2).
//!
//! What you implement here (all of it in **Milestone 5**):
//!
//! - **Node layout**: leaf and internal node formats inside a
//!   `ferricdb_storage::Page`. Remember the first 16 bytes of every page are
//!   reserved (pageLSN + page type tag + reserved) — use the type tag to
//!   distinguish leaf from internal nodes. The rest of the layout (header
//!   fields, key/value packing, sibling links) is yours to design.
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

/// Disk-resident B+Tree mapping a single-column key to [`RecordId`]s.
/// One node = one page. Keys are [`Value`]s of one [`DataType`].
///
/// # Key contract
///
/// - Keys are **unique**: FerricDB indexes are single-column unique
///   indexes. See [`BPlusTree::insert`] for the duplicate-key behavior.
/// - Keys are **non-NULL** values of the tree's declared key type. Callers
///   (the Milestone 5 executor code) guarantee this; if handed
///   [`Value::Null`] or a mismatched type, methods return
///   [`Error::Execution`](ferricdb_common::Error::Execution) rather than
///   panicking.
/// - Key ordering is [`Value::sql_cmp`] restricted to one non-NULL type,
///   which is a total order.
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
    /// formats it as an empty leaf, and makes it the root.
    ///
    /// Implemented in Milestone 5.
    pub fn create(store: Arc<dyn PageStore>, key_type: DataType) -> Result<Self> {
        todo!("Milestone 5")
    }

    /// Reattaches an existing B+Tree whose root node lives at page `root`
    /// (as recorded in the catalog). Performs no I/O by itself; the root is
    /// read lazily on first use.
    ///
    /// Implemented in Milestone 5.
    pub fn open(store: Arc<dyn PageStore>, root: PageId, key_type: DataType) -> Self {
        todo!("Milestone 5")
    }

    /// Page id of the current root node.
    ///
    /// This is the tree's persistent identity: store it (via the catalog)
    /// and pass it back to [`BPlusTree::open`] to reattach the tree later.
    /// It changes whenever [`BPlusTree::insert`] splits the root.
    ///
    /// Implemented in Milestone 5.
    pub fn root_page(&self) -> PageId {
        todo!("Milestone 5")
    }

    /// Inserts `key → rid`, splitting leaves/internal nodes (and possibly
    /// the root) as needed to make room.
    ///
    /// # Duplicate keys
    ///
    /// FerricDB indexes are single-column **unique** indexes: inserting a
    /// key that is already present does **not** overwrite the stored
    /// [`RecordId`]. It returns
    /// `Err(`[`Error::Execution`](ferricdb_common::Error::Execution)`)`
    /// (the message should name the duplicate key) and leaves the tree
    /// unchanged — the previously stored mapping still wins. The provided
    /// `ms5_duplicate_key_insert_errors` test pins this contract.
    ///
    /// Implemented in Milestone 5.
    pub fn insert(&mut self, key: &Value, rid: RecordId) -> Result<()> {
        todo!("Milestone 5")
    }

    /// Point lookup: the [`RecordId`] stored under `key`, or `Ok(None)` if
    /// the key is not present.
    ///
    /// Implemented in Milestone 5.
    pub fn get(&self, key: &Value) -> Result<Option<RecordId>> {
        todo!("Milestone 5")
    }

    /// Removes `key` from the tree. Returns `Ok(true)` if the key was
    /// present (and is now gone), `Ok(false)` if it was not present.
    ///
    /// Lazy deletion is acceptable: remove the entry from its leaf without
    /// merging or redistributing underfull nodes. After a successful
    /// delete, [`BPlusTree::get`] for that key returns `Ok(None)` and range
    /// scans skip it; a subsequent [`BPlusTree::insert`] of the same key
    /// succeeds again.
    ///
    /// Implemented in Milestone 5.
    pub fn delete(&mut self, key: &Value) -> Result<bool> {
        todo!("Milestone 5")
    }

    /// Ordered scan of every `(key, rid)` pair between `lower` and `upper`
    /// per the given [`Bound`]s (any combination of `Included` / `Excluded` /
    /// `Unbounded` on either side).
    ///
    /// Descends once to the first qualifying leaf, then the returned
    /// [`RangeIter`] walks the leaf sibling chain, yielding pairs in
    /// ascending key order and stopping as soon as a key exceeds `upper`.
    /// An empty or inverted range (e.g. `lower > upper`) yields no items —
    /// it is not an error.
    ///
    /// Implemented in Milestone 5.
    pub fn range(&self, lower: Bound<Value>, upper: Bound<Value>) -> Result<RangeIter> {
        todo!("Milestone 5")
    }

    /// Test hook: verifies the structural invariants of the tree and
    /// returns `Err` (any variant; `Error::Execution` with a descriptive
    /// message is conventional) on the first violation found.
    ///
    /// Check at least:
    ///
    /// - **Sortedness**: keys strictly ascending within every node, and
    ///   across the whole leaf chain (no duplicates anywhere).
    /// - **Balance**: every leaf is at the same depth.
    /// - **Key ranges**: each child's keys fall within the bounds implied
    ///   by the separator keys of its parent.
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
/// ascending key order and ends once the upper bound is passed.
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

    /// Yields the next `(key, rid)` pair in ascending key order, stepping
    /// to the next sibling leaf when the current one is exhausted.
    ///
    /// Implemented in Milestone 5.
    fn next(&mut self) -> Option<Self::Item> {
        todo!("Milestone 5")
    }
}
