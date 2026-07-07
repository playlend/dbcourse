//! Provided integration tests for `ferricdb-catalog`.
//!
//! Every test is `#[ignore]`d with the milestone that makes it pass.
//! Un-ignore per milestone, e.g.:
//!
//! ```text
//! cargo test -p ferricdb-catalog -- --include-ignored ms3_
//! ```
//!
//! The tests drive the catalog through the `PageStore` trait using the
//! Milestone 1 `DiskManager` directly, so persistence is tested against a
//! real file (in a temp dir): "reopen" below means dropping the catalog
//! *and* the store, then opening a fresh `DiskManager` on the same path and
//! bootstrapping again.

use std::path::Path;
use std::sync::Arc;

use ferricdb_catalog::{Catalog, ColumnStats, TableStats};
use ferricdb_common::{Column, DataType, Error, Schema, Value, INVALID_PAGE_ID};
use ferricdb_storage::{DiskManager, PageStore};

/// Open (or reopen) a page store backed by the database file at `path`.
fn open_store(path: &Path) -> Arc<dyn PageStore> {
    Arc::new(DiskManager::open(path).expect("open disk manager"))
}

/// Two-column schema used by most tests: id INTEGER NOT NULL, name TEXT NULL.
fn users_schema() -> Schema {
    Schema {
        columns: vec![
            Column {
                name: "id".into(),
                data_type: DataType::Integer,
                nullable: false,
            },
            Column {
                name: "name".into(),
                data_type: DataType::Text,
                nullable: true,
            },
        ],
    }
}

fn assert_schemas_equal(actual: &Schema, expected: &Schema) {
    assert_eq!(
        actual.columns.len(),
        expected.columns.len(),
        "column count differs"
    );
    for (a, e) in actual.columns.iter().zip(&expected.columns) {
        assert_eq!(a.name, e.name, "column name differs");
        assert_eq!(a.data_type, e.data_type, "data type differs for {}", e.name);
        assert_eq!(a.nullable, e.nullable, "nullability differs for {}", e.name);
    }
}

// ---------------------------------------------------------------------------
// Milestone 3 — bootstrap + table DDL
// ---------------------------------------------------------------------------

#[test]
#[ignore = "milestone 3"]
fn ms3_bootstrap_create_table_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_store(&dir.path().join("db"));
    let catalog = Catalog::bootstrap(store).unwrap();

    // A freshly bootstrapped empty store has no user tables (the system
    // tables __tables / __columns are internal and never listed).
    assert!(catalog.tables().unwrap().is_empty());

    let oid = catalog.create_table("users", users_schema()).unwrap();

    // Point lookup returns the full metadata.
    let info = catalog.table("users").unwrap();
    assert_eq!(info.oid, oid);
    assert_eq!(info.name, "users");
    assert_schemas_equal(&info.schema, &users_schema());
    assert_ne!(
        info.first_page, INVALID_PAGE_ID,
        "create_table must allocate the table's heap file"
    );
    assert!(info.indexes.is_empty(), "no indexes before milestone 5");

    // Listing includes it.
    let all = catalog.tables().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].name, "users");
    assert_eq!(all[0].oid, oid);

    // A second table gets a distinct oid.
    let oid2 = catalog
        .create_table(
            "orders",
            Schema {
                columns: vec![Column {
                    name: "id".into(),
                    data_type: DataType::Integer,
                    nullable: false,
                }],
            },
        )
        .unwrap();
    assert_ne!(oid, oid2, "oids must be unique");
    assert_eq!(catalog.tables().unwrap().len(), 2);

    // Unknown tables are a catalog error.
    assert!(matches!(
        catalog.table("nonexistent"),
        Err(Error::Catalog(_))
    ));
}

#[test]
#[ignore = "milestone 3"]
fn ms3_duplicate_create_table_errors() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_store(&dir.path().join("db"));
    let catalog = Catalog::bootstrap(store).unwrap();

    catalog.create_table("users", users_schema()).unwrap();
    let err = catalog.create_table("users", users_schema()).unwrap_err();
    assert!(
        matches!(err, Error::Catalog(_)),
        "duplicate CREATE TABLE must be Error::Catalog, got: {err:?}"
    );

    // The failed create must not have clobbered the existing table.
    assert_eq!(catalog.tables().unwrap().len(), 1);
    assert_schemas_equal(&catalog.table("users").unwrap().schema, &users_schema());
}

