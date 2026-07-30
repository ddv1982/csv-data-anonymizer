# Comment Cleanup — Phased Plan

**Date:** 2026-07-30
**Status:** proposed, no code changes made
**Companion to:** `docs/simplicity-overhaul-plan-2026-07-30.md` (Phase 4 and Phase 5 there,
expanded here into executable batches)

---

## 0. What the measurements say

Whole non-test Rust crate, measured mechanically:

| metric | value |
|---|---|
| non-test Rust lines | 21,845 |
| comment lines | 3,672 |
| ratio | **16%** |
| comment blocks | 565 |
| median block | 4 lines |
| blocks ≥10 lines | **113, holding 1,990 lines (54%)** |
| blocks ≥20 lines | **31, holding 925 lines (25%)** |

**The problem is 31 blocks, not 565.** A quarter of all comment mass sits in thirty-one
places. This is the single most important fact for planning: this is not a habit to be
retrained across the codebase, it is a short list to be worked through.

Classification of a hand-sampled ~168 blocks (~1,290 lines, 35% of corpus):

| Cat | What it is | Share | Action |
|---|---|---|---|
| **A** | genuine "why", names the failure prevented | 58% | **keep** |
| **D** | historical narration — "this used to…" | 14% | **delete** |
| **E** | apologia, design-review argument, meta-commentary | 13% | **delete** |
| **F** | compensating for unclear code | 8% | **change the code** |
| **C** | restates the code | 3% | delete |
| **B** | required API docs | 4% | *phantom* |

**Category B does not exist.** There is no `missing_docs` lint, no `[lints]` table, no
inner attributes in `lib.rs`. `docs:rustdoc` runs `-D warnings`, but rustdoc's warnings are
broken intra-doc links and invalid HTML. Every doc comment here is discretionary; the only
deletion constraint is not orphaning a `` [`link`] `` target.

---

## 1. The triage rule

Apply to every block. This is the whole method.

> **Delete** if it describes a *previous version of the code*.
> **Delete** if it argues with a reviewer rather than informing a maintainer.
> **Relocate** if it is evidence (measurements, tables) rather than explanation.
> **Keep** if a maintainer editing the next line would make a mistake without it.
> **Fix the code** if the comment exists to explain what a name or a type should have said.

The house rule ("name the failure prevented") is being followed — that is why category A is
58%. The defect is that **the rule has no stopping condition**, so a comment that names a
failure also narrates its discovery, defends the alternative not taken, and pre-empts the
next reviewer. The amendment in Phase 4 is what fixes that permanently.

---

## 2. Phases

Sequenced cheapest-and-safest first, so momentum is bought before risk is spent.

---

### Phase C1 — Delete historical narration *(no risk, ~2 hours)*

Comments describing what the code **used to** do. Git already holds this, better.

**19 strict-marker sites**, found by:

```
grep -rnE '(///|//).*(used to|previously (did|was|were|fell)|an earlier version|That reasoning was wrong|It did not always|used to be)' \
  --include=*.rs crates src-tauri | grep -v target | grep -v '/tests' | grep -v tests.rs
```

Each marker anchors a multi-line narration, so the deletable mass is ~500 lines, not 19.
Confirmed sites, by file:

| file | line | what it narrates |
|---|---|---|
| `uniqueness.rs` | 82, 103, 129, 217, 229-230, 261 | six separate narrations, incl. a doc correcting a previous version of itself |
| `release_report.rs` | 50, 373, 536 | incl. "used to be written as `if review_items.is_empty()`" |
| `detection.rs` | 120, 199, 493 | |
| `header_rules.rs` | 40, 291 | |
| `report_notes.rs` | 109 | "It did not always…" (11 lines) |
| `service.rs` | 47 | |
| `service/controls.rs` | 92 | "Masking used to return `None` here" |
| `direct_input/shared.rs` | 225 | |
| `detection/spans.rs` | 532 | |
| `detection/patterns.rs` | 32 | |
| `types.rs` | 955 | |
| `jobs.rs` | 115 | |

**⚠ Do not use `no longer` as a marker.** All four of its hits describe *current* behaviour,
not history — `validators.rs:138` ("`1800FLOWERS` is no longer recognized"), `jobs.rs:22`,
`jobs.rs:409`, `uniqueness.rs:730`. A naive grep rule deletes real information here. This is
exactly why Phase C1 is a reviewed list and not a `sed` script.

