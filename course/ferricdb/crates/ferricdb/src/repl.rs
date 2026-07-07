//! The interactive shell — **you build this in Milestone 0** and extend it
//! in nearly every milestone after (`.ast` and executable DDL in MS3, real
//! query results in MS4, transactions in MS8, `--serve` in MS10).

#![allow(unused_variables, dead_code)] // stubs: remove once you implement

use std::path::Path;

use ferricdb_common::Result;

/// The meta-commands, exactly as `.help` prints them. The smoke test in
/// `tests/smoke.rs` asserts all six are listed — keep it in sync if you
/// reword the help text.
pub const META_COMMANDS: &str = "\
.help              show this help
.exit              leave the shell
.tables            list tables (reads the catalog — Milestone 3)
.schema <table>    show a table's columns and types (Milestone 3)
.ast <sql>         parse a statement and pretty-print its AST (Milestone 3)
.open <path>       switch to another database file (Milestone 1)
";

/// Interactive shell: reads SQL lines and meta-commands, prints result
/// tables.
///
/// Behavior contract (Milestone 0):
///
/// * lines starting with `.` are meta-commands, dispatched before any SQL
///   handling; unknown ones print an error and point at `.help`;
/// * statements buffer across lines until a terminating `;` (show a
///   continuation prompt), then — until Milestone 4 — are echoed back;
/// * `.exit`, Ctrl-D (EOF), and Ctrl-C at an empty prompt end the session
///   cleanly; Ctrl-C at a continuation prompt clears the buffered
///   statement instead.
pub struct Repl {/* ExecContext + rustyline editor — your fields here */}

impl Repl {
    /// Open (or create) the database at `db_path` (in-memory tempfile when
    /// `None`) and set up the line editor.
    pub fn new(db_path: Option<&Path>) -> Result<Self> {
        todo!("Milestone 0")
    }

    /// The read–dispatch–print loop. Returns when the session ends.
    pub fn run(&mut self) -> Result<()> {
        todo!("Milestone 0")
    }
}
