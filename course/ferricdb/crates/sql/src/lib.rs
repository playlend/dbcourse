//! # ferricdb-sql — the SQL front end (Milestone 3)
//!
//! In this crate you build the pipeline that turns SQL text into an
//! abstract syntax tree:
//!
//! ```text
//! "SELECT * FROM users"  --Lexer-->  [Token]  --Parser-->  ast::Statement
//! ```
//!
//! Three pieces, all Milestone 3:
//!
//! 1. **[`Lexer`]** — a hand-written tokenizer producing [`Token`]s
//!    (keywords, identifiers, literals, operators, punctuation).
//! 2. **[`ast`]** — the AST types (provided in full; you do not change
//!    their shape — milestone pages and the binder in `ferricdb-exec`
//!    depend on it exactly as declared).
//! 3. **[`Parser`]** — a recursive-descent statement parser combined with
//!    a Pratt (precedence-climbing) expression parser.
//!
//! Name resolution ("does table `users` exist? what type is column
//! `age`?") is *not* done here — that is the binder's job in
//! `ferricdb-exec`. This crate is purely syntactic and depends only on
//! `ferricdb-common` (for [`Value`], [`DataType`], and [`Error::Parse`]).
//!
//! Grammar scope grows with the course: Milestone 3 covers CREATE/DROP
//! TABLE, INSERT, single-table SELECT with WHERE/ORDER BY/LIMIT, UPDATE,
//! DELETE, and BEGIN/COMMIT/ROLLBACK. Later milestones extend the same
//! parser: CREATE INDEX (MS5), JOIN/GROUP BY/aggregates (MS6),
//! ANALYZE/EXPLAIN (MS7), VACUUM (MS8). The AST already contains every
//! node you will ever need, so you only ever touch the parser.
//!
//! Escape hatch (course policy): if you get stuck on parsing, you may
//! temporarily swap in the `sqlparser` crate behind the same [`Parser`]
//! API — but the hand-written parser is the learning goal, and the
//! provided tests pin its exact output.
//!
//! [`Error::Parse`]: ferricdb_common::Error::Parse
//! [`Value`]: ferricdb_common::Value
//! [`DataType`]: ferricdb_common::DataType

// Remove this once you start implementing.
#![allow(unused_variables, dead_code)]

pub mod ast;

use ferricdb_common::Result;

/// A reserved SQL word, matched case-insensitively by the lexer
/// (`SELECT`, `select`, and `SeLeCt` all lex to `Keyword::Select`).
///
/// `TRUE` and `FALSE` are deliberately *not* keywords — the lexer turns
/// them directly into [`Token::BoolLit`]. `NULL` stays a keyword because
/// it plays two grammatical roles (the literal `NULL` and the `IS [NOT]
/// NULL` / `NOT NULL` constructs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    // -- statements & clauses ------------------------------------------
    /// `SELECT`
    Select,
    /// `FROM`
    From,
    /// `WHERE`
    Where,
    /// `GROUP` (as in `GROUP BY`, Milestone 6)
    Group,
    /// `ORDER` (as in `ORDER BY`)
    Order,
    /// `BY`
    By,
    /// `ASC`
    Asc,
    /// `DESC`
    Desc,
    /// `LIMIT`
    Limit,
    /// `INSERT`
    Insert,
    /// `INTO`
    Into,
    /// `VALUES`
    Values,
    /// `UPDATE`
    Update,
    /// `SET`
    Set,
    /// `DELETE`
    Delete,
    /// `CREATE`
    Create,
    /// `DROP`
    Drop,
    /// `TABLE`
    Table,
    /// `INDEX` (Milestone 5)
    Index,
    /// `ON` (join conditions and `CREATE INDEX ... ON`)
    On,
    /// `PRIMARY` (as in `PRIMARY KEY`)
    Primary,
    /// `KEY`
    Key,
    /// `AS` (aliases and `CAST(expr AS type)`)
    As,
    /// `JOIN` (Milestone 6)
    Join,
    /// `INNER` (Milestone 6)
    Inner,
    /// `CROSS` (Milestone 6)
    Cross,
    /// `BEGIN`
    Begin,
    /// `COMMIT`
    Commit,
    /// `ROLLBACK`
    Rollback,
    /// `ANALYZE` (Milestone 7; also `EXPLAIN ANALYZE`)
    Analyze,
    /// `EXPLAIN` (Milestone 7)
    Explain,
    /// `VACUUM` (Milestone 8)
    Vacuum,
    // -- expression keywords -------------------------------------------
    /// `NOT`
    Not,
    /// `NULL`
    Null,
    /// `AND`
    And,
    /// `OR`
    Or,
    /// `IS`
    Is,
    /// `CAST`
    Cast,
    // -- type names (see the mapping on `ast::ColumnDef`) ---------------
    /// `BOOLEAN`
    Boolean,
    /// `INT` (synonym of `INTEGER`)
    Int,
    /// `INTEGER`
    Integer,
    /// `FLOAT`
    Float,
    /// `DOUBLE` (synonym of `FLOAT`)
    Double,
    /// `TEXT`
    Text,
    /// `VARCHAR` (synonym of `TEXT`; no length parameter)
    Varchar,
}

