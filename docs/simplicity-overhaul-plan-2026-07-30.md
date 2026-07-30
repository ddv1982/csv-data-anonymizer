# Simplicity Overhaul — Phased Plan

**Date:** 2026-07-30
**Status:** proposed, no code changes made
**Method:** six parallel audit workers (structure, DRY, comments, dead code, frontend, tests);
every finding below was re-verified against source by the orchestrator before inclusion.

---

## 0. The verdict on the premise

The overhaul was requested on the hypothesis: *"we have a lot of comments, and that is a
sign we have way too much complexity."*

**Measured: the hypothesis is mostly not supported, but it is pointing at something real.**

A hand classification of ~168 comment blocks (~1,290 lines, 35% of the 3,692-line comment
corpus) gives:

| Category | Share | Verdict |
|---|---|---|
| **A** — genuine "why", names the failure prevented | 58% | **keep** |
| **D** — historical narration ("this used to…") | 14% | **delete** — belongs in git |
| **E** — apologia / design-review argument | 13% | **delete** |
| **F** — comment compensating for unclear code | 8% | **fix the code** |
| **C** — restates the code | 3% | delete |
| **B** — required API docs | 4% | *phantom — see below* |

Three things follow.

1. **Only ~8% of comments indicate complexity**, and they are concentrated in
   **four constructs**, not spread through the codebase. That 8% is the part of the
   hypothesis that is true, and it is actionable.
2. **~27% (D+E) is a changelog written into the source.** Several rounds of review left
   their own reasoning behind in the files. This inflates every density metric without
   helping anyone reading the code today. It is the single largest cleanup available and
   it costs no risk at all.
3. **Category B is a phantom.** There is no `missing_docs` lint, no `[lints]` table, and
   no inner attributes in `crates/csv-anonymizer-core/src/lib.rs`. `docs:rustdoc` runs
   `-D warnings`, but those warnings are broken intra-doc links and invalid HTML — not
   missing docs. **Every doc comment in this repo is discretionary.** The only constraint
   on deletion is not orphaning a `` [`link`] `` target.

The density is also a **tail phenomenon**: 108 comment blocks of ≥10 lines hold 55% of all
comment mass; the median block is 4 lines. This is not a pervasive habit. It is about a
hundred hypertrophied blocks.

### What the audit found *against* the "slop" thesis

Worth stating plainly, because it should change what this overhaul spends effort on:

- **One** `#[allow]`/`#[expect]` in the entire Rust codebase — a well-formed `#[expect]`
  with a written reason and a bounds cap on the next line (`service/preflight.rs:478`).
- **Two** traits in the whole workspace, both with ≥2 non-test implementors. No
  single-implementor trait abstraction anywhere.
- All 10 non-`cfg(test)` `#[cfg]` branches are genuine `unix`/`not(unix)` pairs.
- The Rust↔TypeScript type mirror is machine-verified by a 592-line contract checker that
  validates enum variants, struct fields, **nullability direction**, and numeric limits.

On the axes where sloppy codebases usually leak, this one is disciplined. The problems are
concentrated and specific, not diffuse.

---

## 1. Two defects found that are not cleanup

These are behaviour bugs surfaced by the audit. They should be fixed **before** any
refactor, independently of it.

### 1.1 A privacy caveat has been invisible to users since 22 June

`git log -S` proves it: commit `46eeea4` (v1.0.30) rewrote `SectionHelp.tsx` (+144/−20)
and in the same commit deleted four `titleHelp={<SectionHelp …/>}` call sites. The help
*content* stayed; its renderers went.

Three separate "dead code" findings turn out to be one event:

- `sectionHelp.ts` entries `selectFile`, `configuration`, `appSettings`, `preview` are
  unreachable (~88 lines).
- `Card.titleHelp` is declared, typed, and drives a nested ternary — **passed by nobody**
  (`Card.tsx:5,12,20-25`).
- **`sectionHelp.ts:196-204`, the Preview card's *"What it does not prove"* caveat, has
  not been shown to a user in five weeks.**

