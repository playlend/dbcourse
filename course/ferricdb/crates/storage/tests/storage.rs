//! Provided Milestone 1 test suite (plus MS8 compaction and MS9 put_at hooks).
//!
//! Every test is `#[ignore = "milestone N"]`; run Milestone 1 with:
//! `cargo test -p ferricdb-storage -- --include-ignored ms1_`.
//! The tests compile against the stub API from day one, so signature drift
//! is caught even before you start implementing.

use std::sync::Arc;

use ferricdb_common::{
    Column, DataType, Error, RecordId, Schema, TupleMeta, Value, INVALID_PAGE_ID, PAGE_SIZE,
};
use ferricdb_storage::{
    tuple, DiskManager, HeapFile, HeapPage, HeapPageRef, Page, PageStore, PAGE_TYPE_HEAP,
};
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
    assert_eq!(heap.next_page(), INVALID_PAGE_ID);

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

    // Read-only view sees the exact same header and tuples without &mut Page.
    assert!(HeapPageRef::is_initialized(&page));
    assert_eq!(page.page_type(), PAGE_TYPE_HEAP);
    let ro = HeapPageRef::attach(&page);
    assert_eq!(ro.slot_count(), 2);
    assert_eq!(ro.get(slot_a), Some((meta_a, &b"alpha"[..])));
    assert_eq!(ro.get(slot_b), Some((meta_b, &b"bravo-bravo"[..])));
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
    assert!(
        matches!(heap.delete(s0), Err(Error::InvalidSlot(_))),
        "double-deleting a tombstoned slot errors"
    );
    assert!(
        matches!(heap.delete(99), Err(Error::InvalidSlot(_))),
        "deleting an out-of-range slot errors"
    );
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
        // Check IEEE 754 bit identity on floats (preserves -0.0 sign bit).
        for (a, b) in back.iter().zip(&row) {
            if let (Value::Float(fa), Value::Float(fb)) = (a, b) {
                assert_eq!(fa.to_bits(), fb.to_bits(), "float bits must round-trip");
            }
        }
    }
}

#[test]
#[ignore = "milestone 1"]
fn ms1_tuple_serde_wide_schema_multi_byte_bitmap() {
    // 12 columns exercise a 2-byte null bitmap (ceil(12/8) == 2).
    let schema = Schema {
        columns: (0..12)
            .map(|i| Column {
                name: format!("c{i}"),
                data_type: DataType::Integer,
                nullable: true,
            })
            .collect(),
    };
    let row: Vec<Value> = (0..12)
        .map(|i| {
            if i % 3 == 0 || i == 10 {
                Value::Null
            } else {
                Value::Integer(i * 100)
            }
        })
        .collect();
    let bytes = tuple::serialize(&schema, &row).expect("serialize wide row");
    let back = tuple::deserialize(&schema, &bytes).expect("deserialize wide row");
    assert_eq!(back, row);
}

