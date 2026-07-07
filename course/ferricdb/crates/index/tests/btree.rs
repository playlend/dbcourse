//! Provided Milestone 5 test suite.
//!
//! Run with `cargo test -p ferricdb-index -- --include-ignored ms5_`.
//! `check_invariants` is your best friend: the fuzz test calls it every
//! hundred operations, so a split bug is caught within moments of the
//! insert that planted it.

use std::collections::BTreeMap;
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
    assert_eq!(tree.get(&key(10)).unwrap(), Some(rid(1)));
    assert_eq!(tree.get(&key(20)).unwrap(), Some(rid(2)));
    assert_eq!(tree.get(&key(15)).unwrap(), None);
    tree.check_invariants().unwrap();
}

#[test]
#[ignore = "milestone 5"]
fn ms5_duplicate_insert_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();
    tree.insert(&key(7), rid(1)).unwrap();
    assert!(
        tree.insert(&key(7), rid(2)).is_err(),
        "course B+Trees hold unique keys — a duplicate insert must error"
    );
    assert_eq!(
        tree.get(&key(7)).unwrap(),
        Some(rid(1)),
        "original mapping intact"
    );
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
        assert_eq!(tree.get(&key(k)).unwrap(), Some(rid(k as u64)));
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
        assert_eq!(tree.get(&key(k)).unwrap(), Some(rid(k as u64)));
    }
}

/// REQUIRED NAME (api-spec): seeded interleaved insert/delete fuzz against
/// a BTreeMap reference model, invariants checked continuously.
#[test]
#[ignore = "milestone 5"]
fn ms5_btree_invariants_fuzz() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();
    let mut model: BTreeMap<i64, RecordId> = BTreeMap::new();
    let mut rng = StdRng::seed_from_u64(0xB7EE_FA22);

    for op in 0..2000 {
        let k = rng.random_range(0..1000i64);
        if rng.random_bool(0.65) {
            let r = rid(op as u64);
            match model.entry(k) {
                std::collections::btree_map::Entry::Vacant(e) => {
                    tree.insert(&key(k), r).unwrap();
                    e.insert(r);
                }
                std::collections::btree_map::Entry::Occupied(_) => {
                    assert!(tree.insert(&key(k), r).is_err(), "duplicate {k}");
                }
            }
        } else {
            let existed = model.remove(&k).is_some();
            assert_eq!(
                tree.delete(&key(k)).unwrap(),
                existed,
                "delete({k}) presence must match the model"
            );
        }
        if op % 100 == 0 {
            tree.check_invariants().unwrap();
        }
    }
    tree.check_invariants().unwrap();

    // final contents equal the model, both by point reads and full range
    for (k, r) in &model {
        assert_eq!(tree.get(&key(*k)).unwrap(), Some(*r));
    }
    let all: Vec<(Value, RecordId)> = tree
        .range(Bound::Unbounded, Bound::Unbounded)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let expected: Vec<(Value, RecordId)> = model.iter().map(|(k, r)| (key(*k), *r)).collect();
    assert_eq!(
        all, expected,
        "full range scan equals the reference model, in order"
    );
}

#[test]
#[ignore = "milestone 5"]
fn ms5_range_bounds_combinations() {
    let dir = tempfile::tempdir().unwrap();
    let mut tree = BPlusTree::create(store(&dir), DataType::Integer).unwrap();
    for k in (0..100).map(|i| i * 10) {
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
        collect(Bound::Included(key(970)), Bound::Unbounded),
        vec![970, 980, 990]
    );
    // bounds between keys, and an empty range
    assert_eq!(
        collect(Bound::Included(key(15)), Bound::Included(key(35))),
        vec![20, 30]
    );
    assert!(collect(Bound::Included(key(11)), Bound::Excluded(key(19))).is_empty());
}

#[test]
#[ignore = "milestone 5"]
fn ms5_reopen_by_root_page() {
    let dir = tempfile::tempdir().unwrap();
    let s = store(&dir);
    let root;
    {
        let mut tree = BPlusTree::create(Arc::clone(&s), DataType::Integer).unwrap();
        for k in 0..500 {
            tree.insert(&key(k), rid(k as u64)).unwrap();
        }
        root = tree.root_page();
        s.flush_all().unwrap();
    }
    let tree = BPlusTree::open(Arc::clone(&s), root, DataType::Integer);
    tree.check_invariants().unwrap();
    for k in (0..500).step_by(23) {
        assert_eq!(tree.get(&key(k)).unwrap(), Some(rid(k as u64)));
    }
}
