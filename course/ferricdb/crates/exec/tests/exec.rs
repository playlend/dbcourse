//! Provided Milestone 3 / 4 / 5 / 6 / 7 / 8 test suites for the query engine,
//! driven through `Binder`, `ExternalSorter`, `Planner`, and
//! [`ferricdb_exec::execute`] over a tempfile-backed database.
//!
//! Run per milestone:
//!   cargo test -p ferricdb-exec -- --include-ignored ms3_
//!   cargo test -p ferricdb-exec -- --include-ignored ms4_
//!   cargo test -p ferricdb-exec -- --include-ignored ms5_
//!   cargo test -p ferricdb-exec -- --include-ignored ms6_
//!   cargo test -p ferricdb-exec -- --include-ignored ms7_
//!   cargo test -p ferricdb-exec -- --include-ignored ms8_

use std::sync::Arc;

use ferricdb_buffer::{BufferPoolManager, ClockReplacer};
use ferricdb_catalog::Catalog;
use ferricdb_common::{Column, DataType, Error, Schema, Value};
use ferricdb_exec::binder::{Binder, BoundExpr, BoundStatement};
use ferricdb_exec::plan::LogicalPlan;
use ferricdb_exec::sort::ExternalSorter;
use ferricdb_exec::{execute, ExecContext, Planner, QueryResult};
use ferricdb_sql::ast::BinaryOp;
use ferricdb_sql::Parser;
use ferricdb_storage::{DiskManager, PageStore};
use ferricdb_txn::TransactionManager;

struct Db {
    ctx: Arc<ExecContext>,
    _dir: tempfile::TempDir,
}

fn fresh_db() -> Db {
    let dir = tempfile::tempdir().unwrap();
    let disk = DiskManager::open(&dir.path().join("t.db")).unwrap();
    let pool: Arc<dyn PageStore> = Arc::new(BufferPoolManager::new(
        64,
        disk,
        Box::new(ClockReplacer::new(64)),
    ));
    let catalog = Arc::new(Catalog::bootstrap(Arc::clone(&pool)).unwrap());
    let ctx = Arc::new(ExecContext::new(
        catalog,
        pool,
        Arc::new(TransactionManager::new()),
    ));
    Db { ctx, _dir: dir }
}

fn run(db: &Db, sql: &str) -> Vec<QueryResult> {
    execute(sql, &db.ctx).unwrap_or_else(|e| panic!("{sql}: {e:?}"))
}

fn rows(db: &Db, sql: &str) -> Vec<Vec<Value>> {
    run(db, sql).pop().unwrap().rows
}

fn ints(db: &Db, sql: &str) -> Vec<i64> {
    rows(db, sql)
        .into_iter()
        .map(|r| match &r[0] {
            Value::Integer(v) => *v,
            other => panic!("{sql}: expected an integer, got {other:?}"),
        })
        .collect()
}

fn seed_users(db: &Db) {
    run(
        db,
        "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER)",
    );
    run(
        db,
        "INSERT INTO users (id, name, age) VALUES \
         (1, 'ada', 36), (2, 'bob', 29), (3, 'eve', NULL), (4, 'dan', 41)",
    );
}

// ================================================================= MS3

