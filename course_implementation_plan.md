# Database Systems Curriculum + "Toy Postgres in Rust" Project Plan

This document is a build specification intended to be handed to Claude Code.
It defines (1) a concept curriculum delivered as interactive HTML pages, and
(2) a milestone-based project track in which the learner builds a toy version
of PostgreSQL in Rust. Every lecture page must be grounded in the cited
primary sources listed per module, and every factual claim on a page must
carry an inline citation that links to one of those sources.

---

## 0. Build Progress (updated as sections complete)

- [x] **Section A — Scaffolding** (done 2026-07-05): `course/assets/`
      (style.css design system with dark/light + print, site.js runtime:
      theme/quizzes/tabs/stepper, arch.js parametric architecture diagram
      shared by M00 + all milestone pages), `course/index.html` (syllabus,
      interactive recommended-path map, module/project card maps),
      `authoring/CONVENTIONS.md` (page contract + citation allowlist),
      `authoring/templates/` (module + milestone page templates),
      `authoring/api-spec.md` (FerricDB public API — single source of truth
      for milestone pages and the Rust workspace; adds a `crates/common`
      foundation crate to the Section 5 layout).
- [x] **Section B — Concept modules M00–M12** (13/13 done 2026-07-05).
      m00/m01/m02/m06/m07 by workflow agents (content unreviewed by a second
      pass); m03/m04/m05/m08/m09/m10/m11/m12 inline, grounded in fetched
      sources, with all interactive-widget algorithms behavior-tested in
      node (B+Tree invariants, MVCC 10-rule visibility, ARIES 3-pass sim,
      Selinger DP, Pratt parser, eviction policies). All 30 course pages
      pass `authoring/qa_check.py` CLEAN (links, citations, structure,
      offline rule).
- [x] **Section C — Project track pages** (16/16 done 2026-07-05).
      milestone-01/-05/-06/-09, stretch-s1 by workflow agents; the rest
      inline against `authoring/api-spec.md`.
- [x] **Section D — FerricDB Rust starter workspace** (done 2026-07-06).
      All 11 crates real: `common` fully implemented (12/12 unit tests
      green); storage/buffer/index/sql/catalog/exec/txn/wal/server all
      stubbed to `authoring/api-spec.md` with rustdoc'd public APIs and
      `todo!("Milestone N")` bodies; ferricdb bin (REPL stub, hand-rolled
      flag parsing, lib+bin split); sqllogictest harness + 2 hand-verified
      .slt files (ms4 CRUD, ms6 joins/agg); criterion bench skeleton; CI
      (fmt/clippy/check/test), README, CONTRIBUTING. Status:
      `cargo check --all-targets` green, `cargo clippy --all-targets`
      ZERO warnings, `cargo test` 13 passed / 92 ignored milestone tests
      (all compiling), `cargo fmt` clean. Required api-spec test names all
      present (ms1_slotted_page_roundtrip, ms2_clock_evicts_unpinned,
      ms5_btree_invariants_fuzz, ms8_write_skew_is_documented,
      ms9_crash_injection_no_lost_commits). NOTE: api-spec `PageStore`
      closures were changed to unit-returning (object safety); api-spec.md
      + milestone-01 page updated to match.
- [x] **Section E — QA pass** (done 2026-07-06). Mechanical:
      `authoring/qa_check.py` CLEAN over all 30 pages (structure,
      citations, offline rule, internal links, quizzes); `--net` all 130
      external URLs live; prev/next chain matches the recommended path
      exactly; widget JS syntax- and behavior-tested in node (B+Tree
      invariants, MVCC 10-rule visibility, ARIES 3-pass sim, Selinger DP,
      Pratt parser, eviction policies). Content: the 10 workflow-authored
      pages (m00, m01, m02, m06, m07, milestone-01/-05/-06/-09,
      stretch-s1) each got a dedicated verify agent (wf_63dfa478-91b) —
      all 10 PASS, 20 fixes applied (citation re-anchoring, an anchor
      fragment, one garbled sentence, api-spec alignment), ~40 factual
      claims spot-checked against fetched primary sources, zero remaining
      issues.

**BUILD COMPLETE (2026-07-06).** All five sections done. Not yet committed
to git (commit when asked). Known deliberate deviations from this plan:
`crates/common` added to the §5 workspace layout (documented in
api-spec.md and the project overview page); `PageStore` closures are
unit-returning for object safety (api-spec + MS1 page match the code);
CMU 15-445 links use fall2025 URLs (current-semester redirect noted in
§7.6 still applies to the bare course-home link).

**Publishing prep (2026-07-06):** the API spec moved into the workspace
(`course/ferricdb/docs/api-spec.md`; `authoring/api-spec.md` is now a
pointer) so the starter repo is self-contained. Publishing model: this
monorepo stays the source of truth; the learner-facing starter repo is
produced with `git subtree split --prefix=course/ferricdb` (publish as a
GitHub *template* repo; solutions branch lives only there or private).
The course site deploys to GitHub Pages via `.github/workflows/pages.yml`
(uploads `course/` minus `ferricdb/`; repo setting: Pages → Source =
GitHub Actions).

