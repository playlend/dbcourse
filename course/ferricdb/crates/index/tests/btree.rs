//! Provided Milestone 5 test suite.
//!
//! Run with `cargo test -p ferricdb-index -- --include-ignored ms5_`.
//! `check_invariants` is your best friend: the fuzz test calls it every
//! hundred operations, so a split bug is caught within moments of the
//! insert that planted it.

use std::collections::BTreeSet;
use std::ops::Bound;
use std::sync::Arc;

use ferricdb_common::{DataType, RecordId, Value};
use ferricdb_index::BPlusTree;
use ferricdb_storage::{DiskManager, PageStore};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn store(dir: &tempfile::TempDir) -> Arc<dyn PageStore> {
    Arc::new(DiskManager::open(&dir.path().join("t.db")).unwrap())
}

fn rid(n: u64) -> RecordId {
    RecordId {
        page_id: (n / 1000) as u32,
        slot: (n % 1000) as u16,
    }
}

fn key(k: i64) -> Value {
    Value::Integer(k)
}

#[test]
#[ignore = "milestone 5"]
fn ms5_point_get_hit_and_miss() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();

    tree.insert(&key(10), rid(1)).unwrap();
    tree.insert(&key(20), rid(2)).unwrap();
    assert_eq!(tree.get(&key(10)).unwrap(), vec![rid(1)]);
    assert_eq!(tree.get(&key(20)).unwrap(), vec![rid(2)]);
    assert!(tree.get(&key(15)).unwrap().is_empty());
    tree.check_invariants().unwrap();
}

#[test]
#[ignore = "milestone 5"]
fn ms5_non_unique_keys_and_exact_duplicate_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();

    // Non-unique secondary index & MVCC multi-version updates store multiple
    // distinct RecordIds under the same key, ordered by RecordId.
    tree.insert(&key(7), rid(20)).unwrap();
    tree.insert(&key(7), rid(5)).unwrap();
    tree.insert(&key(7), rid(12)).unwrap();
    assert_eq!(tree.get(&key(7)).unwrap(), vec![rid(5), rid(12), rid(20)]);

    // Exact (key, rid) duplicate must error without disturbing existing entries.
    assert!(
        tree.insert(&key(7), rid(12)).is_err(),
        "exact duplicate (key, rid) must error"
    );
    assert_eq!(tree.get(&key(7)).unwrap(), vec![rid(5), rid(12), rid(20)]);

    // Deleting one (key, rid) removes only that version (what VACUUM needs).
    assert!(tree.delete(&key(7), rid(12)).unwrap());
    assert!(!tree.delete(&key(7), rid(12)).unwrap());
    assert_eq!(tree.get(&key(7)).unwrap(), vec![rid(5), rid(20)]);
    tree.check_invariants().unwrap();
}

#[test]
#[ignore = "milestone 5"]
fn ms5_sequential_ascending_inserts_split_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();
    for k in 0..2000 {
        tree.insert(&key(k), rid(k as u64)).unwrap();
        if k % 100 == 0 {
            tree.check_invariants().unwrap();
        }
    }
    tree.check_invariants().unwrap();
    for k in (0..2000).step_by(37) {
        assert_eq!(tree.get(&key(k)).unwrap(), vec![rid(k as u64)]);
    }
}

#[test]
#[ignore = "milestone 5"]
fn ms5_sequential_descending_inserts_split_correctly() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();
    for k in (0..2000).rev() {
        tree.insert(&key(k), rid(k as u64)).unwrap();
        if k % 100 == 0 {
            tree.check_invariants().unwrap();
        }
    }
    tree.check_invariants().unwrap();
    for k in (0..2000).step_by(41) {
        assert_eq!(tree.get(&key(k)).unwrap(), vec![rid(k as u64)]);
    }
}

/// REQUIRED NAME (api-spec): seeded interleaved insert/delete fuzz against
/// a BTreeSet<(i64, RecordId)> reference model, invariants checked continuously.
#[test]
#[ignore = "milestone 5"]
fn ms5_btree_invariants_fuzz() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();
    let mut model: BTreeSet<(i64, RecordId)> = BTreeSet::new();
    let mut rng = StdRng::seed_from_u64(0xB7EE_FA22);

    for op in 0..2500 {
        let k = rng.random_range(0..800i64);
        if rng.random_bool(0.65) {
            let r = rid(rng.random_range(0..4000u64));
            if model.insert((k, r)) {
                tree.insert(&key(k), r).unwrap();
            } else {
                assert!(tree.insert(&key(k), r).is_err(), "duplicate ({k}, {r:?})");
            }
        } else if !model.is_empty() {
            // Delete either an existing entry or a miss.
            let target = if rng.random_bool(0.7) {
                let idx = rng.random_range(0..model.len());
                *model.iter().nth(idx).unwrap()
            } else {
                (k, rid(99_999))
            };
            let existed = model.remove(&target);
            assert_eq!(
                tree.delete(&key(target.0), target.1).unwrap(),
                existed,
                "delete({target:?}) presence must match the model"
            );
        }
        if op % 100 == 0 {
            tree.check_invariants().unwrap();
        }
    }
    tree.check_invariants().unwrap();

    // final contents equal the model, both by point reads and full range
    let all: Vec<(Value, RecordId)> = tree
        .range(Bound::Unbounded, Bound::Unbounded)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected: Vec<(Value, RecordId)> = model.iter().map(|(k, r)| (key(*k), *r)).collect();
    assert_eq!(
        all, expected,
        "full range scan equals the reference model, in (key, rid) order"
    );
}

