//! Provided Milestone 1 test suite.
//!
//! Every test is `#[ignore = "milestone 1"]`; run the group with
//! `cargo test -p ferricdb-storage -- --include-ignored ms1_`.
//! The tests compile against the stub API from day one, so signature drift
//! is caught even before you start implementing.

use std::sync::Arc;

use ferricdb_common::{Column, DataType, RecordId, Schema, TupleMeta, Value, PAGE_SIZE};
use ferricdb_storage::{tuple, DiskManager, HeapFile, HeapPage, Page, PageStore};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn test_schema() -> Schema {
    Schema {
        columns: vec![
            Column {
                name: "id".into(),
                data_type: DataType::Integer,
                nullable: false,
            },
            Column {
                name: "flag".into(),
                data_type: DataType::Boolean,
                nullable: true,
            },
            Column {
                name: "score".into(),
                data_type: DataType::Float,
                nullable: true,
            },
            Column {
                name: "name".into(),
                data_type: DataType::Text,
                nullable: true,
            },
        ],
    }
}

fn sample_row(i: i64) -> Vec<Value> {
    vec![
        Value::Integer(i),
        if i % 3 == 0 {
            Value::Null
        } else {
            Value::Boolean(i % 2 == 0)
        },
        if i % 5 == 0 {
            Value::Null
        } else {
            Value::Float(i as f64 * 1.5)
        },
        if i % 7 == 0 {
            Value::Null
        } else {
            Value::Text(format!("row-{i}"))
        },
    ]
}

// ---------------------------------------------------------------- pages

#[test]
#[ignore = "milestone 1"]
fn ms1_page_lsn_roundtrip() {
    let mut page = Page::new();
    assert_eq!(page.lsn(), 0, "a fresh page has pageLSN 0");
    page.set_lsn(0xDEAD_BEEF);
    assert_eq!(page.lsn(), 0xDEAD_BEEF);
    // the LSN lives inside the 16-byte reserved header
    assert_ne!(&page.data()[0..8], &[0u8; 8]);
}

/// REQUIRED NAME (api-spec): insert tuples into a HeapPage, read them back
/// with their metadata intact.
#[test]
#[ignore = "milestone 1"]
fn ms1_slotted_page_roundtrip() {
    let mut page = Page::new();
    let mut heap = HeapPage::init(&mut page);

    let meta_a = TupleMeta { xmin: 7, xmax: 0 };
    let meta_b = TupleMeta { xmin: 8, xmax: 3 };
    let slot_a = heap.insert(meta_a, b"alpha").expect("page has room");
    let slot_b = heap.insert(meta_b, b"bravo-bravo").expect("page has room");
    assert_ne!(slot_a, slot_b);
    assert_eq!(heap.slot_count(), 2);

    let (ma, ta) = heap.get(slot_a).expect("slot A live");
    assert_eq!(ma, meta_a);
    assert_eq!(ta, b"alpha");
    let (mb, tb) = heap.get(slot_b).expect("slot B live");
    assert_eq!(mb, meta_b);
    assert_eq!(tb, b"bravo-bravo");
}

#[test]
#[ignore = "milestone 1"]
fn ms1_tombstone_semantics() {
    let mut page = Page::new();
    let mut heap = HeapPage::init(&mut page);
    let s0 = heap.insert(TupleMeta::default(), b"first").unwrap();
    let s1 = heap.insert(TupleMeta::default(), b"second").unwrap();

    heap.delete(s0).expect("delete live slot");
    assert!(heap.get(s0).is_none(), "tombstoned slot reads as absent");
    assert!(heap.get(s1).is_some(), "other slots unaffected");
    assert_eq!(heap.slot_count(), 2, "tombstones keep their line pointer");
}

#[test]
#[ignore = "milestone 1"]
fn ms1_set_meta_updates_in_place() {
    let mut page = Page::new();
    let mut heap = HeapPage::init(&mut page);
    let slot = heap
        .insert(TupleMeta { xmin: 1, xmax: 0 }, b"victim")
        .unwrap();

    heap.set_meta(slot, TupleMeta { xmin: 1, xmax: 42 })
        .expect("stamp xmax");
    let (meta, bytes) = heap
        .get(slot)
        .expect("still live (MVCC decides visibility)");
    assert_eq!(meta.xmax, 42);
    assert_eq!(bytes, b"victim", "tuple bytes untouched by set_meta");
}

#[test]
#[ignore = "milestone 1"]
fn ms1_free_space_decreases_and_bounds_inserts() {
    let mut page = Page::new();
    let mut heap = HeapPage::init(&mut page);
    let before = heap.free_space();
    assert!(before < PAGE_SIZE, "headers cost something");

    heap.insert(TupleMeta::default(), &[0u8; 100]).unwrap();
    let after = heap.free_space();
    assert!(after < before, "free space shrinks on insert");

    // fill until the page refuses, then verify it keeps refusing
    let mut inserted = 1;
    while heap.insert(TupleMeta::default(), &[0u8; 100]).is_some() {
        inserted += 1;
        assert!(inserted < PAGE_SIZE / 50, "page must fill eventually");
    }
    assert!(
        heap.insert(TupleMeta::default(), &[0u8; 4000]).is_none(),
        "a full page refuses further inserts"
    );
    // attach() sees the same state init() built
    let count = heap.slot_count();
    let reheap = HeapPage::attach(&mut page);
    assert_eq!(reheap.slot_count(), count);
}

// ---------------------------------------------------------------- tuples