/// One lexical token produced by [`Lexer::tokenize`].
///
/// Tokens carry no source positions in this course (error messages quote
/// the offending text instead — adding spans is a nice stretch goal).
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A reserved word — see [`Keyword`]. Matched case-insensitively.
    Keyword(Keyword),
    /// An identifier: `[a-zA-Z_][a-zA-Z0-9_]*` that is not a keyword.
    /// Normalized to lowercase (PostgreSQL folds unquoted identifiers;
    /// quoted identifiers are not supported in this course).
    Ident(String),
    /// A single-quoted string literal, e.g. `'hello'`. A doubled quote
    /// `''` inside the literal escapes to a single `'`. Case inside the
    /// quotes is preserved.
    StringLit(String),
    /// An integer literal: one or more ASCII digits. A leading `-` is
    /// never part of the literal — it lexes as [`Token::Minus`] and the
    /// parser applies it as unary negation.
    IntegerLit(i64),
    /// A float literal: digits, a `.`, then digits (e.g. `3.14`). No
    /// exponent form, no leading/trailing bare dot.
    FloatLit(f64),
    /// `TRUE` or `FALSE` (case-insensitive).
    BoolLit(bool),
    // -- operators -------------------------------------------------------
    /// `=`
    Eq,
    /// `<>` or `!=` (both spellings lex to this one token)
    NotEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `+`
    Plus,
    /// `-` (binary subtraction *and* unary negation — the parser decides)
    Minus,
    /// `*` — multiplication, the `SELECT *` wildcard, and `COUNT(*)`
    Star,
    /// `/`
    Slash,
    /// `%` (remainder)
    Percent,
    // -- punctuation -----------------------------------------------------
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `.` (qualified column names: `alias.column`)
    Dot,
}

/// The hand-written tokenizer. Stateless — [`Lexer::tokenize`] is an
/// associated function.
pub struct Lexer;

impl Lexer {
    /// Convert SQL source text into a token stream.
    ///
    /// Lexical rules (the Milestone 3 tests pin these down):
    ///
    /// - Whitespace (space, tab, newline, `\r`) separates tokens and is
    ///   otherwise discarded.
    /// - `--` starts a line comment: everything up to the next newline
    ///   (or end of input) is discarded.
    /// - Keywords are matched case-insensitively ([`Keyword`]).
    /// - Non-keyword words `[a-zA-Z_][a-zA-Z0-9_]*` become
    ///   [`Token::Ident`], lowercased.
    /// - `TRUE`/`FALSE` become [`Token::BoolLit`]; `NULL` stays
    ///   [`Keyword::Null`].
    /// - `'...'` becomes [`Token::StringLit`] with `''` unescaped to `'`.
    /// - Digit runs become [`Token::IntegerLit`]; `digits.digits` becomes
    ///   [`Token::FloatLit`].
    /// - Operators and punctuation per the [`Token`] variants; two-char
    ///   operators (`<=`, `>=`, `<>`, `!=`) lex greedily.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](ferricdb_common::Error::Parse) for an
    /// unterminated string literal, an integer literal that overflows
    /// `i64`, a lone `!` not followed by `=`, or any character that fits
    /// no rule (e.g. `#`, `@`). Never panics on any input.
    pub fn tokenize(input: &str) -> Result<Vec<Token>> {
        todo!("Milestone 3")
    }
}

