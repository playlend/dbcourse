# FerricDB — Database Systems, Built From Scratch

A hands-on database internals course: 13 concept modules paired with an
11-milestone project in which you build **FerricDB, a toy PostgreSQL clone in
Rust** — from an empty REPL to a multi-version, crash-recoverable SQL database
that real Postgres clients (`psql`) can talk to over the wire.

**Take the course → <https://playlend.github.io/dbcourse/>**

The site is pure static HTML with no build step and no external dependencies:
every page also renders fully offline via `file://`, in light or dark mode,
with all diagrams as inline SVG and every factual claim cited to a primary
source.

## What you build

Each concept module (storage formats, buffer pools, B+Trees, SQL parsing,
Volcano execution, join algorithms, cost-based optimization, MVCC, ARIES
recovery, the Postgres wire protocol…) feeds a project milestone where you
implement that layer by hand. The model is the same as CMU's BusTub: the
public API of every crate is fully declared and documented, the test suites
encode the expected behavior, and the method bodies are `todo!("Milestone N")`
stubs — you supply the algorithms.

## Repository layout

| Path | What it is |
|---|---|
| `course/` | The course site (published to GitHub Pages) |
| `course/modules/m00…m12/` | 13 concept modules |
| `course/project/` | Project overview, milestones 00–10, stretch goals S1–S4 |
| `course/ferricdb/` | The Rust starter workspace you implement — see [its README](course/ferricdb/README.md) |
| `authoring/` | Page contract ([CONVENTIONS.md](authoring/CONVENTIONS.md)), API spec, page templates, QA checker |
| `course_implementation_plan.md` | The build specification the course was produced from |

## Getting started as a learner

Read the course online, or clone it and open `course/index.html` in a
browser. The project track needs Rust 1.80+:

```console
$ git clone https://github.com/playlend/dbcourse.git
$ cd dbcourse/course/ferricdb
$ cargo test --workspace     # a fresh clone is green
$ cargo run -p ferricdb      # Milestone 0: bring the REPL up
```

Milestone tests ship `#[ignore = "milestone N"]` and are opted into as you
reach them (`cargo test -- --include-ignored ms1_`), so the workspace always
compiles and CI stays green at every stage. The
[starter README](course/ferricdb/README.md) explains the workflow, the crate
policy, and the workspace tour; a standalone starter repository is produced
from this one with `git subtree split --prefix=course/ferricdb`.

## Working on the course itself

There is no build step — edit the HTML, refresh the browser. To serve it
locally instead of using `file://`:

```console
$ python3 -m http.server -d course
```

Every page must satisfy the authoring contract in
[`authoring/CONVENTIONS.md`](authoring/CONVENTIONS.md) (offline-only assets,
cited claims, semantic structure, dark-mode-safe SVG). The QA checker
enforces it:

```console
$ python3 authoring/qa_check.py          # structural + citation checks
$ python3 authoring/qa_check.py --net    # also verify reference URLs are live
```

Pushes to `main` deploy the site automatically via
[`.github/workflows/pages.yml`](.github/workflows/pages.yml) (the Rust
workspace is excluded from the published site; it carries its own CI for the
standalone starter repo).

## Acknowledgements

The course design draws on [CMU 15-445/BusTub](https://github.com/cmu-db/bustub)
(starter-with-stubs model), [toyDB](https://github.com/erikgrinaker/toydb),
and [mini-lsm](https://github.com/skyzh/mini-lsm). Attribution and
academic-integrity rules for adapted code are spelled out in the
[starter README](course/ferricdb/README.md).

## License

Everything in this repository — course text, diagrams, and code — is released
under the [MIT License](LICENSE). The starter workspace carries
[its own copy](course/ferricdb/LICENSE) so the split-out starter repository
stays licensed.

FerricDB is an independent educational project, not affiliated with or
endorsed by the PostgreSQL Global Development Group.