#[test]
#[ignore = "milestone 3"]
fn ms3_binder_resolves_and_rejects() {
    let db = fresh_db();
    db.ctx
        .catalog
        .create_table(
            "users",
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
                        nullable: false,
                    },
                    Column {
                        name: "score".into(),
                        data_type: DataType::Float,
                        nullable: true,
                    },
                ],
            },
        )
        .unwrap();

    let binder = Binder::new(&db.ctx.catalog);

    // Valid SELECT resolves names and types.
    let stmt = Parser::parse("SELECT id, name AS who FROM users WHERE score > 10")
        .unwrap()
        .pop()
        .unwrap();
    let BoundStatement::Select(bound) = binder.bind(stmt).expect("valid SELECT binds") else {
        panic!("expected BoundStatement::Select");
    };
    assert_eq!(bound.schema.columns.len(), 2);
    assert_eq!(bound.schema.columns[0].name, "id");
    assert_eq!(bound.schema.columns[1].name, "who");

    // Unknown table, unknown column, non-boolean WHERE, and bad arithmetic all return Error::Bind.
    for bad in [
        "SELECT id FROM ghost",
        "SELECT missing FROM users",
        "SELECT id FROM users WHERE id",
        "SELECT name + 1 FROM users",
        "INSERT INTO users (id, name) VALUES (1)",
    ] {
        let stmt = Parser::parse(bad).unwrap().pop().unwrap();
        assert!(
            matches!(binder.bind(stmt), Err(Error::Bind(_))),
            "{bad:?} must fail binding with Error::Bind"
        );
    }
}

// ================================================================= MS4

#[test]
#[ignore = "milestone 4"]
fn ms4_values_projection_expression_eval() {
    let db = fresh_db();
    let r = run(&db, "SELECT 1 + 2 * 3");
    assert_eq!(r[0].rows, vec![vec![Value::Integer(7)]]);

    let r = rows(
        &db,
        "SELECT CAST(1 AS FLOAT), 'a', true, NULL IS NULL, 7 % 3",
    );
    assert_eq!(
        r,
        vec![vec![
            Value::Float(1.0),
            Value::Text("a".into()),
            Value::Boolean(true),
            Value::Boolean(true),
            Value::Integer(1),
        ]]
    );
}

#[test]
#[ignore = "milestone 4"]
fn ms4_crud_roundtrip_with_counts() {
    let db = fresh_db();
    seed_users(&db);

    let mut got = ints(&db, "SELECT id FROM users");
    got.sort_unstable();
    assert_eq!(got, vec![1, 2, 3, 4]);

    let upd = run(&db, "UPDATE users SET age = 30 WHERE name = 'bob'");
    assert_eq!(upd[0].affected, 1);
    assert_eq!(
        ints(&db, "SELECT age FROM users WHERE name = 'bob'"),
        vec![30]
    );

    let del = run(&db, "DELETE FROM users WHERE age >= 36");
    assert_eq!(del[0].affected, 2, "ada (36) and dan (41)");
    let mut left = ints(&db, "SELECT id FROM users");
    left.sort_unstable();
    assert_eq!(left, vec![2, 3]);
}

#[test]
#[ignore = "milestone 4"]
fn ms4_three_valued_logic_in_where() {
    let db = fresh_db();
    seed_users(&db);

    // eve's NULL age: dropped by both the predicate AND its negation
    let over_30: Vec<i64> = {
        let mut v = ints(&db, "SELECT id FROM users WHERE age > 30");
        v.sort_unstable();
        v
    };
    assert_eq!(over_30, vec![1, 4]);
    let not_over_30 = ints(&db, "SELECT id FROM users WHERE NOT (age > 30)");
    assert_eq!(not_over_30, vec![2], "NOT NULL is NULL — eve still drops");
    assert_eq!(ints(&db, "SELECT id FROM users WHERE age IS NULL"), vec![3]);
    assert!(ints(&db, "SELECT id FROM users WHERE age = NULL").is_empty());
    // short-circuit rows of the truth table
    assert_eq!(
        rows(&db, "SELECT id FROM users WHERE false AND age = NULL").len(),
        0
    );
    assert_eq!(
        ints(&db, "SELECT id FROM users WHERE true OR age > 999").len(),
        4,
        "true OR NULL is true"
    );
}

#[test]
#[ignore = "milestone 4"]
fn ms4_order_by_limit_and_null_ordering() {
    let db = fresh_db();
    seed_users(&db);

    assert_eq!(
        ints(&db, "SELECT id FROM users ORDER BY age DESC"),
        vec![4, 1, 2, 3],
        "41, 36, 29, then NULL last (course convention)"
    );
    assert_eq!(
        ints(&db, "SELECT id FROM users ORDER BY age"),
        vec![2, 1, 4, 3],
        "NULLs order last under ASC too"
    );
    assert_eq!(
        ints(&db, "SELECT id FROM users ORDER BY id LIMIT 2"),
        vec![1, 2]
    );
}