## 1. Goals and Design Principles

1. Teach database internals the way top university courses do (CMU 15-445,
   Berkeley CS186), but organized around building a working system.
2. Deliver lectures as standalone HTML pages (no slides). Each page should
   use diagrams (SVG), small interactive widgets (e.g. a B+Tree insert
   visualizer, a buffer pool eviction simulator), worked examples, and
   short self-check quizzes.
3. Pair every concept module with a project milestone so theory is applied
   immediately. This mirrors the structure of CMU 15-445, where lectures on
   buffer pools, B+Trees, execution, and concurrency control map directly to
   projects on the BusTub educational DBMS.
4. All content must be sourced from the reference list in Section 6. Do not
   invent facts, numbers, or algorithm details; when a page explains an
   algorithm (e.g. ARIES, LRU-K, Selinger optimization), it must follow the
   cited paper or official documentation and link to it.
5. The toy database ("FerricDB" as a working name; rename freely) is
   Postgres-flavored: heap files with slotted pages, a cost-aware planner,
   MVCC snapshot isolation, write-ahead logging, and the PostgreSQL wire
   protocol so real clients (psql) can connect.

## 2. Intended Audience and Prerequisites

- Comfortable programmers who know SQL basics and want to learn internals.
- Rust proficiency at the level of "The Rust Programming Language" book
  (ownership, traits, error handling, basic concurrency). Module 0 includes
  a Rust refresher reading list but does not teach Rust from scratch.
- No prior database internals knowledge assumed.

## 3. Deliverable Structure for Claude Code

```
course/
  index.html                  # course home: syllabus, module map, project map
  assets/                     # shared css, js, svg diagram helpers
  modules/
    m00-orientation/index.html
    m01-relational-model-sql/index.html
    m02-storage/index.html
    m03-buffer-pool/index.html
    m04-data-structures-indexes/index.html
    m05-sql-parsing/index.html
    m06-query-execution/index.html
    m07-joins-sorting-aggregation/index.html
    m08-query-optimization/index.html
    m09-transactions-mvcc/index.html
    m10-logging-recovery/index.html
    m11-wire-protocol-server/index.html
    m12-advanced-topics/index.html
  project/
    index.html                # project overview, architecture diagram
    milestone-00 ... milestone-10/index.html
    stretch-s1 ... stretch-s4/index.html   # optional stretch milestones
  ferricdb/                   # the Rust workspace (cargo)
    crates/...
```

HTML page requirements:
- Semantic HTML, a shared stylesheet, dark/light support, print-friendly.
- No build step and no runtime network dependencies: every page must render
  fully when opened via file:// (all CSS/JS/SVG local, no CDNs, no web
  fonts). External URLs appear only as citation/reading links.
- Inline SVG diagrams generated per topic (page layout diagrams, LSN
  timelines, plan trees, lock tables). Prefer hand-authored SVG over images.
- Small vanilla-JS interactive demos where they materially help
  (B+Tree insertion, clock/LRU-K eviction, MVCC visibility checker,
  two-phase locking schedule checker, ARIES redo/undo stepper).
- Each page ends with: "Primary sources" (numbered citations used on the
  page), "Go deeper" (optional readings), and "Milestone link" (what to
  build now).
- Citation format: superscript numbers linking to a per-page reference list;
  every reference must be a URL from Section 6 (or an obvious sub-page of
  one of those sites, e.g. a specific chapter of interdb.jp or a specific
  section of postgresql.org/docs).

## 4. Concept Curriculum (Modules)

Each module below lists: topics, primary sources (verified URLs), and
suggested visuals/interactives. Claude Code should fetch and read the
sources when drafting each page so the content is precise.

Ordering note: modules are numbered by concept area (mirroring CMU lecture
order), but the project is built in milestone order, so the two sequences
interleave. The recommended path, which index.html must present visually:
M00 -> MS0 -> M01 -> M02 -> MS1 -> M03 -> MS2 -> M05 -> MS3 -> M06 -> MS4
-> M04 -> MS5 -> M07 -> MS6 -> M08 -> MS7 -> M09 -> MS8 -> M10 -> MS9
-> M11 -> MS10 -> M12 -> stretch milestones.

### M00 Orientation: What a DBMS Is and How This Course Works
Topics: course map; anatomy of a relational DBMS (client protocol, parser,
planner/optimizer, executor, access methods, buffer manager, storage, WAL,
transaction manager); disk-oriented vs in-memory architectures; why we model
the project on Postgres and SQLite.
Primary sources:
- Hellerstein, Stonebraker, Hamilton, "Architecture of a Database System"
  (Foundations and Trends in Databases, 2007):
  https://dsf.berkeley.edu/papers/fntdb07-architecture.pdf
- SQLite, "Architecture of SQLite": https://sqlite.org/arch.html
- The Internals of PostgreSQL (Suzuki), home: https://www.interdb.jp/pg/
  (note: the site states educational institutions can use it freely;
  link to figures rather than copying them)
- CMU 15-445 course home (any recent semester):
  https://15445.courses.cs.cmu.edu/