#[test]
#[ignore = "milestone 1"]
fn ms1_tuple_serde_all_types_and_nulls() {
    let schema = test_schema();
    let cases: Vec<Vec<Value>> = vec![
        vec![
            Value::Integer(1),
            Value::Boolean(true),
            Value::Float(2.5),
            Value::Text("x".into()),
        ],
        vec![Value::Integer(-9), Value::Null, Value::Null, Value::Null],
        vec![
            Value::Integer(0),
            Value::Boolean(false),
            Value::Float(-0.0),
            Value::Text(String::new()),
        ],
        vec![
            Value::Integer(i64::MAX),
            Value::Null,
            Value::Float(f64::MIN_POSITIVE),
            Value::Text("héllo → 世界".into()),
        ],
    ];
    for row in cases {
        let bytes = tuple::serialize(&schema, &row).expect("serialize");
        let back = tuple::deserialize(&schema, &bytes).expect("deserialize");
        assert_eq!(back, row);
    }
}

#[test]
#[ignore = "milestone 1"]
fn ms1_tuple_serde_property_seeded() {
    let schema = test_schema();
    let mut rng = StdRng::seed_from_u64(0xFE44_1CDB);
    for _ in 0..200 {
        let row = vec![
            Value::Integer(rng.random::<i64>()),
            if rng.random_bool(0.2) {
                Value::Null
            } else {
                Value::Boolean(rng.random_bool(0.5))
            },
            if rng.random_bool(0.2) {
                Value::Null
            } else {
                Value::Float(rng.random::<f64>())
            },
            if rng.random_bool(0.2) {
                Value::Null
            } else {
                let len = rng.random_range(0..200);
                Value::Text((0..len).map(|_| rng.random_range('a'..='z')).collect())
            },
        ];
        let bytes = tuple::serialize(&schema, &row).expect("serialize");
        let back = tuple::deserialize(&schema, &bytes).expect("deserialize");
        assert_eq!(back, row, "round-trip must be lossless");
    }
}

// ---------------------------------------------------------------- disk

#[test]
#[ignore = "milestone 1"]
fn ms1_disk_manager_persistence_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");

    let id;
    {
        let disk = DiskManager::open(&path).expect("create");
        id = disk.allocate_page().expect("allocate");
        let mut page = Page::new();
        page.data_mut()[100] = 0xAB;
        page.data_mut()[PAGE_SIZE - 1] = 0xCD;
        disk.write_page(id, &page).expect("write");
        assert_eq!(disk.num_pages(), id + 1);
    }
    {
        let disk = DiskManager::open(&path).expect("reopen");
        assert_eq!(disk.num_pages(), id + 1, "page count survives reopen");
        let mut out = Page::new();
        disk.read_page(id, &mut out).expect("read");
        assert_eq!(out.data()[100], 0xAB);
        assert_eq!(out.data()[PAGE_SIZE - 1], 0xCD);
    }
}

#[test]
#[ignore = "milestone 1"]
fn ms1_disk_manager_rejects_out_of_range_reads() {
    let dir = tempfile::tempdir().unwrap();
    let disk = DiskManager::open(&dir.path().join("t.db")).unwrap();
    let mut out = Page::new();
    assert!(
        disk.read_page(9999, &mut out).is_err(),
        "unallocated page id must error"
    );
}

// ---------------------------------------------------------------- heap file

#[test]
#[ignore = "milestone 1"]
fn ms1_heapfile_roundtrip_and_scan() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let store: Arc<dyn PageStore> = Arc::new(DiskManager::open(&path).unwrap());
    let schema = test_schema();

    let heap = HeapFile::create(Arc::clone(&store)).expect("create heap");
    let mut rids: Vec<RecordId> = Vec::new();
    for i in 0..500 {
        let bytes = tuple::serialize(&schema, &sample_row(i)).unwrap();
        rids.push(heap.insert(TupleMeta { xmin: 1, xmax: 0 }, &bytes).unwrap());
    }
    assert!(store.num_pages() > 1, "500 rows must span multiple pages");

    // point reads
    for (i, rid) in rids.iter().enumerate() {
        let (_meta, bytes) = heap.get(*rid).unwrap().expect("live tuple");
        assert_eq!(
            tuple::deserialize(&schema, &bytes).unwrap(),
            sample_row(i as i64)
        );
    }

    // delete every 10th, then scan sees exactly the survivors, in order
    for rid in rids.iter().step_by(10) {
        heap.delete(*rid).unwrap();
    }
    let scanned: Vec<_> = heap.scan().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(scanned.len(), 500 - 50);
    let expected: Vec<i64> = (0..500).filter(|i| i % 10 != 0).collect();
    let got: Vec<i64> = scanned
        .iter()
        .map(
            |(_, _, b)| match tuple::deserialize(&schema, b).unwrap()[0] {
                Value::Integer(v) => v,
                _ => panic!("first column is the id"),
            },
        )
        .collect();
    assert_eq!(
        got, expected,
        "scan yields live tuples in (page, slot) order"
    );
}

#[test]
#[ignore = "milestone 1"]
fn ms1_heapfile_survives_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let schema = test_schema();

    let first_page;
    {
        let store: Arc<dyn PageStore> = Arc::new(DiskManager::open(&path).unwrap());
        let heap = HeapFile::create(Arc::clone(&store)).unwrap();
        first_page = heap.first_page();
        for i in 0..50 {
            let bytes = tuple::serialize(&schema, &sample_row(i)).unwrap();
            heap.insert(TupleMeta::default(), &bytes).unwrap();
        }
        store.flush_all().unwrap();
    }
    {
        let store: Arc<dyn PageStore> = Arc::new(DiskManager::open(&path).unwrap());
        let heap = HeapFile::open(store, first_page);
        let n = heap.scan().count();
        assert_eq!(n, 50, "all tuples visible after close/reopen");
    }
}