**Method:** delete the narration; keep the sentence stating the *current* rule. If the
history explains a non-obvious constraint that still binds, restate it in the present tense
("X must not Y, because Z") rather than as a story.

**Verify:** `cargo test --workspace && npm run docs:rustdoc`.

---

### Phase C2 — Delete apologia and review meta-commentary *(no risk, ~2 hours)*

Comments defending a choice at length, or discussing the review process itself. ~250 lines.

Confirmed targets:

- **`uniqueness.rs:58-97`** (40 lines on one `usize` constant). Lines 82-97 are
  *"**The fix is known and deliberately not taken**"* — a design argument for an
  optimisation that was never implemented, including a correction of the comment's own
  earlier version. **Relocate to an issue or a design doc**; keep the ~10 lines stating the
  actual bound and why it is shared.
- **`uniqueness.rs:1010-1041`** (32 lines on `component_hash`). Contains
  *"Deleting the line is an equivalent mutant, verified as one — the boundary assertion in
  `position_is_part_of_each_component` still passes without it."* That belongs **in that
  test**, not in the source. Also holds a stray line-wrap artefact at 1029-1030.
  Delete ~14 lines; keep the additive-composition rationale and the collision note.
- **"Not tested:" / "The honest summary:" hedging paragraphs** — a house tic that mostly
  restates that a measurement is a measurement. `types.rs:875-878`, `:933-946`;
  `strategies/state.rs:118-123`, `:141-142`. Keep one sentence per calibrated constant;
  delete the meta-hedging. ~60 lines.
- **`release_report.rs:104-115`** — 12 lines explaining why an exhaustive `match` replaced
  an array literal ("an array literal is a wildcard arm wearing a disguise"). The argument
  is right and **the compiler already enforces it**. Cut to 3 lines saying so.
- **`detection/header_rules.rs:33-48`** — 16 lines on why `Confidence::Low` is
  unrepresentable. Same treatment: the type system is the enforcement. Cut to 3-4 lines.

---

### Phase C3 — Relocate the evidence *(no risk, but needs a destination first)*

This is **legitimate category-A material in the wrong place** — a lab notebook, not slop.
It must be relocated, not deleted; deleting it destroys measurements that exist nowhere
else and makes the constants unfalsifiable.

**Blocker: `docs/calibration.md` does not exist. Create it first.**

| source | lines | content |
|---|---|---|
| `types.rs:807-879` | 72 | `MIN_SAMPLE_COVERAGE` — the largest comment block in the crate |
| `types.rs:881-947` | 66 | `MIN_INVERTIBLE_DOMINANT_SHARE` |
| `types.rs:1034` | 38 | frequency-inversion risk |
| `strategies/state.rs:96-124` | 28 | memory budget table |
| `strategies/state.rs:126-155` | 29 | memory budget table |

~230 lines relocated. Leave a 6-line summary plus a link per constant.

**⚠ Verify the tables are still accurate before moving them.** They cite harnesses
(`service::tests::cardinality`, `strategies::tests::mapping_budget`). If those have drifted,
some of this is category-D in disguise and should be re-measured, not copied.

---

### Phase C4 — Stop the regrowth *(the permanent fix)*

Without this, C1–C3 will need repeating in six months. Everything above is a one-off; this
is what changes the trend.

**Amend the house rule to:**

> Doc comments explain **why** and name the failure prevented — **in the present tense, in
> one paragraph.** No comment describes a previous version of the code. No comment argues
> with a reviewer. Measurements and tables go in `docs/`, with a link.

**Add a CI check** (`scripts/check-comments.mjs`, joining the existing gate):

1. **Fail on past-tense-about-code markers** — the strict list from C1, *not* `no longer`.
2. **Warn on any comment block > 20 lines** in non-test Rust. Not an error: `file_ops.rs:96`
   (32 lines) and `prompt.rs:1` (30) are plausibly legitimate. A warning with a required
   one-line justification is the right pressure. Threshold chosen from data: 31 blocks
   currently exceed it, holding 25% of all comment mass.