#[test]
#[ignore = "milestone 4"]
fn ms4_halloween_guard() {
    let db = fresh_db();
    run(&db, "CREATE TABLE n (x INTEGER)");
    // 95..105: rows below 100 qualify; incrementing by +1 keeps 95..98 STILL
    // < 100, so a naive streaming UPDATE without a Halloween guard re-visits
    // newly inserted tuples and increments them repeatedly until they hit 100!
    for x in 95..105 {
        run(&db, &format!("INSERT INTO n (x) VALUES ({x})"));
    }
    let upd = run(&db, "UPDATE n SET x = x + 1 WHERE x < 100");
    assert_eq!(upd[0].affected, 5);
    let mut got = ints(&db, "SELECT x FROM n");
    got.sort_unstable();
    assert_eq!(
        got,
        vec![96, 97, 98, 99, 100, 100, 101, 102, 103, 104],
        "each qualifying row processed exactly once (the Halloween problem)"
    );
}

#[test]
#[ignore = "milestone 4"]
fn ms4_errors_are_errors() {
    let db = fresh_db();
    seed_users(&db);
    assert!(execute("SELECT ghost FROM users", &db.ctx).is_err());
    assert!(execute("SELECT id FROM nowhere", &db.ctx).is_err());
    assert!(execute("INSERT INTO users (id, name) VALUES (9)", &db.ctx).is_err());
    assert!(
        execute("SELECT name + 1 FROM users", &db.ctx).is_err(),
        "type error"
    );
}

// ================================================================= MS5

#[test]
#[ignore = "milestone 5"]
fn ms5_create_index_and_index_scan_execution() {
    let db = fresh_db();
    run(&db, "CREATE TABLE items (id INTEGER, grp INTEGER)");
    for i in 0..200 {
        run(&db, &format!("INSERT INTO items VALUES ({i}, {})", i % 10));
    }
    run(&db, "CREATE INDEX idx_items_id ON items (id)");
    run(&db, "CREATE INDEX idx_items_grp ON items (grp)");

    // Subsequent inserts must maintain existing indexes too.
    run(&db, "INSERT INTO items VALUES (200, 0), (201, 1)");

    assert_eq!(ints(&db, "SELECT id FROM items WHERE id = 42"), vec![42]);
    assert_eq!(ints(&db, "SELECT id FROM items WHERE id = 200"), vec![200]);

    let mut range_ids = ints(&db, "SELECT id FROM items WHERE id >= 10 AND id < 15");
    range_ids.sort_unstable();
    assert_eq!(range_ids, vec![10, 11, 12, 13, 14]);

    // Non-unique secondary index scan returns all matching rows.
    let grp0 = ints(&db, "SELECT id FROM items WHERE grp = 0");
    assert_eq!(grp0.len(), 21, "0, 10, ..., 200");
}

// ================================================================= MS6

fn seed_join_tables(db: &Db) {
    run(
        db,
        "CREATE TABLE c (cid INTEGER PRIMARY KEY, nation TEXT NOT NULL)",
    );
    run(
        db,
        "INSERT INTO c (cid, nation) VALUES (1, 'DE'), (2, 'US'), (3, 'DE')",
    );
    run(
        db,
        "CREATE TABLE o (oid INTEGER PRIMARY KEY, cust INTEGER, total FLOAT)",
    );
    run(
        db,
        "INSERT INTO o (oid, cust, total) VALUES \
         (10, 1, 100), (11, 1, 50), (12, 2, 25), (13, 3, 75), (14, 9, 1)",
    );
}

