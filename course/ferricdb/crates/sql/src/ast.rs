//! The FerricDB abstract syntax tree — **provided in full**.
//!
//! These types are the contract between the parser (this crate), the
//! binder/planner (`ferricdb-exec`), and every milestone page that quotes
//! them. Do not change their shape; the provided parser tests assert exact
//! trees against them.
//!
//! The AST is purely *syntactic*: no page ids, no column indexes, no types
//! on expressions. `SELECT ghost FROM nowhere` parses fine — rejecting it
//! is the binder's job (Milestone 3, `ferricdb-exec`).

use ferricdb_common::{DataType, Value};

/// One parsed SQL statement.
///
/// Everything the course grammar will ever contain is declared up front —
/// the parser grows into it milestone by milestone (MS3 core, MS5
/// `CreateIndex`, MS6 joins/aggregates inside [`Select`], MS7
/// `Analyze`/`Explain`, MS8 `Vacuum`).
#[derive(Clone, Debug, PartialEq)]
pub enum Statement {
    /// `CREATE TABLE name (col type [NOT NULL] [PRIMARY KEY], …)`
    CreateTable {
        name: String,
        columns: Vec<ColumnDef>,
    },
    /// `DROP TABLE name`
    DropTable { name: String },
    /// `INSERT INTO table [(columns…)] VALUES (exprs…) [, (exprs…)…]`
    Insert {
        table: String,
        /// Explicit column list, if given; `None` = all columns in
        /// declaration order.
        columns: Option<Vec<String>>,
        /// One entry per `VALUES` row.
        rows: Vec<Vec<Expr>>,
    },
    /// `SELECT …` (boxed: [`Select`] is by far the largest node).
    Select(Box<Select>),
    /// `UPDATE table SET col = expr [, …] [WHERE expr]`
    Update {
        table: String,
        set: Vec<(String, Expr)>,
        filter: Option<Expr>,
    },
    /// `DELETE FROM table [WHERE expr]`
    Delete { table: String, filter: Option<Expr> },
    /// `BEGIN` — parsed in MS3, executed in MS8.
    Begin,
    /// `COMMIT`
    Commit,
    /// `ROLLBACK`
    Rollback,
    /// `CREATE INDEX name ON table (column)` — Milestone 5.
    CreateIndex {
        name: String,
        table: String,
        column: String,
    },
    /// `ANALYZE [table]` — Milestone 7. `None` = every table.
    Analyze { table: Option<String> },
    /// `EXPLAIN [ANALYZE] statement` — Milestone 7.
    Explain {
        /// `EXPLAIN ANALYZE` really executes the statement.
        analyze: bool,
        inner: Box<Statement>,
    },
    /// `VACUUM` — Milestone 8.
    Vacuum,
}

/// One column in a `CREATE TABLE`.
///
/// Type-name mapping (case-insensitive): `BOOLEAN` → [`DataType::Boolean`],
/// `INTEGER` → [`DataType::Integer`], `FLOAT` → [`DataType::Float`],
/// `TEXT` → [`DataType::Text`].
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnDef {
    pub name: String,
    pub data_type: DataType,
    /// `false` iff declared `NOT NULL` (or `PRIMARY KEY`, which implies it).
    pub nullable: bool,
    /// Declared `PRIMARY KEY`. At most one column per table.
    pub primary_key: bool,
}

/// The body of a `SELECT` statement.
#[derive(Clone, Debug, PartialEq)]
pub struct Select {
    /// The projection list: expressions with optional aliases, or `*`.
    pub projection: Vec<SelectItem>,
    /// `FROM` sources. Empty for table-less selects (`SELECT 1 + 1`);
    /// multiple entries are an implicit cross join (`FROM a, b`);
    /// explicit joins nest inside [`TableRef::Join`] (Milestone 6).
    pub from: Vec<TableRef>,
    /// `WHERE`
    pub filter: Option<Expr>,
    /// `GROUP BY` — Milestone 6.
    pub group_by: Vec<Expr>,
    /// `ORDER BY expr [ASC|DESC], …` — `true` = descending.
    pub order_by: Vec<(Expr, bool)>,
    /// `LIMIT n`
    pub limit: Option<u64>,
}

/// One entry in a projection list.
#[derive(Clone, Debug, PartialEq)]
pub enum SelectItem {
    /// `*`
    Wildcard,
    /// `expr [AS alias]`
    Expr { expr: Expr, alias: Option<String> },
}

/// A `FROM` source.
#[derive(Clone, Debug, PartialEq)]
pub enum TableRef {
    /// `name [AS alias]`
    Table { name: String, alias: Option<String> },
    /// `left JOIN right ON expr` / `left CROSS JOIN right` — Milestone 6.
    Join {
        left: Box<TableRef>,
        right: Box<TableRef>,
        kind: JoinKind,
        /// `ON` predicate; `None` for CROSS JOIN.
        on: Option<Expr>,
    },
}

/// Join flavors in the course grammar. (No outer joins — an honest scope
/// cut; see Module 7's "Go deeper".)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinKind {
    Inner,
    Cross,
}

/// An expression tree.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// A literal: `42`, `1.5`, `'text'`, `TRUE`, `NULL`.
    Literal(Value),
    /// A column reference: `age` or qualified `users.age` —
    /// `(qualifier, name)`.
    Column(Option<String>, String),
    /// `NOT expr` / `-expr`
    Unary(UnaryOp, Box<Expr>),
    /// `left op right`
    Binary(Box<Expr>, BinaryOp, Box<Expr>),
    /// `expr IS NULL` (`negated = false`) / `expr IS NOT NULL` (`true`).
    IsNull(Box<Expr>, bool),
    /// `CAST(expr AS type)`
    Cast(Box<Expr>, DataType),
    /// A function call: `COUNT(*)` is `Function("COUNT", [Literal(…)])`
    /// by convention `COUNT(*)` carries a single `Literal(Integer(1))`
    /// argument. Aggregates arrive in Milestone 6.
    Function(String, Vec<Expr>),
}

/// Prefix operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    /// `NOT` (binding power 3)
    Not,
    /// unary `-` (binding power 7)
    Neg,
}

/// Infix operators, listed with their Pratt binding powers (low binds
/// loosest; recurse with `power + 1` for left associativity):
///
/// | power | operators |
/// |-------|-----------|
/// | 1 | `OR` |
/// | 2 | `AND` |
/// | 4 | `= != <> < <= > >=` |
/// | 5 | `+ -` |
/// | 6 | `* / %` |
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    And,
    Or,
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}
