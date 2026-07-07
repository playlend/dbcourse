# FerricDB Course — Page Authoring Conventions

Contract for every page in `course/`. Follow it exactly; the QA pass checks
against it. Read this whole file before writing a page.

## 1. Files and paths

- Course root: `/usr2/dbcourse/course/`
- Module pages: `course/modules/<mNN-slug>/index.html`
- Project pages: `course/project/<milestone-NN|stretch-sN>/index.html`
- All content pages sit exactly two levels below the course root, so shared
  assets are always `../../assets/style.css`, `../../assets/site.js`,
  `../../assets/arch.js`.
- Templates to start from: `authoring/templates/module-template.html` and
  `authoring/templates/milestone-template.html`. Keep the head block (theme
  pre-paint script, stylesheet link, deferred site.js) byte-for-byte except
  the `<title>`.

## 2. Hard requirements (QA rejects violations)

1. **Offline**: the page must render fully via `file://`. No CDNs, no web
   fonts, no external `<script>/<link>/<img>` sources. External URLs may
   appear only inside `href` of citation/reading links.
2. **Citations**: every factual claim (numbers, algorithm behavior, design
   facts about Postgres/SQLite/etc.) carries a superscript citation
   `<sup class="cite"><a href="#ref-N">N</a></sup>` resolving to the
   page-footer "Primary sources" list. Every reference URL must be from the
   master source list (section 8 below) or an obvious sub-page of one of
   those sites (e.g. a specific interdb.jp chapter, a specific
   postgresql.org/docs page, a specific GitHub file in a listed repo).
3. **Grounding**: fetch and read the module's primary sources before
   writing. Paraphrase — never copy prose; redraw all diagrams as original
   inline SVG. Algorithm descriptions must match the cited source. Where
   sources describe different designs (SQLite vs Postgres), present both,
   labeled, ideally with the `.tabs` component.
4. **Course choices are labeled**: any number we chose (8 KB pages, 5433
   port, memory budgets) is stated as a course choice with the source that
   inspired it, e.g. "we use 8 KB pages, matching PostgreSQL's default
   block size¹ and BusTub²".
5. **Semantic HTML**: one `<h1>`, hierarchical `<h2>/<h3>` with `id`s, the
   `.toc` box linking to all `<h2>`s, `lang="en"`, `<title>` matching the
   pattern in the templates.
6. **Ends with** (in this order, inside `<footer class="page-footer">`):
   "Primary sources" (numbered, ids `ref-1…ref-N`, live URLs),
   "Go deeper" (optional readings, 2–5 items), "Milestone link" (modules
   only — which milestone to build now, as a relative link), prev/next nav
   (section 7).
7. **JS discipline**: page-specific script goes in ONE `<script>` block at
   the end of `<body>`, wrapped in
   `document.addEventListener("DOMContentLoaded", function () { ... })` and
   `"use strict"`. Use `Course.stepper`, `Course.svgEl`, `Course.el`
   helpers. No global leaks. The page must remain fully readable with JS
   disabled (include the `noscript-warn` paragraph).
8. **Diagrams**: inline SVG with `role="img"` and a `<title>` child; use
   ONLY the shared classes (`d-box`, `d-box-hl`, `d-box-good`, `d-box-warn`,
   `d-box-bad`, `d-line`, `d-line-hl`, `d-text`, `d-text-sm`, `d-mono`,
   `d-mono-sm`) or `currentColor`/CSS variables so dark mode works. Never
   hard-code colors. Wrap in `<figure>` with a numbered `<figcaption>`.
   Wide diagrams: keep `viewBox` width ≤ 960.
9. **Tables** wrapped in `<div class="table-wrap">`.
10. **Quizzes**: 3–6 self-check questions per module page (1–3 per
    milestone page) using the `.quiz` markup from the template, each with a
    `data-explain` that teaches, not just confirms. Mark correct options
    with `data-correct`. Use unique `name` per question.

## 3. Voice and depth