#[test]
#[ignore = "milestone 6"]
fn ms6_inner_join_drops_orphans() {
    let db = fresh_db();
    seed_join_tables(&db);
    let mut got = ints(&db, "SELECT oid FROM c JOIN o ON cid = cust");
    got.sort_unstable();
    assert_eq!(got, vec![10, 11, 12, 13], "order 14 has no customer");
    // cross join count
    assert_eq!(ints(&db, "SELECT COUNT(*) FROM c, o"), vec![15]);
}

#[test]
#[ignore = "milestone 6"]
fn ms6_group_by_aggregates() {
    let db = fresh_db();
    seed_join_tables(&db);
    let mut got = rows(
        &db,
        "SELECT nation, COUNT(*), SUM(total) FROM c JOIN o ON cid = cust GROUP BY nation",
    );
    got.sort_by(|a, b| format!("{:?}", a[0]).cmp(&format!("{:?}", b[0])));
    assert_eq!(
        got,
        vec![
            vec![
                Value::Text("DE".into()),
                Value::Integer(3),
                Value::Float(225.0)
            ],
            vec![
                Value::Text("US".into()),
                Value::Integer(1),
                Value::Float(25.0)
            ],
        ]
    );
    // zero-input aggregates: COUNT 0, others NULL
    assert_eq!(
        rows(&db, "SELECT COUNT(*), SUM(total), MIN(total), MAX(total), AVG(total) FROM o WHERE total < 0"),
        vec![vec![
            Value::Integer(0),
            Value::Null,
            Value::Null,
            Value::Null,
            Value::Null
        ]]
    );
}

#[test]
#[ignore = "milestone 6"]
fn ms6_external_sorter_spills_runs() {
    let db = fresh_db();
    let schema = Schema {
        columns: vec![
            Column {
                name: "k".into(),
                data_type: DataType::Integer,
                nullable: true,
            },
            Column {
                name: "pad".into(),
                data_type: DataType::Text,
                nullable: false,
            },
        ],
    };
    let mut sorter = ExternalSorter::new(Arc::clone(&db.ctx.store), schema, 4, vec![(0, false)]);
    let mut expected: Vec<i64> = Vec::new();
    for i in 0..1500i64 {
        let k = (i * 7919) % 1500;
        expected.push(k);
        sorter
            .push(vec![Value::Integer(k), Value::Text("x".repeat(40))])
            .unwrap();
    }
    sorter
        .push(vec![Value::Null, Value::Text("null-last".into())])
        .unwrap();

    assert!(
        sorter.spilled_runs() > 0,
        "1500 rows (~80 KB) must spill runs under a 4-page (32 KB) budget"
    );
    let out = sorter.finish().unwrap();
    assert_eq!(out.len(), 1501);
    expected.sort_unstable();
    let got_non_null: Vec<i64> = out[..1500]
        .iter()
        .map(|r| match r[0] {
            Value::Integer(v) => v,
            _ => panic!("expected integer"),
        })
        .collect();
    assert_eq!(got_non_null, expected);
    assert_eq!(out[1500][0], Value::Null, "NULL sorts last");
}