That caveat is the text explaining that Preview is a sample, not a complete privacy
review. In a tool whose entire contract is never letting a file look safer than it is,
this is a privacy-surface regression, not a cleanup item.

**Action:** restore `<SectionHelp topic="preview" />` to both Preview cards
(`AnonymizerWorkflowView.tsx:250-270`, `PasteDataWorkflowView.tsx:201-212`). Then decide
deliberately on the other three, and delete `Card.titleHelp` once they are resolved.

### 1.2 Renaming an enum variant silently reorders the privacy report

`detection/privacy.rs:469` breaks ties with
`format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind))`.

Report ordering is therefore bound to Rust's derived `Debug` output — i.e. to variant
*names*. A rename that is otherwise behaviour-preserving reorders user-facing privacy
evidence, and nothing would catch it.

**Action:** derive `PartialOrd, Ord` on `PrivacyFindingKind` (`types.rs:322`) and compare
directly, pinning order to declaration order. Note this is **not** behaviour-preserving
for equal-score evidence; it needs a test that pins the intended order.

---

## 2. The central sequencing insight

**Do not hand-enumerate dead code. Change visibility and let the compiler enumerate it.**

`lib.rs` declares seven `pub mod`s. Verified usage from outside the crate:

| module | external module-path uses |
|---|---|
| `direct_input` | 4 (`src-tauri/src/commands/csv.rs`) |
| `detection` | 1 (`benches/detector_matrix.rs`) |
| `csv_io`, `metadata`, `strategies`, `smart`, `service` | **0** |

Because `pub` items in a library crate are never reported as dead, clippy is currently
blind to roughly 2,100 lines. Downgrading those five modules to `pub(crate)` turns rustc's
`dead_code` lint on across them — and CI then produces a deletion list far more reliable
than any grep.

This is why Phase 2 is mechanical and Phase 3 is "read what CI just told us".

---

## 3. Phases

Ordered so that each phase's safety net exists before the phase that needs it.

### Phase 0 — Build the net (prerequisite, no refactor)

The refactor's safety net has two holes, both verified:

- **`src-tauri/src/commands/` has zero tests across 795 lines** (`csv.rs` 337,
  `job_registry.rs` 218, `job_commands.rs` 105, `files.rs` 75, `local_ai_commands.rs` 60,
  `settings_commands.rs` 20). Only `shared.rs` has 2. This is the entire IPC boundary
  between a well-tested core and a well-tested frontend — and the frontend tests *mock* it
  (`vi.mock('./tauri')`). Nothing on either side pins it.
- **`release_report.rs` (1,041 lines) has no `mod tests`.** Its ~700 lines of coverage live
  inside `service/tests/anonymize.rs`.

Tasks:

- **0.1** Add a `mod tests` to `src-tauri/src/commands/csv.rs` covering the Local-AI gating
  predicate and the provider-coercion path at minimum. *(effort M)*
- **0.2** Move the ~700 release-report tests from `service/tests/anonymize.rs:1231-1936`
  into a new `release_report/tests.rs`. **Pure move, no behaviour change.** This must
  precede any `release_report.rs` work. *(effort M, −120 lines)*

**Gate: full 14-check CI green before Phase 1.**

### Phase 1 — Bug fixes (§1.1, §1.2)

Independent of the refactor; do them first so later diffs are pure cleanup.
*(effort S each; 1.2 needs a new ordering test)*

### Phase 2 — Visibility downgrade (mechanical)

- **2.1** `pub mod csv_io|metadata|strategies|smart|service` → `pub(crate) mod`; keep all
  existing root `pub use` re-exports so no external import path changes.
- **2.2** `SmartReplacementMap` (`smart.rs:36`) and `TransformState`
  (`strategies/state.rs:16`) plus their 16 `pub fn` methods → `pub(crate)`; drop
  `SmartReplacementMap` from the `lib.rs:22-24` re-export. Verified: 0 references outside
  the core crate.
