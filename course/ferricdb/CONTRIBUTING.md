# Contributing

FerricDB is a **learning starter**: the API shape is provided, the algorithm
bodies are yours to write. Do not open PRs that fill in `todo!()` stubs against
`main` — that is the coursework itself. Complete solutions live on a separate
`solutions` branch and must never be merged into (or copied onto) `main`.

Fixes to *provided* material are welcome: tests, `crates/common`, docs, CI,
`.slt` files. Keep any public-API change in sync with `docs/api-spec.md`.

Run one milestone's test group with, e.g.:
`cargo test -p ferricdb-storage -- --include-ignored ms1_`