Visuals: full-system architecture SVG with clickable components that link to
the module covering each one.
Milestone link: Milestone 0.

### M01 The Relational Model and SQL Semantics
Topics: relations, keys, integrity constraints; relational algebra
(selection, projection, join, aggregation) as the executor's vocabulary;
SQL to algebra mapping; NULL semantics; why declarative queries enable
optimization.
Primary sources:
- Berkeley CS186 online notes: https://cs186berkeley.net/notes/
- CMU 15-445 lecture notes/schedule (relational model + SQL lectures):
  https://15445.courses.cs.cmu.edu/
- Stonebraker and Pavlo, "What Goes Around Comes Around... And Around..."
  (SIGMOD Record 2024) for historical context:
  https://db.cs.cmu.edu/papers/2024/whatgoesaround-sigmodrec2024.pdf
Visuals: interactive relational-algebra tree builder for a sample query.
Milestone link: none directly; the executor built in Milestone 4 implements
this algebra, so the vocabulary is exercised from there onward.

### M02 Storage: Files, Pages, Tuples
Topics: disk vs memory; database files; fixed-size pages; slotted-page
layout; tuple headers; heap tables; free space management; large values
(overflow/TOAST concept); how Postgres lays out heap pages and tuples.
Primary sources:
- The Internals of PostgreSQL, Ch. 1 (DB cluster, heap table structure,
  writing/reading tuples): https://www.interdb.jp/pg/pgsql01.html
- PostgreSQL official docs, "Database Physical Storage" (page layout,
  TOAST): https://www.postgresql.org/docs/current/storage.html
- SQLite "Database File Format" for a contrasting single-file design:
  https://www.sqlite.org/fileformat.html
- Berkeley CS186 notes (disks/files/records): https://cs186berkeley.net/notes/
Visuals: slotted page SVG with byte offsets; interactive "insert a tuple and
watch the line pointer array grow" widget.
Milestone link: Milestone 1.

### M03 The Buffer Pool
Topics: why a DBMS manages its own cache instead of relying on the OS; frames
vs pages; pin counts; dirty pages; replacement policies (LRU, CLOCK, LRU-K);
prefetching; latch vs lock terminology.
Primary sources:
- CMU 15-445 Project 1 spec (buffer pool manager; describes frames, 8 KB
  pages, replacement, disk scheduling):
  https://15445.courses.cs.cmu.edu/fall2025/project1/
- Architecture of a Database System, storage management chapter:
  https://dsf.berkeley.edu/papers/fntdb07-architecture.pdf
- The Internals of PostgreSQL, buffer manager chapter (Ch. 8):
  https://www.interdb.jp/pg/pgsql08.html
Visuals: buffer pool simulator (user issues page requests, watches
eviction policy decisions and hit/miss counters).
Milestone link: Milestone 2.

### M04 Index Data Structures: B+Trees (and Friends)
Topics: why trees beat hash maps for range scans; B+Tree structure, node
layout on pages, search, insert with splits, delete with merges; latch
crabbing for concurrent access; hash indexes; LSM-trees as the alternative
write-optimized design (survey level).
Primary sources:
- cstack "Let's Build a Simple Database" Part 7 (B-Tree intro, why trees,
  leaf vs internal nodes): https://cstack.github.io/db_tutorial/parts/part7.html
- SQLite file format B-tree pages (cells, interior/leaf structure):
  https://www.sqlite.org/fileformat.html
- CMU 15-445 Project 2 spec (B+Tree index; verified Fall 2025):
  https://15445.courses.cs.cmu.edu/fall2025/project2/
- Mini-LSM book for LSM background: https://skyzh.github.io/mini-lsm/
- Mohan and Levine, "ARIES/IM" (index concurrency and recovery, optional
  deep dive): https://users.cs.utah.edu/~pandey/courses/cs6530/fall22/papers/logrecover/aries.pdf
Visuals: the flagship interactive: animated B+Tree insert/delete with
split/merge steps; page-level byte layout diagram.
Milestone link: Milestone 5 (after Milestones 3-4 — see the recommended
path above; IndexScan plugs into the executor built in Milestone 4).

### M05 SQL Processing Front-End: Tokenizing, Parsing, Binding
Topics: lexer/tokenizer; recursive descent parsing; Pratt (top-down
operator precedence) parsing for expressions; the AST; binding names against
the catalog; semantic analysis; how SQLite compiles SQL to bytecode vs how
Postgres builds plan trees (two front-end philosophies).
Primary sources:
- sqlparser-rs README (states the expression parser uses Pratt/TDOP design
  and the statement parser is hand-written recursive descent):
  https://github.com/apache/datafusion-sqlparser-rs
- SQLite "Architecture of SQLite" (tokenizer -> parser -> code generator ->
  VDBE pipeline): https://sqlite.org/arch.html
- cstack tutorial Parts 1-2 (REPL, simplest front-end/VM split):
  https://cstack.github.io/db_tutorial/parts/part1.html and
  https://cstack.github.io/db_tutorial/parts/part2.html