- **2.3** Resolve the two overlapping public surfaces: `pub mod types` *and* a hand-curated
  `pub use types::{…}` list that omits 7 items reachable anyway by module path
  (`DetectionCoverageUnit`, `DetectionCoverageSummary`, `RowUniquenessSummary`,
  `DropColumnEffect`, `MatchedColumn`, `MatchedPart`, `TransformContext`). Pick one.
- **2.4** `detection`'s re-export block (`detection.rs:29-32`) → `pub(crate)`; keep only
  `detect_column_type_with_name` public for the bench, or give the bench a crate-internal path.

*(effort S–M, risk low, behaviour-preserving)*

**Gate: `cargo clippy --workspace --all-targets -- -D warnings`. Capture its dead-code
output — that is the Phase 3 worklist.**

### Phase 3 — Delete what the compiler just found

Pre-verified deletions (all confirmed by orchestrator):

- `ProcessResult.success` — write-only. `success: true` at `csv_io.rs:561`, declared at
  `types.rs:721`, read nowhere. *(safe)*
- `AnonymizerError::Privacy` (`error.rs:70`) — **zero constructions**, prod or test.
  *(needs-decision: confirm no planned release-gate path intends to raise it)*
- `csv_io::process_csv_text` (`csv_io.rs:408-414`) — a pure alias of `process_csv_data`,
  same signature, body is one forwarding call. Retarget its single caller
  (`direct_input/csv_text.rs:106`). *(safe)*
- `service::generate_default_output_path` — `pub use`d but has exactly one caller, in the
  same file. *(safe)*
- `direct_input` quick-generate double forwarding layer (`mod.rs:110-119`). *(safe)*
- Frontend: 6 of 40 unreferenced glossary terms (~24 lines); `Card.titleHelp`;
  `SwitchRow.labelHelp`; five dead hook-return members; `usePersistentSettings`'s
  `onAcceptedSettings` (never supplied — removing it collapses the `callbacksRef`
  indirection). *(safe)*
- `csvStrategies` / `directInputStrategies` are byte-identical (`dataOptions.ts:36-42` vs
  `:44-50`); `canPreview` / `canTransform` are character-identical
  (`usePasteDataWorkflow.ts:48,49`). *(safe)*

Needs a human decision (both touch privacy conservatism — flagged, not actioned):

- `WarningSeverity::Info` (`types.rs:1522`) — never constructed; all three sites use
  `Warning`. Removing it makes downgrading a privacy warning *impossible*, which is
  arguably a safety improvement — but it is a frontend-visible contract enum.
- The `Deserialize` derives on 11 output-only DTOs.

### Phase 4 — The comment purge

This is the largest, lowest-risk win, and it is what the original request was reaching for.

- **4.1 Delete category D (historical narration).** 25 verified sites in non-test Rust
  (`grep -nE '//.*(used to|previously|an earlier version|That reasoning was wrong)'`), each
  anchoring a multi-line narration. ~500 lines. Examples: `uniqueness.rs:126-134`,
  `report_notes.rs:109-119`, `release_report.rs:534-541`, `service/controls.rs:92-96`.
- **4.2 Delete category E (apologia).** ~250 lines. Includes meta-commentary about the
  review process itself (`uniqueness.rs:1025-1027` cites a mutation-testing result — that
  belongs in the test) and `uniqueness.rs:82-97`, a design argument for an optimisation
  that was never implemented, including a correction of a previous version of the same comment.
- **4.3 Relocate the calibration tables to `docs/calibration.md`** (does not exist — must
  be created first). `types.rs:807-879` and `:881-947` are ~140 comment lines holding three
  measurement tables for two `f64` constants; `strategies/state.rs:96-155` is the same
  pattern for memory budgets. **This is legitimate category-A evidence** — the data exists
  nowhere else and the constants are unfalsifiable without it. It is a lab notebook in the
  wrong place, not slop. Leave a 6-line summary and a link per constant. ~250-350 lines
  relocated, **not deleted**.
- **4.4 Adopt a house-rule amendment:** *"Present tense only — no comment describes a
  previous version of the code."* This is grep-checkable in CI and prevents regrowth. It is
  the single change that stops this recurring.

**Expected: comment ratio 16% → ~11-12%, with zero information loss.**