#[test]
#[ignore = "milestone 3"]
fn ms3_drop_table_removes() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_store(&dir.path().join("db"));
    let catalog = Catalog::bootstrap(store).unwrap();

    catalog.create_table("users", users_schema()).unwrap();
    catalog.create_table("orders", users_schema()).unwrap();

    catalog.drop_table("users").unwrap();

    assert!(matches!(catalog.table("users"), Err(Error::Catalog(_))));
    let remaining = catalog.tables().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].name, "orders");

    // Dropping a missing table is a catalog error (and dropping twice too).
    assert!(matches!(
        catalog.drop_table("users"),
        Err(Error::Catalog(_))
    ));
    assert!(matches!(
        catalog.drop_table("never_existed"),
        Err(Error::Catalog(_))
    ));

    // The name becomes reusable after the drop.
    catalog.create_table("users", users_schema()).unwrap();
    assert_eq!(catalog.tables().unwrap().len(), 2);
}

#[test]
#[ignore = "milestone 3"]
fn ms3_catalog_persists_across_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");

    let (users_oid, users_first_page) = {
        let store = open_store(&path);
        let catalog = Catalog::bootstrap(Arc::clone(&store)).unwrap();
        let oid = catalog.create_table("users", users_schema()).unwrap();
        catalog.create_table("scratch", users_schema()).unwrap();
        catalog.drop_table("scratch").unwrap();
        let first_page = catalog.table("users").unwrap().first_page;
        store.flush_all().unwrap();
        (oid, first_page)
        // catalog and store dropped here — simulated shutdown
    };

    // Reopen the same file; bootstrap must *read* the catalog, not reformat.
    let store = open_store(&path);
    let catalog = Catalog::bootstrap(store).unwrap();

    let all = catalog.tables().unwrap();
    assert_eq!(all.len(), 1, "dropped table must stay dropped after reopen");

    let info = catalog.table("users").unwrap();
    assert_eq!(info.oid, users_oid, "oid must be stable across reopen");
    assert_eq!(
        info.first_page, users_first_page,
        "heap file location must be stable across reopen"
    );
    assert_schemas_equal(&info.schema, &users_schema());
}

#[test]
#[ignore = "milestone 3"]
fn ms3_schema_fidelity_survives_reopen() {
    // One column of every data type, with mixed nullability, to catch
    // encode/decode bugs in the __columns representation.
    let schema = Schema {
        columns: vec![
            Column {
                name: "b".into(),
                data_type: DataType::Boolean,
                nullable: false,
            },
            Column {
                name: "i".into(),
                data_type: DataType::Integer,
                nullable: true,
            },
            Column {
                name: "f".into(),
                data_type: DataType::Float,
                nullable: false,
            },
            Column {
                name: "t".into(),
                data_type: DataType::Text,
                nullable: true,
            },
        ],
    };

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");
    {
        let store = open_store(&path);
        let catalog = Catalog::bootstrap(Arc::clone(&store)).unwrap();
        catalog.create_table("every_type", schema.clone()).unwrap();
        store.flush_all().unwrap();
    }

    let store = open_store(&path);
    let catalog = Catalog::bootstrap(store).unwrap();
    let info = catalog.table("every_type").unwrap();
    assert_schemas_equal(&info.schema, &schema);
    // Column *order* matters too: index_of must agree with the original.
    assert_eq!(info.schema.index_of("b"), Some(0));
    assert_eq!(info.schema.index_of("t"), Some(3));
}

// ---------------------------------------------------------------------------
// Milestone 5 — index metadata
// ---------------------------------------------------------------------------

