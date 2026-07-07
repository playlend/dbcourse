//! Provided Milestone 3 test suite: the lexer, the statement grammar, and
//! the Pratt expression parser (with the Milestone 6 grammar extensions
//! ignored separately at the bottom).
//!
//! Run with `cargo test -p ferricdb-sql -- --include-ignored ms3_`.

use ferricdb_common::{DataType, Value};
use ferricdb_sql::ast::{
    BinaryOp, ColumnDef, Expr, JoinKind, SelectItem, Statement, TableRef, UnaryOp,
};
use ferricdb_sql::{Keyword, Lexer, Parser, Token};

fn parse_one(sql: &str) -> Statement {
    let mut stmts = Parser::parse(sql).expect("valid SQL must parse");
    assert_eq!(stmts.len(), 1, "expected exactly one statement");
    stmts.pop().unwrap()
}

fn col(name: &str) -> Expr {
    Expr::Column(None, name.to_string())
}

fn int(v: i64) -> Expr {
    Expr::Literal(Value::Integer(v))
}

fn bin(l: Expr, op: BinaryOp, r: Expr) -> Expr {
    Expr::Binary(Box::new(l), op, Box::new(r))
}

// ------------------------------------------------------------------ lexer

#[test]
#[ignore = "milestone 3"]
fn ms3_lexer_keywords_are_case_insensitive() {
    for src in ["SELECT", "select", "SeLeCt"] {
        assert_eq!(
            Lexer::tokenize(src).unwrap(),
            vec![Token::Keyword(Keyword::Select)],
            "keyword case-insensitivity for {src:?}"
        );
    }
    // ...but non-keywords are identifiers, lowercased
    assert_eq!(
        Lexer::tokenize("Selection").unwrap(),
        vec![Token::Ident("selection".into())]
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_lexer_literals() {
    assert_eq!(Lexer::tokenize("42").unwrap(), vec![Token::IntegerLit(42)]);
    assert_eq!(
        Lexer::tokenize("2.75").unwrap(),
        vec![Token::FloatLit(2.75)]
    );
    assert_eq!(
        Lexer::tokenize("TRUE false").unwrap(),
        vec![Token::BoolLit(true), Token::BoolLit(false)]
    );
    // SQL string escape is a doubled quote, not a backslash
    assert_eq!(
        Lexer::tokenize("'O''Brien'").unwrap(),
        vec![Token::StringLit("O'Brien".into())]
    );
    // case inside string literals is preserved
    assert_eq!(
        Lexer::tokenize("'MiXeD'").unwrap(),
        vec![Token::StringLit("MiXeD".into())]
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_lexer_maximal_munch_operators() {
    assert_eq!(
        Lexer::tokenize(">= <= <> != = < >").unwrap(),
        vec![
            Token::GtEq,
            Token::LtEq,
            Token::NotEq,
            Token::NotEq,
            Token::Eq,
            Token::Lt,
            Token::Gt
        ]
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_lexer_comments_and_whitespace() {
    let toks = Lexer::tokenize("SELECT -- the projection\n\t1").unwrap();
    assert_eq!(
        toks,
        vec![Token::Keyword(Keyword::Select), Token::IntegerLit(1)]
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_lexer_errors_never_panic() {
    for bad in ["'unterminated", "99999999999999999999999999", "!x", "a # b"] {
        assert!(
            Lexer::tokenize(bad).is_err(),
            "{bad:?} must produce Err, not panic"
        );
    }
}

// ------------------------------------------------------------- statements

#[test]
#[ignore = "milestone 3"]
fn ms3_create_table_exact_ast() {
    let got =
        parse_one("CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, age INTEGER)");
    assert_eq!(
        got,
        Statement::CreateTable {
            name: "users".into(),
            columns: vec![
                ColumnDef {
                    name: "id".into(),
                    data_type: DataType::Integer,
                    nullable: false, // PRIMARY KEY implies NOT NULL
                    primary_key: true,
                },
                ColumnDef {
                    name: "name".into(),
                    data_type: DataType::Text,
                    nullable: false,
                    primary_key: false,
                },
                ColumnDef {
                    name: "age".into(),
                    data_type: DataType::Integer,
                    nullable: true,
                    primary_key: false,
                },
            ],
        }
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_insert_multi_row() {
    let got = parse_one("INSERT INTO t (a, b) VALUES (1, 'x'), (2, NULL)");
    assert_eq!(
        got,
        Statement::Insert {
            table: "t".into(),
            columns: Some(vec!["a".into(), "b".into()]),
            rows: vec![
                vec![int(1), Expr::Literal(Value::Text("x".into()))],
                vec![int(2), Expr::Literal(Value::Null)],
            ],
        }
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_select_full_clause_set() {
    let got = parse_one(
        "SELECT id, name AS who FROM users WHERE age >= 18 ORDER BY age DESC, id LIMIT 10",
    );
    let Statement::Select(sel) = got else {
        panic!("expected a Select")
    };
    assert_eq!(
        sel.projection,
        vec![
            SelectItem::Expr {
                expr: col("id"),
                alias: None
            },
            SelectItem::Expr {
                expr: col("name"),
                alias: Some("who".into())
            },
        ]
    );
    assert_eq!(
        sel.from,
        vec![TableRef::Table {
            name: "users".into(),
            alias: None
        }]
    );
    assert_eq!(sel.filter, Some(bin(col("age"), BinaryOp::GtEq, int(18))));
    assert_eq!(
        sel.order_by,
        vec![(col("age"), true), (col("id"), false)],
        "DESC then default ASC"
    );
    assert_eq!(sel.limit, Some(10));
    assert!(sel.group_by.is_empty());
}

#[test]
#[ignore = "milestone 3"]
fn ms3_select_without_from_and_wildcard() {
    let Statement::Select(sel) = parse_one("SELECT 1 + 1") else {
        panic!()
    };
    assert!(sel.from.is_empty(), "FROM is optional");

    let Statement::Select(sel) = parse_one("SELECT * FROM t") else {
        panic!()
    };
    assert_eq!(sel.projection, vec![SelectItem::Wildcard]);
}

#[test]
#[ignore = "milestone 3"]
fn ms3_update_delete_txn_statements() {
    assert_eq!(
        parse_one("UPDATE t SET a = 1, b = b + 1 WHERE id = 3"),
        Statement::Update {
            table: "t".into(),
            set: vec![
                ("a".into(), int(1)),
                ("b".into(), bin(col("b"), BinaryOp::Add, int(1))),
            ],
            filter: Some(bin(col("id"), BinaryOp::Eq, int(3))),
        }
    );
    assert_eq!(
        parse_one("DELETE FROM t"),
        Statement::Delete {
            table: "t".into(),
            filter: None
        }
    );
    assert_eq!(parse_one("BEGIN"), Statement::Begin);
    assert_eq!(parse_one("COMMIT"), Statement::Commit);
    assert_eq!(parse_one("ROLLBACK"), Statement::Rollback);
}

#[test]
#[ignore = "milestone 3"]
fn ms3_scripts_and_semicolons() {
    let stmts = Parser::parse("BEGIN; COMMIT;").unwrap();
    assert_eq!(stmts, vec![Statement::Begin, Statement::Commit]);
    assert!(Parser::parse("").unwrap().is_empty());
    assert!(Parser::parse(" ; ; ").unwrap().is_empty());
}

// ------------------------------------------------------------ expressions

#[test]
#[ignore = "milestone 3"]
fn ms3_precedence_arithmetic() {
    // 1 + 2 * 3 ⇒ 1 + (2 * 3)
    assert_eq!(
        Parser::parse_expr("1 + 2 * 3").unwrap(),
        bin(int(1), BinaryOp::Add, bin(int(2), BinaryOp::Mul, int(3)))
    );
    // (1 + 2) * 3 — parentheses steer, then vanish
    assert_eq!(
        Parser::parse_expr("(1 + 2) * 3").unwrap(),
        bin(bin(int(1), BinaryOp::Add, int(2)), BinaryOp::Mul, int(3))
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_precedence_logic_and_comparison() {
    // a OR b AND c ⇒ a OR (b AND c)
    assert_eq!(
        Parser::parse_expr("a OR b AND c").unwrap(),
        bin(
            col("a"),
            BinaryOp::Or,
            bin(col("b"), BinaryOp::And, col("c"))
        )
    );
    // comparisons bind tighter than AND: a + 1 > b AND c ⇒ ((a+1) > b) AND c
    assert_eq!(
        Parser::parse_expr("a + 1 > b AND c").unwrap(),
        bin(
            bin(bin(col("a"), BinaryOp::Add, int(1)), BinaryOp::Gt, col("b")),
            BinaryOp::And,
            col("c")
        )
    );
    // NOT binds tighter than AND: NOT a AND b ⇒ (NOT a) AND b
    assert_eq!(
        Parser::parse_expr("NOT a AND b").unwrap(),
        bin(
            Expr::Unary(UnaryOp::Not, Box::new(col("a"))),
            BinaryOp::And,
            col("b")
        )
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_left_associativity() {
    // a - b - c ⇒ (a - b) - c
    assert_eq!(
        Parser::parse_expr("a - b - c").unwrap(),
        bin(
            bin(col("a"), BinaryOp::Sub, col("b")),
            BinaryOp::Sub,
            col("c")
        )
    );
}

#[test]
#[ignore = "milestone 3"]
fn ms3_unary_minus_is_null_cast_qualified() {
    assert_eq!(
        Parser::parse_expr("-x + 1").unwrap(),
        bin(
            Expr::Unary(UnaryOp::Neg, Box::new(col("x"))),
            BinaryOp::Add,
            int(1)
        )
    );
    assert_eq!(
        Parser::parse_expr("x IS NULL").unwrap(),
        Expr::IsNull(Box::new(col("x")), false)
    );
    assert_eq!(
        Parser::parse_expr("x IS NOT NULL").unwrap(),
        Expr::IsNull(Box::new(col("x")), true)
    );
    assert_eq!(
        Parser::parse_expr("CAST(age AS FLOAT)").unwrap(),
        Expr::Cast(Box::new(col("age")), DataType::Float)
    );
    assert_eq!(
        Parser::parse_expr("users.age").unwrap(),
        Expr::Column(Some("users".into()), "age".into())
    );
}

// --------------------------------------------------------- invalid corpus

#[test]
#[ignore = "milestone 3"]
fn ms3_invalid_sql_errors_not_panics() {
    let bad = [
        "SELECT",
        "SELECT FROM t",
        "CREATE TABLE t",
        "CREATE TABLE t (a NUMBERISH)",
        "INSERT INTO t VALUES",
        "INSERT INTO t (a VALUES (1)",
        "UPDATE t WHERE x = 1",
        "DELETE t",
        "SELECT * FROM t WHERE",
        "SELECT * FROM t ORDER LIMIT 5",
        "SELECT * FROM t LIMIT many",
        "1 + 2",
    ];
    for sql in bad {
        assert!(
            Parser::parse(sql).is_err(),
            "{sql:?} must be a parse error, not a panic or a success"
        );
    }
}

// -------------------------------------------------- Milestone 6 grammar

#[test]
#[ignore = "milestone 6"]
fn ms6_join_and_group_by_parse() {
    let Statement::Select(sel) =
        parse_one("SELECT c.name FROM customer AS c JOIN orders ON id = cust WHERE total > 0")
    else {
        panic!()
    };
    assert_eq!(sel.from.len(), 1, "a JOIN chain is one FROM entry");
    let TableRef::Join { kind, on, .. } = &sel.from[0] else {
        panic!("expected a join, got {:?}", sel.from[0])
    };
    assert_eq!(*kind, JoinKind::Inner);
    assert!(on.is_some());

    let Statement::Select(sel) = parse_one("SELECT nation, COUNT(*) FROM customer GROUP BY nation")
    else {
        panic!()
    };
    assert_eq!(sel.group_by, vec![col("nation")]);
    assert!(matches!(
        &sel.projection[1],
        SelectItem::Expr { expr: Expr::Function(name, _), .. } if name.eq_ignore_ascii_case("count")
    ));
}

#[test]
#[ignore = "milestone 6"]
fn ms6_cross_join_and_comma_from() {
    let Statement::Select(sel) = parse_one("SELECT 1 FROM a, b") else {
        panic!()
    };
    assert_eq!(sel.from.len(), 2, "comma FROM items stay separate entries");

    let Statement::Select(sel) = parse_one("SELECT 1 FROM a CROSS JOIN b") else {
        panic!()
    };
    let TableRef::Join { kind, on, .. } = &sel.from[0] else {
        panic!()
    };
    assert_eq!(*kind, JoinKind::Cross);
    assert!(on.is_none());
}