### Phase 5 — Category F: fix the code the comments are apologising for

The four constructs where the comment volume genuinely indicates unclear code — the part of
the original hypothesis that is correct.

- **5.1** `release_report.rs:466-485` — a `match` arm whose body duplicates the arm above
  it, kept solely so a 17-line comment has something to attach to. The comment says so
  outright: *"Merged, it would be three lines shorter and nothing would be left to read."*
  Merge into `Review | Blocked =>` and keep one line noting `Blocked` comes only from
  preflight. *(−17 comment, −3 code)*
- **5.2** `uniqueness.rs:535-589` — `CountedColumn`'s four interacting flags (`yielded`,
  `varied`, `rows_yielded`, `first_projection`) need 45 lines of doc to explain which one
  `is_matched` reads. Collapse to a `ProjectionWitness` enum
  (`Silent | Constant(String) | Varied`) + `rows_yielded`; the invariant then holds by
  construction. *(−35 comment via +15 code)*
- **5.3** `uniqueness.rs:340-354` — 15 lines explaining that `Option<Cow>` distinguishes
  `Some("")` from `None`, a distinction the type does not carry and readers demonstrably
  got wrong. Return a named enum (`Projected::NotApplicable | Projected::Key(Cow)`).
  *(−12 comment)*
- **5.4** `uniqueness.rs:1010-1041` — 32 lines on `component_hash`; ~14 are review
  meta-commentary. Move the mutation-testing observation into the test that embodies it.

**Risk note:** 5.2 and 5.3 touch the joint-uniqueness measure. The module rule applies —
over-reporting risk is permitted, under-reporting is not. Neither change may lower
`unique_rows_without`.

### Phase 6 — DRY, Rust

Findings 1, 3, 4, 5 below are **one coherent refactor of the paste/service boundary** and
will conflict if done separately.

- **6.1** The five paste formats hand-assemble the same scaffolding: 4× identical
  `PasteTransformData` tail (`csv_text.rs:234-240`, `text.rs:137-143`,
  `documents.rs:97-103`, `xml.rs:87-93`), 4× identical `PreviewSelection` construction, 3×
  identical smart-replacement prep. **The last four changes to this area each had to be
  made in 4–5 places.** Push into `direct_input/shared.rs`.
- **6.2** "validate indices → apply controls → apply selection" spelled out 4×
  (`service/preview.rs:25-27`, `service.rs:213-215`, `direct_input/shared.rs:129-131`,
  `service/preflight.rs:101-127`).
- **6.3** The "preview already produced usable smart replacements?" gate has 3
  implementations; "build a `TransformState` if it has activity" has 3 more. A change to
  what counts as *activity* currently needs six edits.
- **6.4** `TransformContext` hand-built at ~8 sites. Add
  `TransformContext::for_column(&ColumnMetadata, row_index)`.
- **6.5** `matches!(pii_risk, PiiRisk::High | PiiRisk::Medium)` at 5 sites
  (`metadata.rs:81,85`, `report_notes.rs:61`, `release_report.rs:876,1032` — the last two
  fully qualified, which is itself the argument). Name it: `PiiRisk::is_elevated()`. A
  privacy threshold should not be a literal in five files.
- **6.6** The 100-row detection floor is defined twice under two names
  (`service.rs:30`, `direct_input/shared.rs:29`). Both docs assert the file and paste
  workflows classify on the same basis — currently an unenforced coincidence of two literals.
- **6.7 The coverage disclosure is written twice, in different words**
  (`service/preflight.rs:291-334` vs `report_notes.rs:27-50`) — identical gate, identical
  extraction, differently worded conclusions. `preflight.rs:289-290` even documents that it
  is the "same gate". **Pre-run and post-run screens can drift about the same evidence.**
  Someone must decide which wording is canonical *before* the merge — keep the stricter one.
  *(This also collapses two duplicate tests, ~40 lines.)*

### Phase 7 — Test-suite ergonomics

~2,400-2,700 of 16,335 Rust test lines are fixture boilerplate. Verified churn signal: in
the current working tree, two one-field additions required **34 hand edits across 3 files**.