- Audience: strong programmers, Rust-literate, no DB internals background.
- Tone: direct, technical, concrete; second person for actions ("you will
  implement…"). No marketing fluff, no "simply".
- Module pages: aim for 3,000–5,500 words plus figures/widgets — comparable
  to a good lecture-note chapter. Include at least one worked example with
  real numbers, at least 2 original SVG figures, and the module's
  interactive(s) listed in the plan.
- Milestone pages: spec + hints + tests, never solution code. Rust
  snippets show *signatures and doc comments* of what to implement (from
  `course/ferricdb/docs/api-spec.md`), plus small usage examples. Solution-level
  algorithm bodies are forbidden; pseudocode of well-known published
  algorithms (from cited sources) is fine.

## 4. Interactive widgets

- Shell markup: `.widget` with `.widget-title`, `.widget-sub`, `.controls`,
  and a canvas `div`. Buttons use `.btn` / `.btn primary`.
- Steppers use `Course.stepper(container, {total, captions, render})`.
- Keep state in the IIFE; rebuild SVG/DOM via `Course.svgEl`/`Course.el`.
- Widgets must be deterministic or seeded-random with a visible "Reset".
- Degrade gracefully: the figure caption or surrounding prose must convey
  the idea even if JS is off.

## 5. Architecture diagram (milestone pages + M00)

Call `Course.renderArchitecture(container, {done, current, root: "../.."})`.
Component ids: `client, protocol, frontend, catalog, planner, executor,
txn, heap, index, buffer, wal, disk`. Per-milestone assignment:

| Page | done | current |
|------|------|---------|
| MS0  | —    | — (REPL wraps the whole system; render with no highlights) |
| MS1  | —    | heap, disk |
| MS2  | heap, disk | buffer |
| MS3  | heap, disk, buffer | frontend, catalog |
| MS4  | heap, disk, buffer, frontend, catalog | planner, executor |
| MS5  | heap, disk, buffer, frontend, catalog, planner, executor | index |
| MS6  | heap, disk, buffer, frontend, catalog, planner, index | executor |
| MS7  | heap, disk, buffer, frontend, catalog, executor, index | planner |
| MS8  | heap, disk, buffer, frontend, catalog, planner, executor, index | txn |
| MS9  | heap, disk, buffer, frontend, catalog, planner, executor, index, txn | wal |
| MS10 | heap, disk, buffer, frontend, catalog, planner, executor, index, txn, wal | protocol, client |

M00 renders it with `{links: true}` and no done/current as the clickable
course map. Stretch pages S1–S4: all ids in `done`, none `current`.

## 6. Cross-linking

- Module → its milestone(s) in "Milestone link"; milestone → its module(s)
  in "Required reading".
- When another module explains a concept in depth, link inline to it
  (`../mNN-slug/index.html`) instead of re-teaching.
- FerricDB workspace paths are referenced as
  `course/ferricdb/crates/<crate>` in prose (plain code text, not links).

## 7. Recommended-path order (drives prev/next)

index → M00 → MS0 → M01 → M02 → MS1 → M03 → MS2 → M05 → MS3 → M06 → MS4 →
M04 → MS5 → M07 → MS6 → M08 → MS7 → M09 → MS8 → M10 → MS9 → M11 → MS10 →
M12 → S1 → S2 → S3 → S4

Each page's prev/next nav follows this sequence exactly (M00's prev is the
course home; S4 has no next — use a link back to the course home labeled
"Course home"). Labels: "M04 · B+Trees & Friends", "Milestone 5 · B+Tree
Index", etc.

## 8. Master source allowlist (URLs must come from these sites)

- 15445.courses.cs.cmu.edu (CMU 15-445; project specs under /fall2025/)
- cs186berkeley.net (Berkeley CS186 notes)
- www.interdb.jp/pg (The Internals of PostgreSQL)
- dsf.berkeley.edu/papers/fntdb07-architecture.pdf (Architecture of a DBMS)
- redbook.io (Readings in Database Systems 5th ed.)
- www.postgresql.org/docs/current/ (PostgreSQL docs)
- sqlite.org (architecture, file format docs)
- web.stanford.edu/class/cs345d-01/rl/aries.pdf (ARIES) and
  dl.acm.org/doi/10.1145/128765.128770 (canonical record)
- users.cs.utah.edu/~pandey/courses/cs6530/fall22/papers/logrecover/aries.pdf
  (this file is ARIES/IM despite the name — cite as ARIES/IM, do not "fix")
- web.stanford.edu/class/cs345d-01/rl/selinger.pdf (Selinger et al.)
- db.cs.cmu.edu/papers/2024/whatgoesaround-sigmodrec2024.pdf
- arxiv.org/pdf/1901.01973 (Looking Back at Postgres)
- cstack.github.io/db_tutorial
- github.com/erikgrinaker/toydb (+ raw.githubusercontent.com equivalents)
- skyzh.github.io/mini-lsm and github.com/skyzh/mini-lsm
- github.com/risinglightdb/risinglight, github.com/risinglightdb/sqllogictest-rs
- github.com/cmu-db/bustub
- github.com/apache/datafusion-sqlparser-rs
- github.com/sunng87/pgwire and docs.rs/pgwire
- doc.rust-lang.org (Rust refresher readings — M00 "Go deeper" and milestone
  pages only)
- docs.rs (API docs of the allowed helper crates: rustyline, tokio, bytes,
  thiserror, sqllogictest, criterion, pgwire)

License notes to respect: never copy BusTub solution code; interdb.jp —
paraphrase and redraw, link to figures, attribute; toyDB/mini-lsm are
Apache-2.0 (attribute if adapting code); SQLite docs public domain;
PostgreSQL docs under the PostgreSQL License.

## 9. Fetching sources

- HTML sources: fetch directly (WebFetch).
- PDFs (ARIES, Selinger, fntdb07): download with
  `curl -sL -o <scratchpad>/name.pdf <url>` and use the Read tool with page
  ranges. Read the sections you cite; do not cite unread material.
- GitHub docs: fetch the raw.githubusercontent.com version, cite the
  github.com URL.
- If a URL is dead, find the canonical replacement (do not drop the
  citation); note the substitution in your final report.

## 10. Self-check before finishing a page

- [ ] Renders standalone: correct relative asset paths, no external loads
- [ ] Theme toggle works (head pre-paint script present)
- [ ] Every `<sup class="cite">` target exists; every ref is allowlisted
- [ ] All internal links point at files that exist (or are named in the
      plan's deliverable tree)
- [ ] Quizzes: exactly one `data-correct` per radio group (or checkboxes
      for multi-answer), unique names, feedback div present
- [ ] Widgets run without console errors; Reset restores initial state
- [ ] `<h2>` ids match the `.toc`; prev/next hrefs match section 7
- [ ] No lorem ipsum, no TODO markers, no placeholder text left
