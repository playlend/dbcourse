//! FerricDB entry point.
//!
//! Usage: `ferricdb [--db <path>] [--serve [addr]]`
//!
//! Flags are parsed by hand (the course crate policy has no room for an
//! argument-parsing dependency, and two flags don't need one). `--serve`
//! comes alive in Milestone 10.

use std::path::PathBuf;
use std::process::ExitCode;

use ferricdb::repl::Repl;

fn main() -> ExitCode {
    let mut db_path: Option<PathBuf> = None;
    let mut serve: Option<String> = None;

    let mut args = std::env::args().skip(1).peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--db" => match args.next() {
                Some(p) => db_path = Some(PathBuf::from(p)),
                None => return usage("--db needs a path"),
            },
            "--serve" => {
                // optional address; only consume the next arg if it isn't another flag
                let addr = match args.peek() {
                    Some(next) if !next.starts_with('-') => args.next().unwrap(),
                    _ => "127.0.0.1:5433".to_string(),
                };
                serve = Some(addr);
            }
            "--help" | "-h" => {
                println!("ferricdb [--db <path>] [--serve [addr]]");
                return ExitCode::SUCCESS;
            }
            other => return usage(&format!("unknown flag: {other}")),
        }
    }

    if let Some(addr) = serve {
        // Milestone 10: build the ExecContext, construct
        // ferricdb_server::Server with this address, and run it on a tokio
        // runtime. Until then:
        eprintln!("--serve {addr}: implemented in Milestone 10");
        return ExitCode::FAILURE;
    }

    match Repl::new(db_path.as_deref()).and_then(|mut r| r.run()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ferricdb: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage(msg: &str) -> ExitCode {
    eprintln!("ferricdb: {msg}\nusage: ferricdb [--db <path>] [--serve [addr]]");
    ExitCode::FAILURE
}