- **7.1** `RowUniquenessSummary` **already derives `Default`** (`types.rs:1201`) and the
  suite still writes all 11 fields out 15 times. Across all 25 Rust test files there is
  exactly **one** `..Default::default()`. Mechanical rewrite. *(−135 lines, risk low)*
- **7.2** `ColumnMetadata` (15 fields) has **no** `Default` and is hand-written at ~18
  sites, including three near-identical private builders in three files. Add `Default` (its
  serde defaults already cover the churning field) plus one shared `test_support` module.
  *(−230)*
- **7.3** Param structs: use `cfg(test)` builders, **not** `Default`. A `Default` on a
  serde-deserialized privacy param could mask a missing field in real input — deliberately
  the more conservative option. *(−600)*
- **7.4** One `service/tests/mod.rs` harness for the 99-site arrange preamble.
  `label_output.rs` and `cardinality.rs:374-433` already solved this locally. *(−250)*
- **7.5** Frontend: add `rowUniquenessFixture` / `columnReportFixture` to `builders.ts`
  (an 11-field object is hand-written 18× in one file); extract the shared `vi.hoisted`
  tauri mock (19 identical keys across two files). *(−330)*
- **7.6** `csv_io/tests.rs:29-131` duplicates `sampling/tests.rs` line-for-line since
  `csv_io` began delegating to `SpreadSampler`. Keep the `ParsedSample` bookkeeping
  assertions; leave statistical properties to `sampling/tests.rs`. *(−90)*
- **7.7** Two genuinely redundant test pairs, both strict subsets of multi-seed siblings
  (`cardinality.rs:780` ⊂ `:796`; `cardinality.rs:764` ⊂ `:812`) and one duplicate-header
  test (`strategies/tests.rs:1185-1206` ⊂ `label_output.rs:248-288`). **Verify by deletion
  — confirm the survivor still fails when the behaviour is broken** — before committing.
  Move the doc comments (which carry the threshold rationale) onto the survivors. *(−47)*

**Do not table-ify the detection matrices.** `held_out_corpus.rs`, `multilingual_matrix.rs`
and `cardinality.rs` are already exemplary table-driven suites with documented baselines.

### Phase 8 — Frontend structure

- **8.1** Do **not** unify the three workflows. Audit found they are one real workflow
  (CSV, 8 hooks, job polling, preflight), one 70%-overlapping sibling (Paste, single hook,
  no job/output path) and one unrelated generator (Quick, no columns/preview/selection).
  What they genuinely share is already extracted. Extract only the remaining shared shape:
  a "busy + error + preview + result, invalidated on selection change" primitive
  (`useAnonymizerWorkflow.ts:138-151` vs `usePasteDataWorkflow.ts:162-188`).
- **8.2** `ColumnSelectionPanel` re-declares 11 of `ColumnTable`'s props verbatim and
  forwards them, making both call sites drill through two levels. Take
  `{ actions, notice, footer, children }` instead. *(−30)*
- **8.3** Extract `<CopyableOutputCard>` from the 24-line JSX block duplicated between
  `PasteDataWorkflowView.tsx:221-244` and `QuickDataTypeWorkflowView.tsx:156-179`.
- **8.4** ~113 lines of dispatcher plumbing across three CSV sub-hooks; pass one
  `WorkflowShell` object instead of 40 named arguments.
- **8.5** `App.tsx:19-22` mirrors child busy state upward through effects in two views —
  a derivation implemented as an effect. Hoist the hooks. *(risk med: one render-timing
  change to verify against the tab-disabled assertions.)*
- **8.6 Keep the hand-written Rust↔TS mirror.** Codegen would delete ~120 lines of
  `types.ts` doc comments carrying privacy semantics (what a `null` means, why a figure is
  a lower bound) that `PrivacyReportSummary.tsx` renders decisions from — and would not
  reproduce the nullability-direction or numeric-limit checks. The available win is
  narrowing what crosses the boundary: `ColumnMetadata.sourcePath` and `.privacyFindings`
  are declared and never read on the frontend.

---