#[test]
#[ignore = "milestone 5"]
fn ms5_create_index_and_set_root_persist() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db");

    let idx_oid = {
        let store = open_store(&path);
        let catalog = Catalog::bootstrap(Arc::clone(&store)).unwrap();
        catalog.create_table("users", users_schema()).unwrap();

        // The B+Tree itself is created by the exec layer (BPlusTree::create);
        // the catalog only records its metadata, so any page id works here.
        let idx_oid = catalog
            .create_index("users_id_idx", "users", "id", 42)
            .unwrap();

        // Visible immediately through TableInfo.
        let info = catalog.table("users").unwrap();
        assert_eq!(info.indexes.len(), 1);
        let idx = &info.indexes[0];
        assert_eq!(idx.oid, idx_oid);
        assert_eq!(idx.name, "users_id_idx");
        assert_eq!(idx.table, "users");
        assert_eq!(idx.column, "id");
        assert_eq!(idx.root_page, 42);

        // Root split: the catalog must persist the new root pointer.
        catalog.set_index_root(idx_oid, 77).unwrap();
        assert_eq!(catalog.table("users").unwrap().indexes[0].root_page, 77);

        // Updating an unknown index oid is a catalog error.
        assert!(matches!(
            catalog.set_index_root(9_999_999, 5),
            Err(Error::Catalog(_))
        ));

        store.flush_all().unwrap();
        idx_oid
    };

    // Reopen: the index — with the *updated* root — must still be there.
    let store = open_store(&path);
    let catalog = Catalog::bootstrap(store).unwrap();
    let info = catalog.table("users").unwrap();
    assert_eq!(info.indexes.len(), 1);
    assert_eq!(info.indexes[0].oid, idx_oid);
    assert_eq!(info.indexes[0].name, "users_id_idx");
    assert_eq!(info.indexes[0].column, "id");
    assert_eq!(info.indexes[0].root_page, 77);
}

// ---------------------------------------------------------------------------
// Milestone 7 — statistics
// ---------------------------------------------------------------------------

#[test]
#[ignore = "milestone 7"]
fn ms7_set_stats_stats_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_store(&dir.path().join("db"));
    let catalog = Catalog::bootstrap(store).unwrap();
    catalog.create_table("users", users_schema()).unwrap();

    let stats = TableStats {
        row_count: 1_000,
        page_count: 12,
        columns: vec![
            ColumnStats {
                min: Some(Value::Integer(1)),
                max: Some(Value::Integer(1_000)),
                distinct_estimate: 1_000,
                null_count: 0,
            },
            ColumnStats {
                min: Some(Value::Text("alice".into())),
                max: Some(Value::Text("zoe".into())),
                distinct_estimate: 640,
                null_count: 25,
            },
        ],
    };
    catalog.set_stats("users", stats.clone()).unwrap();

    let got = catalog
        .stats("users")
        .unwrap()
        .expect("stats were just set");
    assert_eq!(got.row_count, stats.row_count);
    assert_eq!(got.page_count, stats.page_count);
    assert_eq!(got.columns.len(), stats.columns.len());
    for (g, e) in got.columns.iter().zip(&stats.columns) {
        assert_eq!(g.min, e.min);
        assert_eq!(g.max, e.max);
        assert_eq!(g.distinct_estimate, e.distinct_estimate);
        assert_eq!(g.null_count, e.null_count);
    }

    // A second ANALYZE replaces the stats.
    let newer = TableStats {
        row_count: 2_000,
        page_count: 23,
        columns: vec![ColumnStats::default(), ColumnStats::default()],
    };
    catalog.set_stats("users", newer).unwrap();
    assert_eq!(catalog.stats("users").unwrap().unwrap().row_count, 2_000);

    // Stats for an unknown table are a catalog error, not Ok(None).
    assert!(matches!(
        catalog.stats("nonexistent"),
        Err(Error::Catalog(_))
    ));
    assert!(matches!(
        catalog.set_stats("nonexistent", TableStats::default()),
        Err(Error::Catalog(_))
    ));
}

#[test]
#[ignore = "milestone 7"]
fn ms7_stats_none_for_never_analyzed_table() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_store(&dir.path().join("db"));
    let catalog = Catalog::bootstrap(store).unwrap();

    catalog.create_table("users", users_schema()).unwrap();
    catalog.create_table("orders", users_schema()).unwrap();

    // Existing but never-ANALYZEd table: Ok(None), so the cost model can
    // fall back to default estimates.
    assert!(catalog.stats("users").unwrap().is_none());

    // Analyzing one table must not conjure stats for another.
    catalog
        .set_stats(
            "orders",
            TableStats {
                row_count: 5,
                page_count: 1,
                columns: vec![ColumnStats::default(), ColumnStats::default()],
            },
        )
        .unwrap();
    assert!(catalog.stats("users").unwrap().is_none());
    assert!(catalog.stats("orders").unwrap().is_some());
}
