//! End-to-end sqllogictest harness (provided plumbing — not learning
//! content). Replays `tests/slt/*.slt` files through
//! [`ferricdb_exec::execute`] over a fresh tempfile database.
//!
//! Run per milestone:
//!   cargo test -p ferricdb -- --include-ignored ms4_slt_crud
//!   cargo test -p ferricdb -- --include-ignored ms6_slt_joins_agg_sort
//!   cargo test -p ferricdb -- --include-ignored slt_all

use std::path::Path;
use std::sync::Arc;

use ferricdb_buffer::{BufferPoolManager, ClockReplacer};
use ferricdb_catalog::Catalog;
use ferricdb_common::Value;
use ferricdb_exec::{execute, ExecContext};
use ferricdb_storage::{DiskManager, PageStore};
use ferricdb_txn::TransactionManager;
use sqllogictest::{DBOutput, DefaultColumnType};

/// One live FerricDB, driven statement-by-statement by the runner.
struct FerricDb {
    ctx: Arc<ExecContext>,
    // keep the tempdir alive for the database file's lifetime
    _dir: tempfile::TempDir,
}

impl FerricDb {
    fn fresh() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let disk = DiskManager::open(&dir.path().join("slt.db")).expect("disk");
        let pool: Arc<dyn PageStore> = Arc::new(BufferPoolManager::new(
            64,
            disk,
            Box::new(ClockReplacer::new(64)),
        ));
        let catalog = Arc::new(Catalog::bootstrap(Arc::clone(&pool)).expect("catalog"));
        let ctx = Arc::new(ExecContext::new(
            catalog,
            pool,
            Arc::new(TransactionManager::new()),
        ));
        FerricDb { ctx, _dir: dir }
    }

    /// Render a value the way the expected files are written: NULL for
    /// NULL, integers/floats/text/booleans in their natural text form.
    fn render(v: &Value) -> String {
        match v {
            Value::Null => "NULL".to_string(),
            Value::Boolean(b) => b.to_string(),
            Value::Integer(i) => i.to_string(),
            Value::Float(f) => format!("{f}"),
            Value::Text(s) => s.clone(),
        }
    }
}

impl sqllogictest::DB for FerricDb {
    type Error = ferricdb_common::Error;
    type ColumnType = DefaultColumnType;

    fn run(&mut self, sql: &str) -> Result<DBOutput<DefaultColumnType>, Self::Error> {
        let mut results = execute(sql, &self.ctx)?;
        // one statement per sqllogictest record
        let result = results.pop().expect("execute returns at least one result");
        if result.schema.columns.is_empty() {
            return Ok(DBOutput::StatementComplete(result.affected));
        }
        let types = result
            .schema
            .columns
            .iter()
            .map(|c| match c.data_type {
                ferricdb_common::DataType::Integer => DefaultColumnType::Integer,
                ferricdb_common::DataType::Float => DefaultColumnType::FloatingPoint,
                _ => DefaultColumnType::Text,
            })
            .collect();
        let rows = result
            .rows
            .iter()
            .map(|row| row.iter().map(Self::render).collect())
            .collect();
        Ok(DBOutput::Rows { types, rows })
    }
}

fn run_slt_file(file: &Path) {
    let mut runner = sqllogictest::Runner::new(|| async { Ok(FerricDb::fresh()) });
    runner
        .run_file(file)
        .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    println!("ok — {}", file.display());
}

#[test]
#[ignore = "milestone 4"]
fn ms4_slt_crud() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/slt/ms4_crud.slt");
    run_slt_file(&file);
}

#[test]
#[ignore = "milestone 6"]
fn ms6_slt_joins_agg_sort() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/slt/ms6_joins_agg_sort.slt");
    run_slt_file(&file);
}

#[test]
#[ignore = "milestone 6"]
fn slt_all() {
    let slt_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/slt");
    let mut files: Vec<_> = std::fs::read_dir(&slt_dir)
        .expect("tests/slt exists")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "slt"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .slt files found in {slt_dir:?}");

    for file in files {
        run_slt_file(&file);
    }
}