- The Internals of PostgreSQL, Ch. 3 "Query Processing" overview:
  https://www.interdb.jp/pg/pgsql03.html
Visuals: live tokenizer demo (type SQL, see token stream and AST tree).
Milestone link: Milestone 3.

### M06 Query Execution: Plans and the Iterator Model
Topics: logical vs physical plans; the Volcano/iterator (open/next/close)
model; materialization vs vectorized execution (survey); expression
evaluation; the catalog as the executor's map of the world; sequential scan,
filter, projection, insert/update/delete operators.
Primary sources:
- Architecture of a Database System (query processor sections):
  https://dsf.berkeley.edu/papers/fntdb07-architecture.pdf
- CMU 15-445 Project 3 (query executors) spec, e.g.
  https://15445.courses.cs.cmu.edu/fall2025/project3/
- toyDB architecture guide (iterator-based engine in Rust; excellent code
  walkthrough): https://github.com/erikgrinaker/toydb/blob/main/docs/architecture/index.md
- The Internals of PostgreSQL, Ch. 3: https://www.interdb.jp/pg/pgsql03.html
Visuals: plan-tree stepper showing tuples flowing up through operators.
Milestone link: Milestone 4.

### M07 Joins, Sorting, Aggregation
Topics: nested loop join, block nested loop, merge join, hash join
(build/probe); external merge sort under limited memory; hash and sort based
aggregation; when the planner picks each.
Primary sources:
- The Internals of PostgreSQL, Ch. 3.5 "Join Operations" (nested loop,
  merge, hash join as implemented in Postgres):
  https://www.interdb.jp/pg/pgsql03.html
- Berkeley CS186 notes (sorting/hashing, joins): https://cs186berkeley.net/notes/
- Architecture of a Database System:
  https://dsf.berkeley.edu/papers/fntdb07-architecture.pdf
Visuals: side-by-side animated comparison of the three join algorithms on
the same two tables; external merge sort pass visualizer.
Milestone link: Milestone 6.

### M08 Query Optimization
Topics: rule-based rewrites (predicate pushdown, projection pruning);
cost models and statistics; selectivity estimation; Selinger-style
dynamic-programming join ordering; how Postgres estimates costs and builds
plan trees; EXPLAIN.
Primary sources:
- Selinger et al., "Access Path Selection in a Relational Database
  Management System" (the classic System R optimizer paper):
  https://web.stanford.edu/class/cs345d-01/rl/selinger.pdf
- The Internals of PostgreSQL, Ch. 3.2-3.6 (cost estimation, plan tree
  creation, multi-table planning): https://www.interdb.jp/pg/pgsql03.html
- PostgreSQL docs, "Planner/Optimizer" and "Using EXPLAIN":
  https://www.postgresql.org/docs/current/planner-optimizer.html and
  https://www.postgresql.org/docs/current/using-explain.html
- Readings in Database Systems (5th ed.), Ch. 7 "Query Optimization"
  commentary: http://www.redbook.io/ch7-queryoptimization.html
  (note: the Red Book hosts commentary only, not the papers themselves)
Visuals: interactive cost calculator for seq scan vs index scan given table
stats; join-order search-space explorer for 3-4 tables.
Milestone link: Milestone 7.

### M09 Transactions, Concurrency Control, MVCC
Topics: ACID; schedules and serializability; anomalies (dirty read, lost
update, fuzzy read, phantom, write skew); two-phase locking and deadlocks;
optimistic control; MVCC: tuple versions, xmin/xmax, snapshots, visibility
rules; snapshot isolation vs serializable; Postgres isolation levels and
SSI at survey level; vacuum/garbage collection.
Primary sources:
- The Internals of PostgreSQL, Ch. 5 "Concurrency Control" (MVCC, tuple
  visibility, snapshots): https://www.interdb.jp/pg/pgsql05.html
- PostgreSQL docs, "Transaction Isolation":
  https://www.postgresql.org/docs/current/transaction-iso.html
- toyDB docs (MVCC-based snapshot isolation in Rust; explicitly documents
  which anomalies it prevents and that write skew remains possible without
  SSI): https://github.com/erikgrinaker/toydb/blob/main/docs/sql.md
- CMU 15-445 Project 4 spec (concurrency control; verified Fall 2025):
  https://15445.courses.cs.cmu.edu/fall2025/project4/
- Berkeley CS186 notes (transactions, concurrency):
  https://cs186berkeley.net/notes/
Visuals: MVCC visibility checker (pick a snapshot, see which tuple versions
are visible); 2PL schedule validator; anomaly zoo with replayable timelines.
Milestone link: Milestone 8.

