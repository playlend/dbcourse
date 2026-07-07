//! Non-ignored smoke test: runs green from day zero.

use ferricdb::repl::META_COMMANDS;

#[test]
fn help_lists_all_six_meta_commands() {
    for cmd in [".help", ".exit", ".tables", ".schema", ".ast", ".open"] {
        assert!(
            META_COMMANDS.contains(cmd),
            "META_COMMANDS must document {cmd}"
        );
    }
}