## 4. What must be left alone

Explicitly out of scope. Simplifying any of these would trade a compiler-checked guarantee
for a silent default, or weaken a privacy claim.

- **The eight exhaustive `match column.strategy` arms** (`strategies/mod.rs:53`,
  `strategies/state.rs:182`, `uniqueness.rs:247`, `service/privacy_report.rs:60,146`,
  `service/controls.rs:76,181`, `release_report.rs:888`). They look like one table copied
  eight times; each answers a different question. Collapsing them trades **eight compile
  errors for one silent default** when a new strategy is added.
- **The seven per-`DataType` tables in `types.rs`** — seven independent properties, not one
  duplicated list. They already delegate where they genuinely share a rule.
- **`RedactionPlaceholder`** (`types.rs:278-289`) — looks like a pass-through to string
  constants; it is a *restricted set*, so type evidence alone cannot claim `[ACCOUNT_ID]`.
  Deleting it removes a compiler-checked constraint on a privacy claim.
- **`path_access.rs` input/output pairs** — parallel-looking, but two different threat
  models; the output path canonicalizes the parent, not the leaf, and validates symlinks.
- **`utils/errors.ts:53-63`** — the four path regexes cover POSIX, Windows drive and UNC
  separately. Collapsing risks leaking a path into a user-visible error.
- **`privacy-report/helpers.ts` no-`toLocaleString` rule** — prevents "4,123" and "4123"
  appearing in one panel.
- **`PrivacyReportSummary.tsx`** — long by design; its five near-identical
  `MatchedColumnLine` calls are deliberate, and the comment records that folding them
  produced a false claim about which cells were published.
- **`useAnonymizeJob.ts:22-105,169-245`** — poll/backoff guarding a documented app-freeze bug.
- **`tauri.ts:281-324` `browserPreviewFallback`** — dead-looking; it is what lets
  `vite dev` render outside Tauri.
- **The validator crates** (`ein`, `card_validate`, `iban`, `ssn`, `email_address`,
  `phonenumber`, `vat_id_validator`) — each used once, each encoding a real checksum rule.
  Hand-rolling checksums in privacy software is a regression.
- **`service/controls.rs:217-219`** — do not merge its "distinct of total" wording with the
  two exact-figure sites. Its figures are *sampled* and its wording says so. Merging would
  print an estimate in exact-figure wording — the one direction this codebase must not move.

---

## 5. Expected outcome

| Phase | Δ lines | Risk |
|---|---|---|
| 0 — build the net | **+** (adds tests) | low |
| 1 — two bug fixes | ~0 | low |
| 2 — visibility downgrade | ~0 | low |
| 3 — delete dead code | −200 to −400, **plus whatever CI finds** | low |
| 4 — comment purge | −750 delete, −300 relocated | none |
| 5 — category-F code fixes | −60 comment, +30 code | **med** |
| 6 — Rust DRY | −300 to −500 | low–med |
| 7 — test ergonomics | −1,600 to −2,000 | low |
| 8 — frontend | −300 to −400 | low–med |

**Total ≈ −3,500 to −4,500 lines of ~48,000**, plus an unknown quantity that only appears
once Phase 2 lets the compiler speak.

The comment ratio falls from 16% to ~11-12%. But the honest headline is that **most of the
reduction is in tests and comments, not in production logic** — because the production
logic was not, on the evidence, the problem. The four constructs in Phase 5 are.

## 6. Ordering constraints (hard)

- Phase 0.2 (move release-report tests) **before** any `release_report.rs` change.
- Phase 2 **before** Phase 3 — the compiler produces the list.
- Phase 6.7 needs a wording decision **before** the merge, not during.
- Phase 8.4 before 8.5 — they touch the same call sites.
- Phase 5 changes must not lower `unique_rows_without`.
- 187 `contains("…")` assertions pin exact report wording. For a privacy report the wording
  *is* the product, so these are load-bearing: any Phase 6.7 wording change must be
  re-verified by hand, not mechanically. Copy the good pattern at `cardinality.rs:21`,
  which imports the constant from production instead of retyping the substring.