/// Variable-length TEXT keys force both leaf splits AND height >= 3 internal
/// node splits (159-byte keys fit ~47 per 8 KB leaf -> 4000 shuffled keys
/// require ~120 leaves, well above the ~48 children a single internal root can
/// hold -> internal root splits -> height 3).
#[test]
#[ignore = "milestone 5"]
fn ms5_text_keys_height_three_internal_splits() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Text).unwrap();
    let mut rng = StdRng::seed_from_u64(0x7E87_B7EE);
    let mut keys: Vec<String> = (0..4000)
        .map(|i| format!("key-{i:06}-{}", "x".repeat(148)))
        .collect();
    // Deterministic Fisher-Yates shuffle so splits hit internal nodes in
    // non-monotonic positions.
    for i in (1..keys.len()).rev() {
        let j = rng.random_range(0..=i);
        keys.swap(i, j);
    }

    for (idx, s) in keys.iter().enumerate() {
        tree.insert(&Value::Text(s.clone()), rid(idx as u64))
            .unwrap();
        if idx % 400 == 0 {
            tree.check_invariants().unwrap();
        }
    }
    tree.check_invariants().unwrap();

    // Multi-leaf range scan across sorted string keys.
    let lo = Value::Text(format!("key-{:06}-{}", 1000, "x".repeat(148)));
    let hi = Value::Text(format!("key-{:06}-{}", 1500, "x".repeat(148)));
    let range: Vec<String> = tree
        .range(Bound::Included(lo), Bound::Excluded(hi))
        .unwrap()
        .map(|r| match r.unwrap().0 {
            Value::Text(s) => s,
            other => panic!("text keys only, got {other:?}"),
        })
        .collect();
    assert_eq!(range.len(), 500, "range spans multiple leaves cleanly");
    assert!(range.windows(2).all(|w| w[0] < w[1]), "strictly sorted");
}

#[test]
#[ignore = "milestone 5"]
fn ms5_float_boolean_and_invalid_keys() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);

    let mut ftree = BPlusTree::create(Arc::clone(&s), DataType::Float).unwrap();
    ftree.insert(&Value::Float(1.5), rid(1)).unwrap();
    ftree.insert(&Value::Float(-2.25), rid(2)).unwrap();
    assert_eq!(ftree.get(&Value::Float(-2.25)).unwrap(), vec![rid(2)]);
    assert!(
        ftree.insert(&Value::Float(f64::NAN), rid(3)).is_err(),
        "NaN keys have no SQL order and must be rejected"
    );
    assert!(
        ftree.insert(&Value::Null, rid(4)).is_err(),
        "NULL keys must be rejected"
    );
    assert!(
        ftree.insert(&Value::Integer(1), rid(5)).is_err(),
        "wrong DataType must be rejected"
    );
    ftree.check_invariants().unwrap();

    let mut btree = BPlusTree::create(s, DataType::Boolean).unwrap();
    btree.insert(&Value::Boolean(true), rid(10)).unwrap();
    btree.insert(&Value::Boolean(false), rid(11)).unwrap();
    btree.insert(&Value::Boolean(false), rid(12)).unwrap();
    assert_eq!(
        btree.get(&Value::Boolean(false)).unwrap(),
        vec![rid(11), rid(12)]
    );
    btree.check_invariants().unwrap();
}

#[test]
#[ignore = "milestone 5"]
fn ms5_range_bounds_combinations() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();
    // 1,200 keys span multiple leaves, testing leaf-sibling-chain walking.
    for k in (0..1200).map(|i| i * 10) {
        tree.insert(&key(k), rid(k as u64)).unwrap();
    }

    let collect = |lo: Bound<Value>, hi: Bound<Value>| -> Vec<i64> {
        tree.range(lo, hi)
            .unwrap()
            .map(|r| match r.unwrap().0 {
                Value::Integer(k) => k,
                other => panic!("integer keys only, got {other:?}"),
            })
            .collect()
    };

    assert_eq!(
        collect(Bound::Included(key(200)), Bound::Excluded(key(240))),
        vec![200, 210, 220, 230]
    );
    assert_eq!(
        collect(Bound::Excluded(key(200)), Bound::Included(key(240))),
        vec![210, 220, 230, 240]
    );
    assert_eq!(
        collect(Bound::Unbounded, Bound::Excluded(key(30))),
        vec![0, 10, 20]
    );
    assert_eq!(
        collect(Bound::Included(key(11970)), Bound::Unbounded),
        vec![11970, 11980, 11990]
    );
    // cross-leaf slice (500 keys)
    let big = collect(Bound::Included(key(2000)), Bound::Excluded(key(7000)));
    assert_eq!(big.len(), 500);
    assert_eq!(big[0], 2000);
    assert_eq!(*big.last().unwrap(), 6990);
    // bounds between keys, empty range, and inverted range
    assert_eq!(
        collect(Bound::Included(key(15)), Bound::Included(key(35))),
        vec![20, 30]
    );
    assert!(collect(Bound::Included(key(11)), Bound::Excluded(key(19))).is_empty());
    assert!(collect(Bound::Included(key(500)), Bound::Included(key(100))).is_empty());
}

#[test]
#[ignore = "milestone 5"]
fn ms5_reopen_by_root_page() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let root;
    {
        let mut tree = BPlusTree::create(Arc::clone(&s), DataType::Integer).unwrap();
        // 1,500 keys guarantee root splits off the initial leaf page.
        for k in 0..1500 {
            tree.insert(&key(k), rid(k as u64)).unwrap();
        }
        root = tree.root_page();
        s.flush_all().unwrap();
    }
    let tree = BPlusTree::open(Arc::clone(&s), root, DataType::Integer);
    tree.check_invariants().unwrap();
    for k in (0..1500).step_by(23) {
        assert_eq!(tree.get(&key(k)).unwrap(), vec![rid(k as u64)]);
    }
}