#[test]
#[ignore = "milestone 6"]
fn ms6_external_sort_under_tiny_budget() {
    let db = fresh_db();
    run(&db, "CREATE TABLE big (k INTEGER, pad TEXT NOT NULL)");
    // enough rows (~80 KB) that they cannot fit in a 4-page (32 KB) budget
    for chunk in 0..20 {
        let mut vals = Vec::new();
        for i in 0..100 {
            let k = (chunk * 100 + i) * 7919 % 2000; // scrambled but deterministic
            vals.push(format!("({k}, 'padpadpadpadpadpadpadpad')"));
        }
        run(
            &db,
            &format!("INSERT INTO big (k, pad) VALUES {}", vals.join(", ")),
        );
    }

    // in-memory reference with the default budget
    let reference = ints(&db, "SELECT k FROM big ORDER BY k");
    assert_eq!(reference.len(), 2000);

    // now force spilling (memory_pages = 4 gives textbook 3-way merge)
    let mut tiny = db.ctx.new_session();
    tiny.memory_pages = 4;
    let spilled = execute("SELECT k FROM big ORDER BY k", &tiny)
        .unwrap()
        .pop()
        .unwrap();
    let spilled: Vec<i64> = spilled
        .rows
        .into_iter()
        .map(|r| match r[0] {
            Value::Integer(v) => v,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(
        spilled, reference,
        "external sort result equals in-memory sort"
    );
}

// ================================================================= MS7

fn lit(v: i64) -> BoundExpr {
    BoundExpr::Literal(Value::Integer(v))
}

fn colref(table: usize, column: usize) -> BoundExpr {
    BoundExpr::Column {
        table,
        column,
        data_type: DataType::Integer,
    }
}

#[test]
#[ignore = "milestone 7"]
fn ms7_constant_folding() {
    let db = fresh_db();
    let planner = Planner::new(&db.ctx);
    let out_schema = Schema {
        columns: vec![Column {
            name: "c0".into(),
            data_type: DataType::Integer,
            nullable: true,
        }],
    };
    let plan = LogicalPlan::Projection {
        input: Box::new(LogicalPlan::Values {
            rows: vec![vec![lit(1)]],
            schema: Schema::default(),
        }),
        exprs: vec![BoundExpr::Binary(
            Box::new(lit(2)),
            BinaryOp::Mul,
            Box::new(BoundExpr::Binary(
                Box::new(lit(3)),
                BinaryOp::Add,
                Box::new(lit(4)),
            )),
        )],
        schema: out_schema,
    };
    let optimized = planner.optimize(plan).unwrap();
    let LogicalPlan::Projection { exprs, .. } = optimized else {
        panic!("folding must preserve the Projection node");
    };
    assert_eq!(exprs, vec![lit(14)], "2 * (3 + 4) folds to 14 at plan time");
}

#[test]
#[ignore = "milestone 7"]
fn ms7_predicate_pushdown_single_side() {
    let db = fresh_db();
    run(&db, "CREATE TABLE t (a INTEGER NOT NULL, b INTEGER)");
    let t = db.ctx.catalog.table("t").unwrap();
    let planner = Planner::new(&db.ctx);

    // Filter(left.a = 5) above a cross join must land on the LEFT input
    let plan = LogicalPlan::Filter {
        predicate: BoundExpr::Binary(Box::new(colref(0, 0)), BinaryOp::Eq, Box::new(lit(5))),
        input: Box::new(LogicalPlan::NestedLoopJoin {
            left: Box::new(LogicalPlan::SeqScan {
                table_idx: 0,
                table: t.clone(),
                filter: None,
            }),
            right: Box::new(LogicalPlan::SeqScan {
                table_idx: 1,
                table: t.clone(),
                filter: None,
            }),
            on: None,
        }),
    };
    let optimized = planner.optimize(plan).unwrap();
    // accepted shapes: the filter fused into the left scan, or a Filter
    // node directly above the left scan — but NOT above the join anymore
    fn left_scan_is_filtered(p: &LogicalPlan) -> bool {
        match p {
            LogicalPlan::NestedLoopJoin { left, .. } => match left.as_ref() {
                LogicalPlan::SeqScan { filter, .. } => filter.is_some(),
                LogicalPlan::Filter { input, .. } => {
                    matches!(input.as_ref(), LogicalPlan::SeqScan { .. })
                }
                _ => false,
            },
            _ => false,
        }
    }
    assert!(
        left_scan_is_filtered(&optimized),
        "single-side predicate must sit below the join, got {optimized:?}"
    );
}

#[test]
#[ignore = "milestone 7"]
fn ms7_analyze_then_explain_picks_access_path() {
    let db = fresh_db();
    run(&db, "CREATE TABLE t (id INTEGER PRIMARY KEY, grp INTEGER)");
    let mut vals = Vec::new();
    for i in 0..2000 {
        vals.push(format!("({i}, {})", i % 4));
    }
    run(
        &db,
        &format!("INSERT INTO t (id, grp) VALUES {}", vals.join(", ")),
    );
    run(&db, "CREATE INDEX t_id ON t (id)");
    run(&db, "ANALYZE t");

    let selective = run(&db, "EXPLAIN SELECT grp FROM t WHERE id = 42");
    let plan = selective[0].explain.clone().expect("EXPLAIN fills explain");
    assert!(
        plan.contains("IndexScan"),
        "1/2000 selectivity must choose the index: {plan}"
    );

    let unselective = run(&db, "EXPLAIN SELECT grp FROM t WHERE id > 2");
    let plan = unselective[0].explain.clone().unwrap();
    assert!(
        plan.contains("SeqScan"),
        "~all-rows selectivity must choose the seq scan: {plan}"
    );
}

#[test]
#[ignore = "milestone 7"]
fn ms7_explain_does_not_execute_but_analyze_does() {
    let db = fresh_db();
    run(&db, "CREATE TABLE t (x INTEGER)");
    run(&db, "INSERT INTO t (x) VALUES (1)");

    let r = run(&db, "EXPLAIN DELETE FROM t");
    assert!(r[0].explain.is_some());
    assert_eq!(
        ints(&db, "SELECT COUNT(*) FROM t"),
        vec![1],
        "EXPLAIN must not run the DELETE"
    );

    let r = run(&db, "EXPLAIN ANALYZE DELETE FROM t");
    assert!(r[0].explain.is_some());
    assert_eq!(
        ints(&db, "SELECT COUNT(*) FROM t"),
        vec![0],
        "EXPLAIN ANALYZE runs it"
    );
}

// ================================================================= MS8

#[test]
#[ignore = "milestone 8"]
fn ms8_sql_transactions_commit_rollback_isolation() {
    let db = fresh_db();
    run(
        &db,
        "CREATE TABLE acct (id INTEGER PRIMARY KEY, bal INTEGER)",
    );
    run(&db, "INSERT INTO acct VALUES (1, 100)");

    let s1 = db.ctx.new_session();
    let s2 = db.ctx.new_session();

    // Uncommitted writes in s1 are invisible to s2 (no dirty read) and vanish on ROLLBACK.
    execute("BEGIN", &s1).unwrap();
    execute("UPDATE acct SET bal = 250 WHERE id = 1", &s1).unwrap();
    let s2_bal = execute("SELECT bal FROM acct WHERE id = 1", &s2)
        .unwrap()
        .pop()
        .unwrap()
        .rows;
    assert_eq!(s2_bal, vec![vec![Value::Integer(100)]], "no dirty read");
    execute("ROLLBACK", &s1).unwrap();
    assert_eq!(ints(&db, "SELECT bal FROM acct WHERE id = 1"), vec![100]);

    // Snapshot isolation: s2 sees its BEGIN snapshot even after s1 commits.
    execute("BEGIN", &s2).unwrap();
    execute("BEGIN", &s1).unwrap();
    execute("UPDATE acct SET bal = 150 WHERE id = 1", &s1).unwrap();
    execute("COMMIT", &s1).unwrap();
    let s2_still = execute("SELECT bal FROM acct WHERE id = 1", &s2)
        .unwrap()
        .pop()
        .unwrap()
        .rows;
    assert_eq!(
        s2_still,
        vec![vec![Value::Integer(100)]],
        "snapshot isolation prevents fuzzy reads"
    );
    execute("COMMIT", &s2).unwrap();

    // After s2 closes, a new statement sees 150 and VACUUM succeeds.
    assert_eq!(ints(&db, "SELECT bal FROM acct WHERE id = 1"), vec![150]);
    execute("VACUUM", &db.ctx).unwrap();
    assert_eq!(ints(&db, "SELECT bal FROM acct WHERE id = 1"), vec![150]);
}