/// Recursive-descent statement parser + Pratt expression parser.
///
/// Stateless from the outside — both entry points are associated
/// functions that internally tokenize with [`Lexer::tokenize`] first.
///
/// Grammar (EBNF-ish; `[...]` optional, `{...}` repeated, `|` choice).
/// Productions marked with a milestone are parsed starting in that
/// milestone; everything unmarked is Milestone 3:
///
/// ```text
/// script       := [ statement { ";" statement } ] [ ";" ]
/// statement    := create_table | drop_table | create_index
///               | insert | select | update | delete
///               | "BEGIN" | "COMMIT" | "ROLLBACK"
///               | analyze | explain | "VACUUM"                    (MS8)
///
/// create_table := "CREATE" "TABLE" ident "(" column_def { "," column_def } ")"
/// column_def   := ident type { "PRIMARY" "KEY" | "NOT" "NULL" | "NULL" }
/// type         := "BOOLEAN" | "INT" | "INTEGER" | "FLOAT" | "DOUBLE"
///               | "TEXT" | "VARCHAR"
/// drop_table   := "DROP" "TABLE" ident
/// create_index := "CREATE" "INDEX" ident "ON" ident "(" ident ")"  (MS5)
///
/// insert       := "INSERT" "INTO" ident [ "(" ident { "," ident } ")" ]
///                 "VALUES" row { "," row }
/// row          := "(" expr { "," expr } ")"
///
/// select       := "SELECT" select_item { "," select_item }
///                 [ "FROM" table_ref { "," table_ref } ]
///                 [ "WHERE" expr ]
///                 [ "GROUP" "BY" expr { "," expr } ]              (MS6)
///                 [ "ORDER" "BY" order_key { "," order_key } ]
///                 [ "LIMIT" integer_literal ]
/// select_item  := "*" | expr [ "AS" ident ]
/// order_key    := expr [ "ASC" | "DESC" ]
/// table_ref    := table { join_clause }                           (joins MS6)
/// table        := ident [ [ "AS" ] ident ]
/// join_clause  := [ "INNER" ] "JOIN" table "ON" expr              (MS6)
///               | "CROSS" "JOIN" table                            (MS6)
///
/// update       := "UPDATE" ident "SET" ident "=" expr
///                 { "," ident "=" expr } [ "WHERE" expr ]
/// delete       := "DELETE" "FROM" ident [ "WHERE" expr ]
/// analyze      := "ANALYZE" [ ident ]                             (MS7)
/// explain      := "EXPLAIN" [ "ANALYZE" ] statement               (MS7)
/// ```
///
/// Notes the tests rely on:
/// - `FROM` is optional: `SELECT 1 + 1` parses with an empty `from` vec.
/// - A missing `WHERE` yields `filter: None`; `ORDER BY` defaults to
///   ascending (`desc == false`); `LIMIT` takes a plain non-negative
///   integer literal.
/// - `PRIMARY KEY` implies `nullable: false`; column options may appear
///   in any order after the type.
/// - Comma-separated `FROM` items each become their own entry in
///   `Select::from` (the planner treats multiple entries as a cross
///   product); `JOIN` chains nest left-associatively inside one entry.
pub struct Parser;

impl Parser {
    /// Parse a script of `;`-separated statements into a list of
    /// [`ast::Statement`]s. A trailing semicolon is allowed; empty
    /// statements (stray semicolons, all-whitespace input) produce no
    /// entry, so empty input yields an empty vec.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Parse`](ferricdb_common::Error::Parse) — never
    /// panics — on any lexical or syntactic problem, with a message that
    /// names what was expected and what was found.
    pub fn parse(sql: &str) -> Result<Vec<ast::Statement>> {
        todo!("Milestone 3")
    }

    /// Parse a single standalone expression (test convenience; also used
    /// by the REPL's `.ast` meta-command). Trailing unconsumed tokens are
    /// an error.
    ///
    /// Operator precedence, lowest binding to tightest binding (modeled
    /// on PostgreSQL's table):
    ///
    /// ```text
    /// 1. OR                                  (left-associative)
    /// 2. AND                                 (left-associative)
    /// 3. NOT                                 (prefix)
    /// 4. IS NULL / IS NOT NULL               (postfix)
    /// 5. =  <>  !=  <  <=  >  >=             (left-associative)
    /// 6. +  -                                (left-associative)
    /// 7. *  /  %                             (left-associative)
    /// 8. - (unary negation)                  (prefix)
    /// 9. primary: literal, [qualifier.]column, CAST(expr AS type),
    ///    function(args), COUNT(*), (expr)
    /// ```
    ///
    /// So `1 + 2 * 3` is `1 + (2 * 3)`, `a OR b AND c` is
    /// `a OR (b AND c)`, `NOT a AND b` is `(NOT a) AND b`, and
    /// `-2 * 3` is `(-2) * 3`. Function names are folded to lowercase and
    /// `COUNT(*)` parses as `Function("count", vec![])`.
    pub fn parse_expr(sql: &str) -> Result<ast::Expr> {
        todo!("Milestone 3")
    }
}