This is cheap to write and it is the single change that prevents the next review round from
re-inflating the source.

---

### Phase C5 — Fix the four constructs the comments are apologising for *(real risk)*

**This is the 8% — the part of the "too many comments means too much complexity"
hypothesis that is correct.** Everything above deletes prose. This changes code, and it is
the only phase here that can break behaviour.

Do it **after** C1–C3, so the diffs are readable, and **after** the overhaul plan's Phase 0
(the release-report tests must be moved first).

1. **`release_report.rs:466-485` — a match arm that exists to host its own comment.**
   Its body duplicates the `Review` arm above it. The comment says so:
   *"Merged, it would be three lines shorter and nothing would be left to read."*
   → Merge into `Review | Blocked =>`; keep one line noting `Blocked` reaches this only
   from preflight. *(−17 comment, −3 code)*

2. **`uniqueness.rs:535-589` — four interacting flags needing 45 lines to disambiguate.**
   `CountedColumn` carries `yielded`, `varied`, `rows_yielded`, `first_projection`, and the
   doc's whole job is explaining which one `is_matched` reads.
   → Collapse to `ProjectionWitness { Silent | Constant(String) | Varied }` + `rows_yielded`.
   The invariant then holds **by construction** and the doc is unnecessary.
   *(−35 comment, +15 code)*

3. **`uniqueness.rs:340-354` — a distinction the type does not carry.**
   15 lines explain that `Option<Cow>` means `Some("")` = "projection succeeded, cell not
   blank" vs `None` = "did not apply" — a distinction readers demonstrably got wrong.
   → `enum Projected { NotApplicable, Key(Cow<'_, str>) }`. *(−12 comment)*

4. **`uniqueness.rs:610-652` — the flag/ceiling interplay**, downstream of (2); reassess
   once `ProjectionWitness` lands, as much of it may evaporate.

**Hard constraint:** items 2–4 touch the joint-uniqueness measure. The module rule binds —
over-reporting risk is permitted, under-reporting is not. **No change here may lower
`unique_rows_without`.** Verify against `uniqueness/tests.rs` and the 187 exact-wording
assertions before and after.

---

## 3. Expected outcome

| phase | Δ comment lines | risk | order |
|---|---|---|---|
| C1 narration | −500 | none | first |
| C2 apologia | −250 | none | any |
| C3 relocate | −230 (moved, not lost) | none | needs `docs/calibration.md` |
| C4 CI rule | 0 | none | **before the next review round** |
| C5 code fixes | −64 comment, +15 code | **med** | last |

**Comment ratio: 16% → ~11-12%.** Roughly 1,000 lines leave the source, of which ~230 move
to `docs/` rather than disappearing.

**`uniqueness.rs` specifically** goes from 54% comments (582/1070) to roughly 25-30% — it is
the file that dominates every one of these phases, and it is worth noting *why*: its code is
not especially complex. It holds one enum, one five-method struct and four free functions.
The 54% is prose volume, not code density. That file is the clearest single refutation of
the original hypothesis — and, via C5, also its clearest supporting case.

---

## 4. What not to touch

- **`service/controls.rs`** (25% ratio) is the healthiest file sampled — 10 blocks, all but
  one squarely category A, each naming a concrete failure. Only `:92-96` is narration (in
  C1). **Use this file as the reference for what right-sized looks like here:** ~7.8 comment
  lines per block, ~30 lines of code between blocks.
- **`useAnonymizeJob.ts:22-105`** (frontend) — 84 lines of constants and comments guarding a
  documented app-freeze bug. High ratio is the point.
- **`privacy-report/PrivacyReportSummary.tsx`** — ~120 of its 430 lines are comments
  recording why each sentence is worded as it is and which Rust function it must stay
  consistent with. The comment at `:266-271` records that folding five near-identical calls
  into one list previously produced **a false claim about which cells were published**.
- **`privacy-report/helpers.ts:42-55`** — explains the no-`toLocaleString` rule that stops
  "4,123" and "4123" appearing in one panel.
- **Any comment naming a cross-file consistency obligation** — e.g. `detection.rs:478-488`,
  which documents that `classify_pii_risk` must agree with the `DataType` tables in
  `types.rs`. These are load-bearing precisely because the compiler cannot check them.