#[test]
#[ignore = "milestone 1"]
fn ms1_tuple_serde_rejects_bad_inputs() {
    let schema = test_schema();
    // wrong arity
    assert!(tuple::serialize(&schema, &[Value::Integer(1)]).is_err());
    // NULL in non-nullable column 0
    assert!(tuple::serialize(
        &schema,
        &[Value::Null, Value::Null, Value::Null, Value::Null]
    )
    .is_err());
    // type mismatch (Text in Integer column)
    assert!(tuple::serialize(
        &schema,
        &[
            Value::Text("oops".into()),
            Value::Null,
            Value::Null,
            Value::Null,
        ]
    )
    .is_err());
    // truncated buffer
    assert!(tuple::deserialize(&schema, &[]).is_err());
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
        matches!(
            disk.read_page(9999, &mut out),
            Err(Error::InvalidPage(9999))
        ),
        "unallocated page id must error with InvalidPage"
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

    // delete every 10th, then scan sees exactly the survivors in (page, slot) order
    for rid in rids.iter().step_by(10) {
        heap.delete(*rid).unwrap();
    }
    let scanned: Vec<_> = heap.scan().collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(scanned.len(), 500 - 50);

    // A short later row may validly backfill free space on an earlier page,
    // so sort expected survivors by their assigned (page_id, slot) address.
    let mut expected: Vec<(RecordId, i64)> = rids
        .iter()
        .enumerate()
        .filter(|(i, _)| i % 10 != 0)
        .map(|(i, &rid)| (rid, i as i64))
        .collect();
    expected.sort_by_key(|(rid, _)| *rid);

    let got: Vec<(RecordId, i64)> = scanned
        .iter()
        .map(|(rid, _, b)| {
            let id = match tuple::deserialize(&schema, b).unwrap()[0] {
                Value::Integer(v) => v,
                _ => panic!("first column is the id"),
            };
            (*rid, id)
        })
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
        let rows = heap.scan().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(rows.len(), 50, "all 50 tuples readable after close/reopen");
        let mut ids: Vec<i64> = rows
            .iter()
            .map(
                |(_, _, b)| match tuple::deserialize(&schema, b).unwrap()[0] {
                    Value::Integer(v) => v,
                    _ => panic!("first column is id"),
                },
            )
            .collect();
        ids.sort_unstable();
        assert_eq!(ids, (0..50).collect::<Vec<_>>());
    }
}

#[test]
#[ignore = "milestone 1"]
fn ms1_heapfile_rejects_oversized_tuple() {
    let dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn PageStore> = Arc::new(DiskManager::open(&dir.path().join("t.db")).unwrap());
    let heap = HeapFile::create(store).unwrap();
    let huge = vec![0xAAu8; PAGE_SIZE];
    assert!(
        matches!(
            heap.insert(TupleMeta::default(), &huge),
            Err(Error::PageFull)
        ),
        "a tuple larger than one page must return Error::PageFull"
    );
}

// -------------------------------------------------- MS8 & MS9 page hooks

#[test]
#[ignore = "milestone 8"]
fn ms8_heap_page_compact_reclaims_space() {
    let mut page = Page::new();
    let mut heap = HeapPage::init(&mut page);
    let mut slots = Vec::new();
    for i in 0..20u8 {
        slots.push(heap.insert(TupleMeta::default(), &[i; 200]).unwrap());
    }
    let free_before = heap.free_space();
    for &s in slots.iter().step_by(2) {
        heap.delete(s).unwrap();
    }
    let reclaimed = heap.compact();
    assert!(
        reclaimed >= 2000,
        "compaction must reclaim dead tuple bodies"
    );
    assert!(heap.free_space() > free_before);
    // Live slot numbers and payloads are preserved intact.
    for (idx, &s) in slots.iter().enumerate() {
        if idx % 2 == 0 {
            assert!(heap.get(s).is_none());
        } else {
            let (_, bytes) = heap.get(s).expect("live slot preserved");
            assert_eq!(bytes, &[idx as u8; 200]);
        }
    }
}

#[test]
#[ignore = "milestone 9"]
fn ms9_heap_page_put_at_redo_undo() {
    let mut page = Page::new();
    let mut heap = HeapPage::init(&mut page);
    let m1 = TupleMeta { xmin: 1, xmax: 0 };
    let m2 = TupleMeta { xmin: 2, xmax: 0 };

    // Redo an insert directly at slot 2 (skips 0 and 1 as dead slots).
    heap.put_at(2, m1, b"hello").unwrap();
    assert_eq!(heap.get(0), None);
    assert_eq!(heap.get(1), None);
    assert_eq!(heap.get(2), Some((m1, &b"hello"[..])));

    // Overwrite slot 2 with a longer payload (Update redo / undo).
    heap.put_at(2, m2, b"a much longer updated payload")
        .unwrap();
    assert_eq!(
        heap.get(2),
        Some((m2, &b"a much longer updated payload"[..]))
    );

    // Tombstone, then resurrect slot 2 (Delete undo).
    heap.delete(2).unwrap();
    assert_eq!(heap.get(2), None);
    heap.put_at(2, m1, b"hello").unwrap();
    assert_eq!(heap.get(2), Some((m1, &b"hello"[..])));
}
