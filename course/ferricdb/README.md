# FerricDB

FerricDB is a **toy PostgreSQL clone written in Rust**, built incrementally as
the project track of the database course (start at `course/index.html`). Over
eleven milestones you take it from an empty REPL to a multi-version,
crash-recoverable SQL database that real Postgres clients (`psql`) can talk to
over the wire.

This repository is the **starter workspace**: the public API of every crate is
fully declared — real structs, real fields, real trait definitions, rustdoc on
everything — but the method bodies are `todo!("Milestone N")` stubs. You supply
the algorithms; the provided test suites tell you when you got them right. The
model is the same as CMU's BusTub: shape is given, behavior is yours.

The single source of truth for every public type and signature is
[`docs/api-spec.md`](docs/api-spec.md) in this workspace.
Milestone pages quote it verbatim; if a page and the spec disagree, the spec
wins.

## Workspace tour

| Crate | Package | What you build | Milestone | Concept module |
|---|---|---|---|---|
| `crates/common` | `ferricdb-common` | Shared types: `Value`, `Schema`, `RecordId`, `TupleMeta`, `Error` — **provided fully implemented** | — (used from 1) | m01, m02 |
| `crates/storage` | `ferricdb-storage` | Slotted heap pages, tuple (de)serialization, disk manager, heap files | 1 | m02-storage |
| `crates/buffer` | `ferricdb-buffer` | Buffer pool manager, clock replacer (LRU-K as stretch), RAII pin guards | 2 | m03-buffer-pool |
| `crates/sql` | `ferricdb-sql` | Lexer, AST, recursive-descent statement parser + Pratt expression parser | 3 | m05-sql-parsing |
| `crates/catalog` | `ferricdb-catalog` | Self-hosted system catalog (stored in its own heap pages); table stats | 3, 7 | m05, m08 |
| `crates/exec` | `ferricdb-exec` | Binder (3), planner + Volcano executors (4), joins/aggregation/external sort (6), cost-based optimizer (7) | 3, 4, 6, 7 | m06, m07, m08 |
| `crates/index` | `ferricdb-index` | Disk-resident B+Tree index with range scans | 5 | m04-data-structures-indexes |
| `crates/txn` | `ferricdb-txn` | MVCC snapshots, visibility rules, transaction manager, first-committer-wins | 8 | m09-transactions-mvcc |
| `crates/wal` | `ferricdb-wal` | Write-ahead log and ARIES-style recovery (analysis / redo / undo) | 9 | m10-logging-recovery |
| `crates/server` | `ferricdb-server` | PostgreSQL frontend/backend protocol v3 server on tokio | 10 | m11-wire-protocol-server |
| `crates/ferricdb` | `ferricdb` (binary) | REPL, meta-commands, sqllogictest harness — grows every milestone | 0+ | m00-orientation |

## What is provided vs. what you implement

**Provided (do not rewrite):**

- `crates/common` — fully implemented shared plumbing. It is infrastructure,
  not learning content.
- Every integration and unit **test suite** (under each crate's `tests/` and
  `#[cfg(test)]` modules). Tests encode the expected behavior.
- The **sqllogictest files** in `crates/ferricdb/tests/slt/msN_*.slt` and the
  harness that runs them.
- CI configuration, this README, and the workspace `Cargo.toml`.

**You implement:** everything else. Every stubbed body is marked
`todo!("Milestone N")` so you always know which milestone owns it. Each crate
root also carries `#![allow(unused_variables, dead_code)]` — remove that line
once you start implementing so the compiler starts helping you again.

## The ignored-test workflow

All milestone tests are `#[ignore = "milestone N"]` and named with an `msN_`
prefix (e.g. `ms1_slotted_page_roundtrip`, `ms2_clock_evicts_unpinned`... see
the spec for the canonical names). That means a **fresh clone is green**:

```console
$ cargo test --workspace        # passes: milestone tests are skipped
```

When you start milestone N, opt in to exactly that group:

```console
$ cargo test --workspace -- --include-ignored msN_

# usually you scope it to the crate you are working in:
$ cargo test -p ferricdb-storage -- --include-ignored ms1_
$ cargo test -p ferricdb-buffer  -- --include-ignored ms2_

# sqllogictest files run through the harness test from milestone 4 on:
$ cargo test -p ferricdb -- --include-ignored slt_all
```

Do not delete the `#[ignore]` attributes; `--include-ignored` keeps earlier
milestones' tests running as regression checks while leaving CI green for
classmates who are not there yet.

## Crate policy

Core algorithms are implemented **by hand** — that is the whole point. The
only external crates allowed are the ones already declared in the workspace
`Cargo.toml`: `thiserror`, `bytes`, `rustyline` (REPL), `tokio` (server crate
only), and as dev-dependencies `tempfile`, `rand` (fixed-seed property tests),
`sqllogictest` (harness), `criterion` (benches). `sqlparser` is permitted
solely as the documented Milestone 3 escape hatch and is not a default
dependency. Do not add anything else.

### Attribution

- [toyDB](https://github.com/erikgrinaker/toydb) and
  [mini-lsm](https://github.com/skyzh/mini-lsm) are **Apache-2.0**. You may
  study them, and if you adapt code from either, you must retain the license
  notice and add a comment attributing the source.
- **BusTub solution code must never be copied.** Publishing or copying CMU
  15-445 solutions violates that course's academic-integrity policy, and this
  course's too. Reading the BusTub *starter* for inspiration is fine; its
  solutions are off-limits.

## CI

`.github/workflows/ci.yml` runs on every push and pull request:

1. `cargo fmt --all --check` — formatting.
2. `cargo clippy --workspace --all-targets` — lints (`todo!()` stubs are
   explicitly allowed, so the starter is clippy-clean).
3. `cargo check --workspace --all-targets` — everything, including all test
   files, must **compile** at all times.
4. `cargo test --workspace` — runs the non-ignored tests only.

Because milestone tests are opt-in via `--include-ignored`, CI stays green
whether you are on milestone 1 or milestone 10. Your progress is measured
locally (and by whatever grading harness your course run uses), not by CI.

## Getting started

```console
$ cargo run -p ferricdb            # milestone 0: bring the REPL up
$ cargo test --workspace           # sanity: green on a fresh clone
$ cargo test -p ferricdb-storage -- --include-ignored ms1_   # begin milestone 1
```

Then open `course/index.html`, read the Milestone 0 page, and work forward.
Keep [`docs/api-spec.md`](docs/api-spec.md) open in a tab —
it is the contract every test compiles against.

## License

MIT — see [LICENSE](LICENSE).