---

## 5. The honest summary

Of ~3,672 comment lines, about **1,000 should leave the source** — and roughly a quarter of
those should land in `docs/` rather than be destroyed.

But the reason for the density is **not** that the code is too complex. It is that the
codebase has been through several review rounds, and each one wrote its reasoning into the
source instead of into git. 27% of the comment mass is a changelog and a design-review
transcript. Only 8% marks code a maintainer would genuinely struggle to read, and that 8%
is four constructs in two files.

So: C1–C3 are worth doing because they are free. **C4 is worth doing because it is the only
phase that prevents a seventh round from undoing the first six.** And C5 is worth doing
because it is the real thing the original instinct was pointing at.

---

# Implementation record — 2026-07-30

Executed on an uncommitted working tree (7,933 insertions pre-existing), with a tarball
backup at `.waves/overhaul/backup/`. Baseline before: **573 tests passing**.

## Result vs. projection

| metric | before | after | plan projected |
|---|---|---|---|
| non-test Rust lines | 21,845 | 21,697 | — |
| comment lines | 3,672 | 3,449 | ~2,900 |
| ratio | 16.0% | **15.9%** | 11–12% |
| past-tense narration sites | 19 | **0** | 0 |
| tests | 573 | **574** | — |

**The projection was wrong and the reason matters.** The plan assumed ~500 lines of
category-D narration were freely deletable. In practice the workers — each required to
justify every deletion — found that most historical narration was the *only* record of a
constraint that still binds, and converted it to present tense rather than removing it.
Narration is gone (19 → 0); volume barely moved. Of the ~223 comment lines removed, roughly
200 is the C3 relocation.

The estimate came from an audit sample, not from line-by-line reading. Reading changed it.

## What shipped

- **C1/C2** — 16 files, all verified by stripped-diff to contain **zero code changes**.
- **C3** — `docs/calibration.md` created (262 lines) from two extracts; five in-code anchors
  verified to resolve. Every measured figure copied before deletion.
- **C4** — `scripts/check-comments.mjs` + 7 unit tests, wired as `comments:check`.
  Errors on past-tense narration; warns above 20 lines. Deliberately does **not** match
  "no longer" — every occurrence in this repo describes current behaviour, and a test pins
  that so nobody "improves" the pattern list.
- **C5.1** — `release_report.rs` `Blocked` arm merged into `Review`. 26 lines → 13.
- **C5.2** — `CountedColumn`'s four interacting fields → `ProjectionWitness`
  (`Unseen | Constant(String) | Varied`) + `rows_yielded`. The invariant "varied implies
  yielded" now holds by construction, and the manual `first_projection = None` memory
  release became automatic via the unit variant.

## What was NOT done, and why

- **C5.3 (`Option<Cow>` → `Projected` enum): skipped.** On reading the call sites, `Option`
  is not accidental — `released.get(position)` returns one, and `.and_then(apply)`
  deliberately unifies two different absences (cell missing from a short row, projection
  not applicable), which the comment at the site states is correct. A named enum would need
  converting back to `Option` to compose, or a third variant. Both are worse. The plan item
  was written from an audit summary rather than from the composition; it was wrong.
- **C5.4** was downstream of C5.2 and no longer identifiable as a separate defect.

## Two things found during implementation

- **A test coverage gap, found by mutation.** Making `Constant("")` count as yielded left
  all 573 tests green. The `yielded` branch is read only when `rows_measured < 2`, and no
  test covered a one-row file whose projection is empty. Added
  `a_single_row_does_not_name_a_column_whose_projection_is_empty`; re-running the mutation
  now fails exactly that test. The gap pre-dated the refactor.
- **`docs:rustdoc` caught an orphaned intra-doc link** that the C5.2 refactor introduced
  (`[`CountedColumn::varied`]`). Fixed. This is the check earning its place in the gate.

## Gate at completion

`cargo fmt` OK · `clippy -D warnings` 0 issues · `cargo test --workspace` 574/0 ·
`docs:check` 19 files · `contracts:check` 17 enums/38 structs/2 limits ·
`docs:rustdoc` 0 errors · `comments:check` OK · `frontend:test` 127/127

Nothing committed.