### M10 Logging and Recovery
Topics: failure classification; write-ahead logging rule; force/no-force,
steal/no-steal; LSNs, pageLSN, the dirty page table and transaction table;
ARIES three phases (analysis, redo "repeating history", undo with CLRs);
checkpoints; how Postgres implements WAL, XLOG records, checkpoints, and
point-in-time recovery.
Primary sources:
- Mohan et al., "ARIES: A Transaction Recovery Method Supporting
  Fine-Granularity Locking and Partial Rollbacks Using Write-Ahead Logging"
  (ACM TODS 17(1), 1992): https://web.stanford.edu/class/cs345d-01/rl/aries.pdf
  (canonical record: https://dl.acm.org/doi/10.1145/128765.128770)
- The Internals of PostgreSQL, Ch. 9 "Write Ahead Logging" (XLOG record
  layout, checkpoint processing, database recovery):
  https://www.interdb.jp/pg/pgsql09.html
- PostgreSQL docs, "Reliability and the Write-Ahead Log":
  https://www.postgresql.org/docs/current/wal.html
- Berkeley CS186 notes (recovery/ARIES): https://cs186berkeley.net/notes/
Visuals: the second flagship interactive: an ARIES stepper. Show a log,
crash at a chosen point, then step through analysis/redo/undo watching the
DPT, transaction table, and CLRs update.
Milestone link: Milestone 9.

### M11 Talking to the World: The Postgres Wire Protocol
Topics: client/server protocol design; the PostgreSQL frontend/backend
protocol: startup and authentication, simple query flow, extended query
flow (Parse/Bind/Execute), row description and data rows, error handling;
why speaking the PG protocol gives instant ecosystem compatibility.
Primary sources:
- PostgreSQL docs, "Frontend/Backend Protocol":
  https://www.postgresql.org/docs/current/protocol.html
  (message flow: https://www.postgresql.org/docs/current/protocol-flow.html)
- pgwire Rust crate docs and README (implements startup, simple query, and
  extended query subprotocols; notes the protocol carries no SQL semantics):
  https://github.com/sunng87/pgwire and https://docs.rs/pgwire/latest/pgwire/
Visuals: message sequence diagrams for simple and extended query flows.
Milestone link: Milestone 10.

### M12 Advanced Topics and Where to Go Next (survey, optional)
Topics: LSM storage engines and compaction; column stores and OLAP;
distributed consensus (Raft) and distributed SQL; in-memory systems;
what production Postgres adds that the toy omits (TOAST details, SSI,
parallel query, replication).
Primary sources:
- Mini-LSM course (3-week LSM engine in Rust): https://skyzh.github.io/mini-lsm/
  and https://github.com/skyzh/mini-lsm
- RisingLight educational OLAP DB in Rust:
  https://github.com/risinglightdb/risinglight
- toyDB (Raft + distributed SQL in Rust): https://github.com/erikgrinaker/toydb
- The Internals of PostgreSQL, Ch. 10-11 (PITR, streaming replication):
  https://www.interdb.jp/pg/
- Stonebraker and Pavlo 2024 survey:
  https://db.cs.cmu.edu/papers/2024/whatgoesaround-sigmodrec2024.pdf
- Hellerstein, "Looking Back at Postgres": https://arxiv.org/pdf/1901.01973
Milestone link: stretch milestones S1-S4.

## 5. Project Track: Build FerricDB (Toy Postgres in Rust)

General rules for milestone pages:
- Each milestone page must contain: learning objectives, required reading
  (module link + specific sources), a spec of the public Rust API to
  implement, step-by-step guidance (not full solutions), a test plan with
  provided test skeletons, common pitfalls, and "definition of done".
- Follow the CMU model: projects are cumulative and each one builds on the
  previous. Provide a reference architecture diagram updated per milestone.
- Testing approach: unit tests per crate plus end-to-end sqllogictest files
  once SQL works. Use the sqllogictest-rs crate (used by RisingLight and
  others): https://github.com/risinglightdb/sqllogictest-rs
- Rust workspace layout suggestion:
  crates/storage (pages, heap files, disk manager), crates/buffer,
  crates/index (bptree), crates/sql (lexer, parser, AST), crates/catalog,
  crates/exec (planner + executors), crates/txn (MVCC), crates/wal,
  crates/server (wire protocol), crates/ferricdb (binary + REPL).
- Crate policy: implement core algorithms by hand (pages, buffer pool,
  B+Tree, executor, MVCC, WAL). Allowed helper crates: bytes, thiserror,
  tempfile, criterion (benchmarks), rustyline (REPL), tokio (server
  milestone only). The SQL parser may either be hand-written (recommended,
  Module 5 teaches it) or use sqlparser-rs as an escape hatch; the
  milestone should present both paths.

### Milestone 0: Skeleton, REPL, and Test Harness
Build: cargo workspace, ferricdb binary with a rustyline REPL supporting
meta-commands (.help, .exit, .tables), CI with fmt/clippy/tests.
Guidance sources: cstack Part 1 (REPL first, mirrors how SQLite starts):
https://cstack.github.io/db_tutorial/parts/part1.html
Done when: REPL echoes statements and meta-commands work; CI green.

### Milestone 1: Pages, Tuples, and Heap Storage
Build: 8 KB fixed-size pages; a slotted-page implementation (header, line
pointer array growing forward, tuple data growing backward); tuple
serialization for a small type system (BOOLEAN, INTEGER/i64, FLOAT/f64,
TEXT); a DiskManager (read_page/write_page by page id); a HeapFile with
insert/get/delete by (page_id, slot) tuple id.
Guidance sources: interdb Ch. 1.3 heap table structure
(https://www.interdb.jp/pg/pgsql01.html), PostgreSQL storage docs
(https://www.postgresql.org/docs/current/storage.html), CS186 storage notes.
Pitfalls to document: alignment, slot reuse vs tombstones, page header
checksums (optional), variable-length TEXT encoding (length prefix).
Done when: property tests round-trip random tuples through pages and files.

### Milestone 2: Buffer Pool Manager
Build: fixed number of frames; page table (page_id -> frame); pin/unpin with
pin counts; dirty flags and flush; a replacement policy (start with CLOCK,
stretch goal LRU-K); all storage access from now on goes through the pool.
Guidance sources: CMU Project 1 spec for the component breakdown and
semantics (https://15445.courses.cs.cmu.edu/fall2025/project1/); interdb
Ch. 8 for how Postgres's buffer manager works
(https://www.interdb.jp/pg/pgsql08.html).
Pitfalls: evicting pinned pages, forgetting to flush dirty victims,
deadlocks between the pool latch and per-frame latches.
Done when: multi-threaded stress test passes; hit-rate benchmark reported.

### Milestone 3: SQL Front-End (Lexer, Parser, AST, Catalog)
Build: hand-written lexer; recursive-descent statement parser with a Pratt
expression parser (precedence, unary/binary operators, literals, casts);
AST types; a system catalog stored in the database itself (tables for
tables/columns, bootstrap pages); binder resolving names/types against the
catalog. Grammar scope: CREATE TABLE, DROP TABLE, INSERT, SELECT with
WHERE/ORDER BY/LIMIT, UPDATE, DELETE, BEGIN/COMMIT/ROLLBACK (parsed now,
executed in Milestone 8).
Guidance sources: sqlparser-rs design notes (Pratt/TDOP for expressions,
recursive descent for statements):
https://github.com/apache/datafusion-sqlparser-rs; SQLite front-end
pipeline: https://sqlite.org/arch.html; cstack Part 2 for the front-end/VM
split: https://cstack.github.io/db_tutorial/parts/part2.html
Done when: parser test corpus (valid + invalid SQL) passes; EXPLAIN-style
AST dump command works in the REPL.

### Milestone 4: Planner and Volcano Executor
Build: logical plan from bound AST; physical operators implementing an
iterator (open/next/close) trait: SeqScan, Filter, Projection, Insert,
Update, Delete, Values, Limit, Sort (in-memory for now); expression
evaluator; wire it to the REPL so real queries run end to end.
Guidance sources: toyDB architecture guide for a clean Rust iterator engine
(https://github.com/erikgrinaker/toydb/blob/main/docs/architecture/index.md);
CMU Project 3 spec for operator semantics
(https://15445.courses.cs.cmu.edu/fall2025/project3/); Architecture of a
Database System for the executor chapter.
Done when: a sqllogictest suite of basic CRUD queries passes.

### Milestone 5: B+Tree Index
Build: disk-resident B+Tree keyed on the primary key (then secondary
indexes); node = one page; search, insert with splits, delete (lazy delete
acceptable first, merges as stretch); range iterators; IndexScan operator;
planner picks IndexScan for equality/range predicates on indexed columns;
latch crabbing for concurrent readers/writers (stretch).
Guidance sources: cstack Part 7 for concepts
(https://cstack.github.io/db_tutorial/parts/part7.html); SQLite file format
for concrete page/cell layout inspiration
(https://www.sqlite.org/fileformat.html); CMU Project 2 (B+Tree) spec for
operation semantics and edge cases
(https://15445.courses.cs.cmu.edu/fall2025/project2/).
Done when: randomized insert/delete fuzz tests validate tree invariants
(sorted keys, balanced height, key ranges); index scans return same results
as seq scans on the same predicate.

### Milestone 6: Joins, Aggregation, External Sort
Build: NestedLoopJoin, HashJoin (build/probe), and optionally MergeJoin;
HashAggregate with GROUP BY and aggregate functions (COUNT, SUM, AVG, MIN,
MAX); external merge sort spilling runs to temp files through the buffer
pool; ORDER BY uses it when input exceeds memory budget.
Guidance sources: interdb Ch. 3.5 join operations
(https://www.interdb.jp/pg/pgsql03.html); CS186 notes on sorting/hashing.
Done when: TPC-H-inspired mini queries (provided) produce correct results;
sort verified under an artificially small memory budget.

### Milestone 7: Query Optimization
Build: rewrite rules (predicate pushdown below joins, projection pruning,
constant folding); table statistics (row counts, min/max, distinct count
estimates gathered by an ANALYZE command); simple cost model (page I/O +
cpu-per-tuple, in the spirit of Postgres costs); access path selection
(seq vs index scan); join algorithm choice and greedy or DP join ordering
for up to ~6 tables (Selinger-style); EXPLAIN and EXPLAIN ANALYZE output.
Guidance sources: Selinger et al.
(https://web.stanford.edu/class/cs345d-01/rl/selinger.pdf), with Red Book
Ch. 7 commentary (http://www.redbook.io/ch7-queryoptimization.html);
interdb Ch. 3.2-3.6 cost estimation and plan creation
(https://www.interdb.jp/pg/pgsql03.html); PostgreSQL planner docs
(https://www.postgresql.org/docs/current/planner-optimizer.html).
Done when: EXPLAIN shows plan changes as indexes/statistics change; a test
asserts the optimizer picks the index scan when selectivity is low and the
seq scan when it is high.

### Milestone 8: Transactions and MVCC Snapshot Isolation
Build: transaction ids; tuple version metadata (xmin/xmax analog);
snapshots captured at BEGIN; visibility check function applied by scans;
first-committer-wins write-conflict detection (abort with serialization
error); COMMIT/ROLLBACK; a simple VACUUM command that removes dead versions
older than the oldest active snapshot.
Guidance sources: interdb Ch. 5 for Postgres's MVCC and visibility rules
(https://www.interdb.jp/pg/pgsql05.html); PostgreSQL transaction isolation
docs (https://www.postgresql.org/docs/current/transaction-iso.html); toyDB
docs for a Rust MVCC design and its documented anomaly coverage
(https://github.com/erikgrinaker/toydb/blob/main/docs/sql.md, "Transactions"
section) plus its step-by-step MVCC design walkthrough
(https://github.com/erikgrinaker/toydb/blob/main/docs/architecture/mvcc.md).
Teach honestly: snapshot isolation prevents dirty reads/writes, lost
updates, fuzzy reads, and phantoms, but write skew remains possible without
serializable SI; the page must state this and cite the sources above.
Done when: scripted concurrent-transaction tests demonstrate each anomaly
is prevented (and write skew demonstrably occurs, as documented).

### Milestone 9: Write-Ahead Logging and ARIES-Style Recovery
Build: append-only WAL with LSNs; log records for insert/update/delete,
commit, abort, CLRs, and checkpoints; pageLSN on every page; WAL rule
enforced by the buffer pool (flush log before flushing a dirty page,
no-force at commit beyond flushing the log, steal allowed); recovery in
three phases: analysis (rebuild transaction table and dirty page table from
the last checkpoint), redo (repeat history from the earliest recLSN), undo
(roll back losers writing CLRs); crash-injection test harness (kill the
process at random points, restart, verify invariants).
Guidance sources: the ARIES paper for the algorithm and the repeating
history + CLR paradigm
(https://web.stanford.edu/class/cs345d-01/rl/aries.pdf); interdb Ch. 9 for
Postgres's XLOG format, checkpointing, and recovery
(https://www.interdb.jp/pg/pgsql09.html); PostgreSQL WAL docs
(https://www.postgresql.org/docs/current/wal.html); CS186 recovery notes.
Done when: the crash-injection suite passes hundreds of randomized crash
points with zero committed-data loss and zero resurrected aborted data.

### Milestone 10: PostgreSQL Wire Protocol Server
Build: a TCP server (tokio) speaking the PG frontend/backend protocol:
startup (trust auth first, cleartext password optional), simple query
protocol (Query -> RowDescription -> DataRow* -> CommandComplete ->
ReadyForQuery), error responses, and optionally the extended protocol
(Parse/Bind/Execute) for prepared statements. Two paths: hand-roll the
message framing from the official spec (recommended for learning) or use
the pgwire crate.
Guidance sources: PostgreSQL protocol docs (message formats and flow):
https://www.postgresql.org/docs/current/protocol.html and
https://www.postgresql.org/docs/current/protocol-flow.html; pgwire crate
(https://github.com/sunng87/pgwire) which implements startup, simple, and
extended query handling and ships a SQLite-backed example server.
Done when: `psql -h localhost -p 5433` connects and runs the full
sqllogictest suite through a real psql session.

### Stretch Milestones (optional pages, lower detail)
- S1: Lock manager + row locks for SELECT FOR UPDATE (CMU 15-445 Project 4
  as reference: https://15445.courses.cs.cmu.edu/fall2025/project4/).
- S2: Serializable isolation via SSI survey + simplified implementation.
- S3: Swap the heap for an LSM engine following Mini-LSM
  (https://skyzh.github.io/mini-lsm/).
- S4: Raft replication following toyDB's architecture
  (https://github.com/erikgrinaker/toydb).

## 6. Master Source List (verified)

University courses and notes:
1. CMU 15-445/645 Intro to Database Systems (lectures on YouTube, notes,
   project specs; BusTub project structure: buffer pool, B+Tree, executors,
   concurrency control): https://15445.courses.cs.cmu.edu/
2. Berkeley CS186 online notes: https://cs186berkeley.net/notes/ and course
   site https://cs186berkeley.net/

Books and long-form references (free, credible):
3. Hironobu Suzuki, "The Internals of PostgreSQL" (continuously updated; as
   of mid-2026 the site states it covers PostgreSQL 19-beta and earlier, and
   a new Ch. 12 on logical replication was added May 2026; teachers and
   students at educational organizations may use it freely):
   https://www.interdb.jp/pg/
4. Hellerstein, Stonebraker, Hamilton, "Architecture of a Database System",
   Foundations and Trends in Databases, 2007:
   https://dsf.berkeley.edu/papers/fntdb07-architecture.pdf
5. Bailis, Hellerstein, Stonebraker (eds.), "Readings in Database Systems"
   5th ed. (the Red Book): http://www.redbook.io/
6. PostgreSQL official documentation (storage, WAL, MVCC/transaction
   isolation, planner, protocol): https://www.postgresql.org/docs/current/
7. SQLite official docs: architecture (https://sqlite.org/arch.html) and
   file format (https://www.sqlite.org/fileformat.html)

Foundational papers:
8. Mohan et al., "ARIES", ACM TODS 17(1), 1992:
   https://web.stanford.edu/class/cs345d-01/rl/aries.pdf
   (record: https://dl.acm.org/doi/10.1145/128765.128770)
   Companion: Mohan and Levine, "ARIES/IM" (B+Tree concurrency and
   recovery), SIGMOD 1992:
   https://users.cs.utah.edu/~pandey/courses/cs6530/fall22/papers/logrecover/aries.pdf
   (the file is named aries.pdf but contains ARIES/IM — verified; do not
   "fix" this link)
9. Selinger et al., "Access Path Selection in a Relational DBMS" (System R
   optimizer), SIGMOD 1979:
   https://web.stanford.edu/class/cs345d-01/rl/selinger.pdf
   (Red Book Ch. 7 commentary: http://www.redbook.io/ch7-queryoptimization.html)
10. Stonebraker, Pavlo, "What Goes Around Comes Around... And Around...",
    SIGMOD Record 2024:
    https://db.cs.cmu.edu/papers/2024/whatgoesaround-sigmodrec2024.pdf
11. Hellerstein, "Looking Back at Postgres", 2019:
    https://arxiv.org/pdf/1901.01973

Build-a-database tutorials and reference implementations:
12. cstack, "Let's Build a Simple Database" (SQLite clone in C; no longer
    actively developed but content remains a solid staged walkthrough):
    https://cstack.github.io/db_tutorial/
13. erikgrinaker/toydb (educational distributed SQL DB in Rust: MVCC
    snapshot isolation, iterator query engine, Raft; author later worked on
    CockroachDB and Neon; excellent architecture guide):
    https://github.com/erikgrinaker/toydb
14. skyzh/mini-lsm ("LSM in a Week" Rust course, 3 weeks x 7 chapters, with
    starter code, tests, and solution checkpoints):
    https://skyzh.github.io/mini-lsm/ and https://github.com/skyzh/mini-lsm
15. risinglightdb/risinglight (educational OLAP DB in Rust):
    https://github.com/risinglightdb/risinglight
16. cmu-db/bustub (C++ educational DBMS behind 15-445; useful for API and
    test design inspiration, do not copy code):
    https://github.com/cmu-db/bustub

Rust crates and tooling:
17. apache/datafusion-sqlparser-rs (SQL parser; Pratt expression parsing +
    recursive descent statements): https://github.com/apache/datafusion-sqlparser-rs
18. sunng87/pgwire (PostgreSQL wire protocol library for Rust servers;
    examples/ includes a SQLite-backed server, verified):
    https://github.com/sunng87/pgwire and
    https://docs.rs/pgwire/latest/pgwire/
19. risinglightdb/sqllogictest-rs (sqllogictest parser/runner in Rust):
    https://github.com/risinglightdb/sqllogictest-rs

## 7. Content-Quality Rules for Claude Code

1. Read the primary sources for a module (web fetch) before writing that
   module's page; paraphrase, never copy prose or figures. Redraw all
   diagrams as original SVG.
2. Every algorithm description must match the cited source (e.g. ARIES
   phase order and CLR behavior must match Mohan et al.; MVCC visibility
   must match interdb Ch. 5 / Postgres docs). If sources differ (SQLite vs
   Postgres design), present both and label them.
3. Numbers must be sourced or clearly labeled as this course's choices
   (e.g. "we use 8 KB pages, matching PostgreSQL's default block size and
   BusTub" with citations).
4. Respect licenses: BusTub course policy forbids copying solution code;
   interdb.jp permits free use by teachers/students at educational
   organizations and noncommercial use with attribution, but the author
   retains copyright — so redraw all figures and link rather than copy;
   toyDB and mini-lsm are Apache-2.0 (attribution required if any
   code is adapted); SQLite docs and code are public domain; PostgreSQL
   docs are under the PostgreSQL License.
5. Keep milestone guidance at the "spec + hints + tests" level so learners
   implement the core algorithms themselves; full reference solutions can
   live in a separate solutions branch.
6. Verify all URLs at build time; if one is dead, find the canonical
   replacement rather than dropping the citation. Known checker caveats
   (all URLs in this document were validated 2026-07-05): dl.acm.org
   returns 403 to non-browser clients (bot protection) — verify ACM links
   manually and keep them; https://15445.courses.cs.cmu.edu/ redirects to
   the current semester (spring2026 at validation time), which is expected.