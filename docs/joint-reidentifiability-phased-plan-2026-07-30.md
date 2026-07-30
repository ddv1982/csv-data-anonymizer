# Joint Re-identifiability Phased Plan - 2026-07-30

## Scope

This plan addresses the one finding from the 2026-07-30 improvement survey that was deliberately
left unimplemented, recorded as the open item in
[`review-remediation-phased-plan-2026-07-30.md`](review-remediation-phased-plan-2026-07-30.md):

> Nothing measures joint re-identifiability across the columns actually written out.

Every privacy figure the tool reports today is **per column**. `ColumnValueDistribution` says what
one column's pseudonyms leak about that column. `report_identifier_class_for_column` counts direct
and quasi identifiers one at a time. `preview_warning_for_column` warns one column at a time. The
release report can therefore say "no high or medium risk column was left unselected" about a file
in which postcode, birth date and job title together single out a third of the rows — because no
code path ever looks at more than one column at once.

The deliverable is a figure the tool does not have: **how many rows in the file being handed over
are unique on the columns an outsider could match against data they already hold.** It turns
"3 quasi-identifiers" into "412 of 5,000 rows are unique on the columns you are releasing."

This is the first change in this series that alters what the product *claims about its output*
rather than fixing a defect in what it does. That is why it was split out and why Phase 2 —
the phase that turns the number into a readiness claim — is separable from Phase 1, which only
measures.

### Not in scope

- **Changing detection.** The "disclose only" decision of 2026-07-30 still holds: `sampleRowCount`
  stays at 100 and sampling stays non-adaptive. This metric is computed on the full streaming
  transform pass, not on the detection sample, so it does not depend on that decision either way.
- **Blocking a release.** See "Decision 3" below. The metric reports; it never refuses.
- **Suppression, generalisation, or any automatic remediation.** The tool will say a file is
  re-identifying. It will not silently alter the file to make the number look better — that would
  change the user's data behind a privacy statistic, which is the opposite of the honesty this
  metric exists to provide.
- **Differential privacy, l-diversity, t-closeness.** A k-anonymity floor is the weakest of the
  standard measures and the only one computable in one bounded streaming pass without asking the
  user to nominate sensitive attributes. Stronger measures are a different product.

## Where This Fits The Existing Code

The pass this hangs off already exists and already ends in the right place.

- `crates/csv-anonymizer-core/src/csv_io.rs:516-554` — `process_csv_reader_to_writer`, the single
  loop that reads, transforms and writes every data row. Blank rows are written through at `:533`
  before the transform, so they are already excluded from anything measured after that point.
- `crates/csv-anonymizer-core/src/strategies/state.rs:12-29` — `TransformState`, which already
  accumulates per-column ledgers across the whole run and already carries a memory budget.
- `crates/csv-anonymizer-core/src/strategies/state.rs:56-64` — `TransformState::report()`, which
  already exists specifically because "a distribution is not a running total": it materialises
  end-of-run figures out of live state. A uniqueness histogram has exactly that shape.
- `crates/csv-anonymizer-core/src/service/privacy_report.rs:10-50` — `build_privacy_report`, the
  single funnel every path uses to turn a `TransformReport` into a `PrivacyReport`.

Six call sites feed that funnel: `service.rs:248` (CSV file and preview), `direct_input/csv_text.rs:120`,
`direct_input/xml.rs:92`, `direct_input/quick.rs:84`, `direct_input/text.rs:142`,
`direct_input/documents.rs:102`. Only the tabular ones have rows to measure — see Decision 4.

Because the tracker lives in `TransformState`, every path that drives `transform_row_with_state`
gets it without new plumbing, and every path that does not drive rows at all gets `None` rather
than a fabricated zero.

## Decisions Taken Before Planning

These four shape every phase below. Each is reversible, and each is written down because getting
it wrong produces a metric that is confidently misleading — worse than no metric.

### Decision 1: Measure the released values, on the linkable subset of columns

**Revised during Phase 1.** The rule below was written to reuse
`DataType::pseudonymization_preserves_structure()`. Implementing it showed that predicate is
the wrong instrument: it returns `Some` for almost every type, because approximate length
survival counts as structure. Reusing it would have put nearly every pseudonymized column
into the subset and made the metric fire on nearly every file — the cry-wolf failure this
decision exists to prevent.

The shipped rule turns on a sharper question: **given the original value, can someone compute
or recognise what we released?** A pseudonymized address keeps a length within about 20%,
which nobody can filter on; a pseudonymized email keeps its domain verbatim, which anybody
can. Partially reproducible transforms are *projected* rather than included whole — a
pseudonymized email column is hashed on its domain alone, since hashing the whole cell would
report every row as unique on a column that in truth only sorts rows by employer.

The shipped classification lives in `crates/csv-anonymizer-core/src/uniqueness.rs` with the
reasoning per arm.

**Corrected again, same day.** The first shipped version excluded format-only survivors — a
digit count, a phone separator layout, a count of name parts — on the grounds that each has
too few distinct values to be what singles a row out. That was wrong for two reasons, and both
are worth naming because they are easy mistakes to repeat:

- It **predicted the size of an effect instead of measuring it**, in a measure whose entire
  premise is that individually weak signals combine into a strong one. The regression test
  `shape_signals_too_weak_alone_still_combine` is a five-row file where a phone layout splits
  it 3/2, a name-part count splits it 2/3, neither singles anybody out alone — the test proves
  that rather than asserting it — and their intersection holds one row. The excluding version
  reported that file as having no unique rows.
- It **dropped data from the measurement to fix a presentation problem.** The real concern was
  that naming a pseudonymized `customer_id` beside a released `postal_code` would credit the
  id's width with work it did not do, and send a reader to remove the wrong column. That is now
  fixed where it belongs: `shape_only_columns` is reported separately from `linkable_columns`,
  both are counted, and the report says "postal_code, and the surviving format of customer_id".
  That is the fixture in
  `service::tests::anonymize::the_finding_separates_released_values_from_surviving_formats`,
  which pins the phrasing.

The line is now **exact reproducibility**, which rests on the transform rather than on a
judgment about worth. What stays excluded is the generic-string path, whose pseudonym is
80–120% of the original's length: an approximate length cannot be reproduced, so there is no
filter an outsider could apply and nothing to count.

The metric is computed on the **output** row, not the input row. That is the file the user is
handing over, and it means redaction and masking automatically get credit for the discriminating
power they actually removed rather than the power we assume they removed.

But not every released column is *linkable*. An opaque token on a primary key makes every row
unique by construction, and that uniqueness helps an attacker not at all — they cannot match
`a7f3c2` against anything they hold. Including such columns would fire the alarm on essentially
every file with a primary key, and an alarm that always fires carries no information.

A column joins the linkable subset iff its released value can be matched against outside
knowledge:

| Strategy | In subset | Why |
| --- | --- | --- |
| `PassThrough` | yes | The value is the original. |
| `Mask` | yes | Partial original survives, and we now disclose that it does. |
| `Auto` / `Pseudonymize` | **projected, per type** | As revised above. Partial-value: `Email` on its domain, `Timestamp` on the decade of its released date plus its time of day (see Revisions 2 and 3). Format-only, counted and reported apart: `Phone` on its dial layout, `NumericId` on its width and leading zeros, `NumericValue` on its sign and decimal places, the name types on their part count, `Uuid` on its letter case. Excluded: the generic-string types, whose only survivor is an approximate length. A type released untouched — `Enum`, `CountryCode`, `Boolean`, `Currency`, `Percentage` — is in whole, because the cell *is* the original. |
| `Tokenize` | no | Opaque by construction. |
| `Label` | no | An internal ordinal. Preserves the equality partition, but an attacker needs to know the *value*, not merely that two rows agree. |
| `Redact` | no | Constant. Contributes nothing to any partition. |
| `LocalAi` | **projected, as its fallback** | Corrected — see Revision 1. A value the model produced is invented, but a value its leak guard *refused* takes the pseudonymizing transformers, and the report cannot tell which happened per row. Classified as the fallback, with one difference from `Pseudonymize`: the `uses_default_pass_through` types are not passed through on this path, so they are unlinkable here where they are the original there. |

Consequence to state plainly in the report: **an empty linkable subset is not proof of safety.**
It means nothing in the released file matches outside knowledge *by the rules above*, which is a
claim about strategies, not about the data. Phase 2's wording must not let it read as "anonymous."

A second, cheaper figure — distinctness over *all* released columns — is computed in the same pass
and reported alongside. It answers a different and simpler question ("could this file be shuffled
or aggregated?") and it needs no judgment at all, which makes it a useful check on the first
figure: if the two diverge wildly, the subset rule is doing a lot of work and deserves the reader's
attention.

### Decision 2: Over-report uniqueness, never under-report

Hash collisions merge two genuinely different rows into one equivalence class. That makes classes
look *larger*, which makes the file look *safer*. That is the unsafe direction, so it gets engineered
against rather than documented away.

Key rows on **128 bits** (two `DefaultHasher` passes with distinct domain-separation prefixes) rather
than 64. At 64 bits and 10 million rows the expected collision count is about 2.7 × 10⁻³ — small, but
it is a silent under-statement of risk, and the fix costs one extra hash per row. At 128 bits the
figure is ~10⁻²² and can be dismissed in a doc comment honestly.

No new dependency: `std::collections::hash_map::DefaultHasher` is SipHash-1-3 and is already used
in this repo for the content-derived prompt nonce. As there, the doc comment must say it is not
cryptographic — this is collision avoidance, not tamper resistance.

### Decision 3: Report, do not block

The file-based run writes to a temporary path and publishes atomically, so refusing after the last
row is *technically* possible. It is still wrong:

- There is no threshold that is right for every release. k ≥ 5 is a convention, not a law, and a
  tool that hard-refuses at an arbitrary k teaches users to bypass it.
- Refusal arrives after the entire run. On an hour-long file it destroys the work and offers no
  remedy the user can act on except "release fewer columns" — which the report can simply say.
- The paste and quick paths return their output in memory and are not publishable-then-refusable in
  the same way. A metric that blocks on one path and warns on another is worse than one that warns
  everywhere.

So: `Review`, with the number stated. Phase 2 defines the exact wording.

### Decision 4: Absent, not zero, where rows do not exist

`direct_input/text.rs` and `direct_input/documents.rs` anonymize unstructured prose; there is no
tuple to hash. `direct_input/quick.rs` handles a single value. These must report the metric as
**absent**, never as "0 unique rows" — which would read as a clean bill of health for a path that
never looked.

The DTO is therefore `Option<RowUniquenessSummary>`, and the frontend renders nothing when it is
`None`. This mirrors how `DetectionCoverage` was threaded on 2026-07-30 and should reuse that shape
so there is one idiom rather than two.

## Phase 1: Measure It

Status: completed on 2026-07-30.

Goal: the figure exists, is exact, is bounded in memory, and is observable in the payload and in
tests. Nothing in the UI changes and no readiness claim changes. Shipping measurement before
interpretation means the definition can be validated against real files before the product says
anything to a user based on it.

Targets:

- `crates/csv-anonymizer-core/src/strategies/state.rs` — new `RowUniquenessTracker` held by
  `TransformState`, fed once per transformed row.
- `crates/csv-anonymizer-core/src/strategies/mod.rs:142` — `transform_row_with_state` feeds the
  tracker after building the output row. This is the only place that sees the input row, the
  output row and the column controls together, which is what the subset rule needs.
- `crates/csv-anonymizer-core/src/types.rs` — new `RowUniquenessSummary`, and a
  `Option<RowUniquenessSummary>` field on `TransformReport` (`:1164`) and `PrivacyReport`.
- `crates/csv-anonymizer-core/src/service/privacy_report.rs:10` — carry it through.
- `frontend/src/types.ts` — mirror the struct; `scripts/check-contracts.mjs:99` — register it.

Shape:

```rust
pub struct RowUniquenessSummary {
    /// Data rows hashed. Blank rows are written through untransformed and are not counted.
    pub rows_measured: usize,
    /// Column indices in the linkable subset, so the number below is auditable rather than
    /// asserted. An empty vector means the subset rule found nothing matchable — which is a
    /// claim about strategies, not about the data.
    pub linkable_columns: Vec<usize>,
    /// Distinct equivalence classes over the linkable subset.
    pub distinct_classes: usize,
    /// Rows alone in their class. The headline figure.
    pub unique_rows: usize,
    /// The k-anonymity floor: the smallest class size present.
    pub smallest_class: usize,
    /// The k below which 5% of rows sit — a floor that one freak row cannot dominate.
    pub fifth_percentile_class_size: usize,
    /// The same distinctness over *every* released column, for the shuffling question.
    pub distinct_rows_all_columns: usize,
    /// True when tracking stopped early; every figure above is then a lower bound on risk.
    pub measurement_incomplete: bool,
}
```

Memory bound, and why it is not the mapping ceiling. Worst case is one entry per row in each of two
`HashMap<u128, u32>`. That is real memory and must be bounded, but it must **not** be folded into
`TransformState::mapping_entries`: a redact-only run costs zero mapping entries today, and adding
one per row would start refusing large redact-only files that stream fine now. That is a regression
in the name of a report.

Instead the tracker gets its own ceiling with a different failure mode, and the asymmetry is the
point:

- Exceeding the *mapping* ceiling must refuse the run, because dropping mapping entries silently
  breaks the guarantee that one source value keeps one replacement, and corrupts the output.
- Exceeding the *uniqueness* ceiling costs only a report figure. So it stops tracking, sets
  `measurement_incomplete`, and lets the run finish. "Not measured — the file is larger than the
  uniqueness check holds" is honest and costs the user nothing.

**Shipped as a layout-derived estimate, not a measurement.** The plan asked for a `VmHWM`
measurement in the style of `APPROXIMATE_BYTES_PER_MAPPING_ENTRY` (`state.rs`). The ceiling
shipped at 2,000,000 classes per map — about 150 MB across both — derived from the size of a
`HashMap<u128, u32>` entry rather than read off a running process, and the doc comment says so
plainly.

The reason the weaker method is acceptable here and was not there is the cost of being wrong.
An under-estimate of the mapping's memory gets the process OOM-killed with no message; an
under-estimate here stops a measurement early and reports `measurement_incomplete`. Worth
re-measuring if the ceiling is ever raised, and not worth the harness at this figure.

Tests:

- Hand-built fixtures with a known class structure — a 10-row file with classes of 4/3/2/1 must
  produce `smallest_class = 1`, `unique_rows = 1`, `distinct_classes = 4`.
- The golden case from the survey: a file where three passed-through quasi-identifiers make a known
  number of rows unique, asserted exactly.
- A tokenized primary key must **not** enter the subset — the regression test for Decision 1, and
  the one most likely to be broken by a later well-meaning change.
- A mask column and a structure-preserving pseudonymized timestamp must both enter it.
- An all-redacted release reports an empty subset and one class.
- The non-tabular paths report `None`.
- A run past the tracker ceiling completes, writes correct output, and sets
  `measurement_incomplete`.

Risk: low. Nothing user-visible changes. The one way this phase can go wrong is a subset rule that
disagrees with the table in Decision 1, which is what the third and fourth tests pin down.

## Phase 2: Turn It Into A Claim

Status: completed on 2026-07-30. **This is the phase that changes what the product says.**

Targets: `crates/csv-anonymizer-core/src/release_report.rs`, `report_notes.rs`,
`service/preflight.rs`.

A new evidence item, `row-uniqueness`, whose status follows Decision 3:

- **`Info`** when the linkable subset is empty — with wording that says *why* it is empty
  ("nothing released is matchable by strategy") rather than implying the data is anonymous.
- **`Review`** whenever `unique_rows > 0`, stating the count and the share:
  *"412 of 5,000 released rows (8.2%) are unique on postcode, birth date and job title. Anyone
  holding those three fields for a person can find that person's row."*
- **`Review`** when `unique_rows == 0` but `smallest_class` is small (< 5), stating the floor.
  k = 2 is not safety.
- **`Verified`** only when the linkable subset is empty **or** `fifth_percentile_class_size` clears
  a stated floor. Keeping a reachable `Verified` matters for the same reason it did for `Uuid` on
  2026-07-30: a status that is always `Review` carries no information.
- **`Review`** with an explicit "not measured" detail when `measurement_incomplete` is set. Never
  `Verified` on an unmeasured file.

The readiness rollup at `build_readiness` must fold the `Review` cases into `reviewItems`, and the
existing "no high or medium risk column was left unselected" verified item needs re-reading in this
light — it is the sentence this whole plan exists because of, and it should not sit next to a
finding that contradicts it without acknowledging it.

Risk: medium, and entirely in the wording. A number this sharp is easy to over-claim with. Every
string in this phase should be read against the house rule that documentation is honest about
limits: the metric measures linkage against the columns in *this* file, and says nothing about an
attacker who joins on something we classified as non-linkable.

## Phase 3: Surface It

Status: completed on 2026-07-30.

Targets: `frontend/src/components/privacy-report/`, plus its tests and the a11y spec.

Render the summary in the privacy report when present, absent entirely when `None`. The unique-row
count is the headline; `linkable_columns` renders as the named column list, because a user's first
question will be "which columns?" and the answer is already in the payload.

Frontend work is display-only — no computation, no thresholds duplicated in TypeScript. Any
threshold lives in Rust so the report and the UI cannot disagree.

Risk: low. Existing e2e and a11y mocks will need the new field; that was the exact break that cost
a gate run on 2026-07-30, so update `frontend/e2e/workflow.spec.ts` in the same commit as the DTO,
not after.

## Phase 4: Say Which Column To Drop

Status: shipped 2026-07-30, on the terms below except for the tracker shape and the cap — see
"Phase 4 Shipped" at the end of this document for what changed and why. Optional; genuinely
useful; the only phase that gives the user an action.

"412 rows are unique" prompts "so what do I do?" Leave-one-out attribution answers it: for each
column in the linkable subset, how many rows would be unique if that column were dropped.

Computable in the same single pass with one additional tracker per linkable column (each hashing
the subset minus one column), so it stays one read of the file. Cost is (k + 1) trackers, so bound
it: run the attribution only when the subset is small (≤ 8 columns is a reasonable starting cap)
and skip it with a stated reason otherwise, reusing the incomplete-measurement idiom from Phase 1.

Output: *"Dropping `birth_date` would leave 3 unique rows instead of 412."* That is the sentence
that makes the whole feature actionable rather than merely alarming.

Risk: medium on memory, low on correctness — it is the Phase 1 tracker instantiated k more times,
so it inherits its tests.

## Sequencing

Phases 1 → 2 → 3 are ordered by dependency, not just severity: 2 needs 1's figure and 3 needs 2's
wording. Phase 4 depends only on Phase 1 and can be deferred indefinitely or dropped.

A reasonable minimum ship is Phases 1–3. Phase 1 alone is also a defensible stopping point: it puts
the number in the payload and under test without making any new promise, which is the cheapest way
to find out whether the subset rule survives contact with real files.

## Gate

Unchanged, and every phase must leave it green:

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, `contracts:check`, `docs:check`, `docs:rustdoc`, `deadcode:required`,
`frontend:lint`, `frontend:typecheck`, `frontend:test`, `smoke:rust`, `frontend:e2e`,
`frontend:a11y`.

Phases 1 and 3 touch the contract pair, so `contracts:check` is the one most likely to catch a
half-threaded DTO.

## Completion Evidence - 2026-07-30

Phases 1–3 shipped together. Phase 4 followed after four review rounds on them.

New: `crates/csv-anonymizer-core/src/uniqueness.rs` and its tests,
`RowUniquenessSummary` on `TransformReport` and `PrivacyReport`,
`TransformState::record_released_row`, the `row-uniqueness` evidence item and readiness entry in
`release_report.rs`, and the `JointReIdentifiability` block in `PrivacyReportSummary.tsx`.

The finding that motivated all of it, now under test end to end
(`service::tests::anonymize::the_release_report_states_how_many_rows_the_released_columns_single_out`):
a 20-row file with the name column redacted and postcode, birth date and job title released
untouched. Every per-column check passes. The report now also says:

> 2 of 20 released row(s) (10.0%) are unique on postal_code, birth_date, job_title. Anyone
> holding those fields for a person finds that person's row, however each column reads on its own.

Four wordings are deliberate and are pinned by tests rather than left to a future editor:

- A format-only contributor is named as one: "postal_code, and the surviving format of
  customer_id", never a flat list. Both are counted; only one of them is what singled the row
  out, and a reader acting on the flat list would remove the wrong column.

- A fully redacted release reports **Info — not applicable**, never `Verified`. Nothing was
  matchable *by strategy*, which is not a finding that the rows cannot be re-identified.
- An unmeasured file reports **Review — not measured**, never `Verified`, and its zeroed counts
  are not rendered as findings.
- Preflight emits no item at all. It runs before the transform, so it has no released rows, and
  an item there would be a claim about a file that does not exist yet.

Gate, all green:

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` (7 + 456 + 60, 4 pre-existing ignored), `contracts:check`
(16 enums, 36 structs, 2 limits), `docs:check` (16 files), `docs:rustdoc`, `deadcode:required`,
`frontend:lint`, `frontend:typecheck`, `frontend:test` (16 files, 111 tests),
`smoke:rust`, `frontend:e2e` (4), `frontend:a11y` (1).

## Post-Review Remediation - 2026-07-30

Three reviewers read the shipped diff. They found four high-severity defects, all in this work,
all of which had passed the green gate above. One root cause explains three of them: **no test
covered the `Verified` branch.** Every test pinned a review, an info, or a column list, so the
one branch that issues a green tick on a privacy report was the one branch nothing asserted.

Fixed, each with the test that would have caught it:

- **`Verified` was reachable at k = 2.** `VERIFIED_GROUP_FLOOR` was compared against the fifth
  percentile only, never against `smallest_class`. A file with two people in a group and a
  percentile of 99 passed to the green arm — while the review arm one line above says a group
  this small is not anonymity. Pinned by
  `service::tests::anonymize::a_pair_alone_in_its_group_is_reviewed_and_not_verified`.
- **The verified sentence inverted the percentile**, reading "groups of N or more" where the
  review arm and the frontend both correctly read "or fewer". The figure that qualifies the claim
  was printed as a second reassurance.
- **A column that projected to nothing was still named, and still verified.** The column lists
  were fixed at activation from strategy and detected type, never from what was actually
  extracted, so the empty-subset guard could not fire. Now every projection records whether it
  ever yielded anything, and a silent column is named nowhere. Pinned by
  `uniqueness::tests::a_column_that_yields_nothing_is_named_nowhere`.
- **Local AI columns were classified as leaking nothing.** See Revision 1.
- **The phone projection had no shape-fallback guard**, so it digit-masked generic pseudonyms and
  hashed the leftover random letters — a false alarm, which is the failure that teaches people to
  ignore the true ones. It now calls the same `is_phone_shaped` predicate that decided the
  transform.
- **`measurement_incomplete` fired on the all-column histogram.** That map fills far faster than
  the linkable one by construction, so any file with more than two million distinct rows threw a
  perfectly good joint measure away and printed "not measured". The two ceilings are now reached
  independently, and `distinct_rows_all_columns` became `Option<usize>` — absent, not zero.
- **The verified arm was the only one with no "what was not measured" caveat**, on the one path
  where a reader stops reading.
- **The frontend opened on a conjunction** — "Also counted, by surviving format only…" — when
  there was no value-carrying column for it to follow, and then never said what the figures had
  been counted over at all. Rust had a branch for that case; TypeScript did not.
- **The contract gate compared field names and never types**, so `Option<T>` against `T` was
  unverified. It now also compares optionality, which found three pre-existing mismatches in the
  opposite direction (`detectors`, `privacyFindings`, `privacyEvidence` optional in TypeScript
  against `Vec` in Rust, a value that can never arrive absent). Deliberately optionality only:
  a half-parser of two type languages that silently passes what it cannot read is worse than the
  honest gap it replaces.

### Revision 1: Local AI columns are measured by what their fallback leaks

The original table read "`LocalAi` — no — replacement values are invented". The reasoning in the
code was that a rejected value "is no more reproducible than a pseudonym is", which is true and is
an argument for the opposite conclusion: every projection in this module exists *because*
pseudonyms are reproducible in part.

The path is not rare. With no provider configured, every value in the column falls back; once a
column passes `SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN` distinct values, every later value falls
back too. So the effect was to under-report a whole column's risk — the one direction Decision 2
forbids.

### Revision 2: A pseudonymized date is projected onto its decade, not excluded

This is the one place the exact-reproducibility line is broken on purpose, and it was flagged
before the review as needing a decision rather than a silent resolution.

`transform_timestamp` draws an independent offset of up to ±365 days per source value, so under
exact reproducibility a date-only column projects to the empty string on every row: scored at
zero. Date of birth is the textbook quasi-identifier, so the rule zeroed out the strongest signal
the measure exists to catch — and, before the empty-projection guard above, went on to issue a
verified claim naming the column.

A released **year** was the first repair proposed and is worse than the disease. Two candidate
years and roughly one row per birth date makes nearly every row unique, on nearly every file with
a date column, while the attacker's true candidate set is every row inside their ±1-year window —
frequently dozens. Reporting k = 1 where k is really 30 is not a measurement.

A **decade** is approximate in the same way the attack is approximate. A 1984 date is released
somewhere in 1983–1985, so filtering on "the 1980s" is right unless the original sat within a year
of the boundary. It splits classes across those boundaries and merges rows the attacker would also
struggle to separate, and both errors are the size of the attacker's own uncertainty rather than
an order of magnitude past it. Month and day are dropped, because nothing about them survives the
shift; the time-of-day suffix is kept exactly, and in practice it is the more dangerous half.

Pinned by `uniqueness::tests::date_only_values_link_on_their_decade_and_not_on_nothing`.

### Gate after remediation

All thirteen green: `cargo fmt --all --check`,
`cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
`contracts:check`, `docs:check`, `docs:rustdoc`, `deadcode:required`, `frontend:lint`,
`frontend:typecheck`, `frontend:test`, `smoke:rust`, `frontend:e2e`, `frontend:a11y`.

## Second Review Round - 2026-07-30

The remediation above was itself reviewed by three agents. It held up on the points it was
about — every projection was checked against its transformer's real code, the Local AI mapping
was verified across all 22 `DataType`s, 200,000 generic pseudonyms produced zero false phone
matches, and `linkable_stopped ⟹ all_columns_stopped` turned out to be an invariant rather
than a coincidence — and it had **one defect of its own that was worse than anything it
fixed**, plus a false claim in the wording it had just corrected.

### Revision 3: three kinds of contribution, not two

`RowUniquenessSummary` carried two lists of column indices, value-carrying and format-only.
That cannot express the partial match, which is the *common* case on a pseudonymized file, so
`EmailDomain` and `TimestampDecadeAndTimeOfDay` were classified value-carrying and printed as
bare column names. The verified sentence then read:

> Every released row shares its combination of birth_date, email with at least 23 other(s)

on a file where all 24 released rows were distinct on those two cells. A false sentence under
a green tick, in the arm a reader acts on — the same defect class as the three the first
remediation fixed, one level up.

Replaced by `matched_columns: Vec<MatchedColumn>`, pairing each column index with a
`MatchedPart` (`WholeValue`, `EmailDomain`, `DateDecadeAndTime`, `SurvivingFormat`). Only
`WholeValue` licenses naming a column bare. The report formats what it is told and decides
nothing, so the wording cannot drift from what the projection extracts. The same pairing gives
the decade approximation from Revision 2 a natural place to be disclosed *to the reader*
rather than only in a doc comment, which was the other thing both reviewers asked for:

> Every released row shares postal_code, the domain of email, the decade and time of
> birth_date, and the surviving format of customer_id with at least 7 other(s) … A shifted
> date is matched at decade resolution: someone holding a real date narrows to a two-year
> window inside that decade rather than to the decade itself, so treat this group size as an
> upper bound, and expect it to move between runs when a shift crosses a decade boundary.

Pinned by `service::tests::anonymize::a_partial_match_is_named_as_the_part_it_matched`.

### The wire-format regression, and the gate that demanded it

The first remediation made `detectors`, `privacyFindings` and `privacyEvidence` required in
TypeScript on the stated grounds that "Rust sends `Vec`, which serializes to `[]` on every
report". False: those are the only three `Vec` fields in the checked sources carrying
`#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so the key is **absent** whenever
the list is empty, which is the common case. Three correct declarations were replaced with
three that lie, and `ColumnTable.tsx` survived only on a defensive `?? []` that the new gate's
own error message invited removing.

The gate was the cause, not just a bystander: modelling optionality as `Option<T>` versus `?`
made it **fail on the correct declaration and pass on the incorrect one**. It now asks a
one-directional question — *can TypeScript hold everything the wire can send?* — answered from
the serde attributes:

| Rust | Wire | TypeScript must |
| --- | --- | --- |
| `skip_serializing_if` | key absent | be optional (`?`) |
| `Option<T>`, no skip | `null`, key present | admit `null` |
| neither | always present | be neither |

A third defect in the same change: the new field regex required the type on the same line, so
a rustfmt-wrapped field became invisible to the *name* comparison as well — narrowing a check
that had worked. Declarations are now accumulated across lines. All four cases are covered by
`scripts/__tests__/check-contracts.test.mjs`, each verified to fail before the fix.

### Also fixed

- **`'#'` was both the digit placeholder and an accepted phone character** (the DTMF hash), so
  `"0612345678#"` and `"06123456785"` collapsed to the same key and two separately-filterable
  dial layouts merged into one class. Merging under-states risk, which is the direction
  `hash_fields` was widened to 128 bits to avoid, reintroduced one level above the hash.
- **`unique_share` printed "0.0%"** for 1 unique row in 10,000. Now "under 0.1%".
- **Half the new floor check was unreachable.** `fifth_percentile_class_size` is drawn from a
  list whose first element *is* `smallest_class`, so it can never be the smaller of the two.
  The disjunct is gone and the comment says why the percentile is reported rather than gated.
- **`distinct_rows_all_columns` was computed, budgeted and rendered nowhere** — a second 2M-entry
  map and a second whole-row hash per row, for a figure no prose and no JSX read. Now shown as
  a fifth metric, and its own test asserts presence *and* absence rather than passing either way.

### Test weaknesses the round exposed

Each of these passed while failing to constrain what it claimed. They are the reason the round
was worth running:

- The verified fixture used four groups of six, making `smallest_class` and
  `fifth_percentile_class_size` both 6 — so swapping the two figures in the sentence left the
  whole suite green, on the one test whose job was to catch that confusion. Now 5/50/50.
- The below-floor sentence was pinned only by `contains("holds 2 row(s)")`. A reviewer replaced
  it with an inverted, reassuring sentence ending "which is fine" and every test passed. Now
  asserted whole.
- The `measurement_incomplete` prose had no report-layer test at all.
- The frontend `distinctRowsAllColumns` test was **vacuous** — the component never read the
  field, so changing `null` to `999999` left 114 tests green.
- All three phone-guard fixtures had fewer than 7 digits, so they exited `is_phone_shaped` on
  the digit count and never reached the character-set clause. Deleting that clause would have
  left them green.

### Third Round: the deferred list, cleared - 2026-07-30

Every item previously carried as "knowingly not fixed" was reconsidered and fixed, except one
that turned out not to be a defect. None of these was reachable on a realistic file; they were
taken because a list of known-wrong-but-rare behaviours is how a measure stops being trusted.

- **A column name containing a comma is now quoted.** The names are comma-joined, so `city,
  state` read as two columns and disagreed with the "N columns" label with nothing to say
  which was wrong.
- **A year outside `0..=9999` is read as a date.** `chrono` writes one in expanded form,
  `+10000-01-01`, which a fixed ten-character prefix rejected — so such a row projected to
  nothing and merged with every unparseable row in the file. Merging under-states risk. The
  parser now takes four *or more* year digits and an optional sign, and a negative year keeps
  its sign in the decade so 1980 BCE cannot share a class with 1980 CE.
- **A released UUID with no letters gets its own class.** It is upper case and lower case at
  once, so folding it into either merged it with values whose original case is known. Its own
  class over-states instead, which is both the permitted direction and the honest reading —
  an outsider filtering on case cannot place such a value either.
- **`contributed` keeps updating after the ceiling stops the histogram.** The column list is
  still reported once measurement stops, so the flags behind it have to stay true of the whole
  file. Only columns still silent are re-checked, which after the first few rows is none.
- **Class counts saturate.** Wrapping the `u32` would report the largest class in the file as
  the smallest, under a verified tick. Unreachable below 4.29 billion identical rows.
- **Rebinding the tracker's columns mid-run is caught by a `debug_assert`.** Every recorded
  position would otherwise describe a different column than the value being read. Asserted
  rather than defended against, because the honest repair for a varying shape is a second
  tracker, not a mid-run rebind.

## Fourth Review Round - 2026-07-30

Three read-only reviewers over the whole uncommitted diff. Read-only on purpose: in round two
two agents edited shared files concurrently and one reported a defect that was really the
other's temporary edit. They still ran the suite and built counterfactuals under `/tmp`.

They found four HIGH defects. Two were in code the third round had just written, and one was
in a *test* the third round had just written — which is the pattern of this whole exercise:
the tests were consistently weaker than they looked.

### The measure was still naming columns it had no right to name

- **A masked column was named bare.** `Mask` mapped to `WholeValue`, which is the one class
  that licenses printing a column name on its own, so a file with a masked name column was
  reported as *"Every released row shares full_name, city with at least 7 other(s)"* — telling
  the reader the names are in the file when what is in the file is `****** *******`. The mask
  now has its own projection reported as a surviving format, which is exactly what it is.
- **A published blank-cell pattern was scored at zero.** `transform_value_with_state` returns
  an empty-ish cell *verbatim before any strategy runs*, so a redacted, labelled or tokenized
  column still publishes which of its rows were blank, and whether each wrote `""`, `NULL` or
  `null`. Someone holding the original record knows which of its fields were blank, so this
  meets the same rule as every other projection. Demonstrated: four rows, three redacted
  columns, every released row distinguishable by its null pattern alone, and the report
  printing *"No released column carries anything an outsider could match against data they
  already hold."* Every column now carries `BlankCellPattern` where it survives no other way,
  and `LinkableProjection::for_column` is total — there is no silent exemption left.
- **A constant projection was still named.** Four projections never return an empty string —
  an empty cell reads as `0:0` under `NumericIdWidth` — so three all-blank columns satisfied
  the "contributed" test and a verified finding named all three. The test is now whether the
  projection ever *varied*, with the weaker one below two rows, because on a one-row file
  nothing can vary and dropping every column would report "nothing is matchable" about a row
  that is trivially unique.

### The wording was still ambiguous past two groups

Groups were comma-joined with an Oxford comma, which marks only the *first* boundary:
*"postal_code, city, and the surviving format of phone, customer_id"* reads `customer_id` as a
fourth column released as it stands — the exact misreading the grouping exists to prevent.
Groups are now semicolon-separated, commas being reserved for names within a group. A blank
header now falls back to its position rather than printing as nothing (*"unique on , city"*),
and a name containing a quote is escaped rather than merely wrapped.

### The decade caveat pointed the wrong way on one arm

One caveat string was appended to every arm. On the arm that quotes `unique_rows` and no group
size, *"treat this group size as an upper bound"* had no referent and inverted the direction:
a decade is coarser than the attacker's window, so it *merges* rows, making group sizes too
large and the singled-out count too **small**. A reader took "upper bound" as "at most this
many people are identifiable" when the truth is "at least". The caveat is now worded per arm.

### `NumericValueWidth` threw away a reproducible property

`generate_numeric_component` reproduces the integer part's leading zeros byte for byte and
returns an all-zero component *verbatim*, and the projection read only the widths. So `0.5`
and `4.2` shared a class while anyone holding either could tell them apart on the first
character — reported k = 4 against a true k = 2, under a Verified tick, on any ratio, rate or
percentage column. The sibling `NumericIdWidth` had read its leading zeros all along.

### Three tests that constrained nothing

- **The placeholder test.** `assert!(!is_phone_shaped("\0\0\0\0\0\0\0"))` passes for
  *every* character: seven non-digits fail the digit-count gate before the character set is
  consulted. A reviewer enumerated the 41 characters `is_phone_shaped` accepts alongside seven
  digits and showed `'('`, `'x'` or `' '` as the placeholder would reintroduce the exact
  collision and keep the test green. Now asserted with seven digits present.
- **The unique-rows arm.** Pinned only by the substring "2 of 20". Replacing "however each
  column reads on its own" with "which is fine" left all 477 tests green — the round-three
  defect, relocated to the arm that reports individually identifiable people.
- **`unique_share` had a floor but no ceiling.** Widening its threshold from `0.05` to `50.0`
  left every test green while a 10% exposure rate printed as "under 0.1%".

Plus a fixture that was a dice roll: `a_partial_match_is_named_as_the_part_it_matched` used
dates from 1980–1987, and `1980-04-10` is day 100 of its decade, so about a third of runs
shifted it into 1979 and split the class. The test never flaked, because every assertion was
present in all three arms — but that also meant the arm was unpinned, and the only test
guarding the decade caveat passed roughly two runs in three when the caveat was deleted. The
fixture now uses two mid-year decades that a ±365-day shift cannot carry across a boundary.

### The gate, again

`rustStructFieldTypes` read only the first line of an attribute, so a rustfmt-wrapped
`#[serde(...)]` made `default,` look like a declaration: it flushed as a non-match, discarded
the pending attributes, and the field reached **neither** check. Proven: the gate failed on the
correct declaration and passed on one with the field deleted outright — the round-three
signature in a new form, latent only because all 33 attributes in the repo happen to be
single-line. Attributes now accumulate across lines like declarations do.

Separately, `#[serde(skip)]`, `skip_serializing`, field-level `rename` and `flatten` each
change or remove the wire key, and the rule got all four backwards. None is present in a
registered struct, and an unrecognised one is now a hard error rather than a wrong verdict.

### What the round cleared

Worth recording, because it is most of what three reviewers spent their time on. `decade_and_suffix`
survived an exhaustive fuzz over 177,156 strings with no panic, and the expanded-year path is
genuinely reachable (`9999-12-31` produced `+10000-04-23` on 4 of 8 draws). The Local AI mapping
was verified against **all 22 `DataType`s** by running each through the forced-fallback path.
12 phone originals × 50 redraws all kept an identical projected layout, and **0 of 200,000**
generic pseudonyms passed `is_phone_shaped`. An independent brace-aware parser agreed with the
contract gate on **268 fields across 37 structs, with zero differences**, and all 20
`skip_serializing_if`/`Option` fields were checked by hand against their TypeScript
declarations. The component was mutated eleven ways and the gate five, and every behaviour named
in a test title killed exactly the test naming it — **no vacuous frontend test remained**.
`linkable_stopped ⟹ all_columns_stopped` was proven an invariant rather than a coincidence.

### Knowingly not fixed

> Superseded in part by "Sixth Round: the deferred list, worked through" below, which
> fixed four of these seven items. Read that section before acting on this list.

- `row_uniqueness_evidence` is called twice per report, once from `build_evidence` and once
  from `build_readiness`, and deliberately stays that way. Both callers running the *same*
  function is what makes the evidence row and the readiness item unable to word the finding
  differently — a property a test asserts. Threading a precomputed item through two builder
  signatures used by two call sites each would trade that guarantee for two saved `format!`
  calls per run.
- A column that contributed on some rows and not others is claimed of every row: a `Timestamp`
  column where one value parses and 99 hit the shape fallback is named, and the verified arm
  says "shares the decade and time of birth_date" of 99 rows that have no decade. Over-states
  rather than under-states, so it errs in the permitted direction.
- The `Blocked` arm of `build_readiness` is unreachable and therefore unverifiable — replacing
  its body with `{}` leaves every test green. It is a net for a status this function cannot
  currently return, and only its comment holds it in place.
- A row longer than its metadata publishes uncounted verbatim columns. Unreachable through
  `csv_io`, which normalises every row to the header length, but `transform_row_with_state` is
  `pub` and so reachable from outside the crate.

## Phase 4 Shipped - 2026-07-30

The last phase, and the only one that gives the reader something to do. `RowUniquenessSummary`
gained `drop_column_effects: Vec<DropColumnEffect>` and `drop_attribution_incomplete: bool`;
`release_report::drop_column_advice` and the frontend's `DropColumnAdvice` render the same
finding into the release report and the privacy panel.

### The plan's O(k²) shape was replaced before it was written

The plan budgeted "one additional tracker per linkable column (each hashing the subset minus
one column)", which re-hashes the row once per column and is quadratic in the column count.
That is what forced its cap of "≤ 8 columns is a reasonable starting cap" — and that cap was
written when `LinkableProjection::for_column` still returned `Option`. Round three made it
total, so every column is counted now, and a cap of 8 would have withheld the attribution from
any file with nine columns: nearly all of them.

`component_hash` composes instead. Each column contributes an independent 128-bit hash of
`(position, length, bytes)`, the row's key is the wrapping sum, and the key without column `i`
is the total minus that column's component — so all `k` leave-one-out keys fall out of one pass.
What the sum gives up is sequence, which is why the position is hashed *into* each component;
without it `["a", "b"]` and `["b", "a"]` would share a class, merging two rows, which is the
one direction this module may not be wrong in. The collision argument from `hash_fields`
carries over: uniform independent components sum to a uniform value, so a merge still sits
around 1e-22.

That made the cap a CPU bound rather than a memory one, and it is set at 24.

### Memory is bounded by a shared budget, not a per-column one

`ATTRIBUTION_CLASS_CEILING` is 4,000,000 classes across *all* the leave-one-out histograms
together. Per column it would not compose: twenty-four maps each allowed `CLASS_CEILING` is
3.6 GB, which would make an actionable footnote the largest allocation in the process. Shared,
the attribution costs at most what the two existing histograms cost together — about 150 MB by
the layout arithmetic already in `CLASS_CEILING`'s doc — so the module's whole claim on memory
at most doubles and stays an order of magnitude under the mapping's 5.1 GB.

A wide file with millions of rows therefore loses its attribution and keeps its joint measure.
That is the right way round: the joint measure is the finding and the attribution is the advice
about it.

### Three empty lists that mean different things

`drop_column_effects` is empty when nothing was measured, when the file is wider than the cap,
and when there is no matched column to drop. `drop_attribution_incomplete` is what tells the
first two apart from the third, and it is written out explicitly on the `linkable_stopped`
early return rather than left to `Default` — the field defaulting to `false` there would
publish "we looked and no column helps" about a file nothing was measured on.

The report says so too. Silence on an unmeasured attribution is indistinguishable from "no
column would help", which is the opposite finding and the one that stops a reader looking.

### One predicate, not two

`matched_columns` and `drop_column_effects` are filtered by the same `is_matched`. The report
pairs an effect with the name of a matched column, so two copies of that rule drifting apart
would produce an effect for a column the reader was never told was matched — or advice to drop
a column the finding does not rest on. The file's own earlier comment named this failure:
"a reader who then removes the wrong column has been misled by the report that was supposed to
help them."

### A test fixture that was wrong before the code was

The first hand-worked table asserted that dropping the culprit column would leave one unique
row; it leaves two. The code was right and the arithmetic in the comment was not. Replaced with
a five-row table whose leave-one-out answers are 1, 5 and 5 — a case where the culprit is
unambiguous rather than one where the assertion happened to be close.

## Fifth Review Round - 2026-07-30

Four read-only reviewers over the Phase 4 diff. One found a defect that predates Phase 4.

### A whole `MatchedPart` had fallen out of the sentence

`counted_column_names` built its groups from an array of four variants. `BlankPattern` was
added to the enum in round three, when the missingness leak was made visible, and nothing added
it here — so a column matched only on its blank-cell pattern was counted into the class
arithmetic and then named nowhere. On a file whose matched columns were *all* blank patterns
the list came back empty and the finding read "are unique on ." with nothing after it. The
frontend rendered the group correctly all along, so the report and the panel disagreed about
the same measurement.

An array literal is a wildcard arm wearing a disguise: it compiles no matter which variants it
forgets. Replaced with `group_order`, an exhaustive `match`, so a variant added later breaks the
build — which is the rule the rest of the module already followed. Two tests now cover it, one
per variant.

### The advice named an action that would have made things worse

"Dropping birth_date would leave 3 unique rows" does not say what dropping *is*, and the two
things a reader can do in this app are both something else. Unticking a column writes it
through unchanged, which `for_column` then reads as `WholeValue` — so a reader who took "drop"
to mean "untick" would have released the raw dates. Redacting is closer and still short,
because the blank-cell pattern survives it.

Now "Removing `birth_date` from the file", which is the measured counterfactual. The sentence
also ends by saying the group sizes behind the new count are not re-measured: `unique_rows_without`
counts only rows standing alone, so it can reach zero while every remaining group is a pair,
which this same item calls "not anonymity" three lines further down.

### Two doc comments were arithmetically wrong

The per-column ceiling was quoted at 3.6 GB; 24 × 2,000,000 × 38 bytes is 1.8 GB. And "the
attribution costs at most what the two histograms cost together" assumed a 0.875 load factor
that twenty-four independently-grown tables cannot all sit at — just past a doubling it is
nearer 0.44, so the honest figure is about twice that, not the same.

More seriously, the claim about *when* the ceiling bites was wrong in a way that mattered. The
doc said "a wide file with millions of rows". The real crossover is the ceiling divided by the
column count, because every counted column gets a histogram — including the ones `is_matched`
will never report, and a column whose projection is *constant* fills fastest of all, since its
key is the row total minus a fixed component and therefore a bijection holding one entry per
distinct row. That is about 167,000 rows at the column cap and 667,000 for an ordinary
six-column file: both below the joint measure's own ceiling, so on mid-sized files the
attribution stops first. Now stated as such.

### Mutation testing, actually run this time

The mutation slice was routed to a read-only agent, which could only derive its verdicts. Run
properly afterwards: 20 mutations, 18 killed, and the first pass was invalid because the report
wording had changed without its test — a red baseline that made every mutation look killed.

Three survived and two were fixed:

- Removing the sort's tie-break changed nothing, because every fixture gave its columns an
  `index` equal to its position. `CountedColumn` keeps the two apart precisely because metadata
  need not be a dense ascending prefix of the row; the new test is the input where that is
  observable.
- Giving `BlankPattern` the same `group_order` slot as `WholeValue` changed only reading order,
  which nothing asserted. Now pinned across all five variants.
- Deleting `field.len().hash()` from `component_hash_with_seed` survives and always will:
  `field.as_bytes()` hashes through `impl Hash for [T]`, which writes its own length prefix. An
  equivalent mutant, verified as one and now documented as one rather than left looking like a
  coverage gap.

### Four tests of mine that constrained nothing

- `no_dropped_column_raises_the_unique_count` made every row distinct, so `unique_rows` equalled
  the row count and each map could hold at most that many singletons: `n <= n` held whatever the
  code did, including keying every row into its own class. Rebuilt so the baseline is 2 of 6.
- `a_stopped_measurement_reports_no_attribution` set `linkable_stopped` and called
  `stop_attribution()` by hand, then asserted the state those calls produce. It proved `summary`
  reads a flag, not that anything sets it — deleting the real call from `record_row` left it
  green. Now driven through the ceiling.
- `a_file_wider_than_the_cap_is_measured_without_attribution` built its fixture from the
  constant, so it scaled with it and proved only that cap+1 exceeds cap. Now literals, and both
  sides of the boundary.
- `a_file_with_no_unique_rows_reports_effects_that_reduce_nothing` used `.all()`, which is
  vacuous on an empty list, and its docstring claimed a report sentence that arm cannot produce.

`ATTRIBUTION_CLASS_CEILING` had no test at all — it could be deleted, set to `usize::MAX`, or
never incremented towards, with everything green. Its sibling `CLASS_CEILING` has had one since
Phase 1.

### The contract gate, proven rather than assumed

A reviewer copied `scripts/` and `types.ts` to a scratch directory and killed six mutations
against the copy: a renamed TS field, a nullable array, an optional array, a deleted bool, a
removed `rename_all`, and a rustfmt-wrapped `#[serde(skip_serializing_if = ...)]` split across
four lines — the exact hole round four found. It is closed.

### Knowingly not fixed, Phase 4

> Superseded in part by "Sixth Round: the deferred list, worked through" below, which
> fixed four of these seven items. Read that section before acting on this list.

- The budget is spent partly on histograms that are never read. Fixing it properly needs the
  joint histogram keyed additively too, so a column's map can be rehydrated from it at the row
  where the column first varies. That is a re-architecture of reviewed code for a bound that
  currently only costs advice, not findings, so it is written down rather than done.
- `toLocaleString` in the panel groups digits by host locale while the release report prints raw
  integers, so one figure reads "4,123" in one document and "4123" in the other. Pre-existing
  house divergence, not introduced here.
- The counterfactual is over the linkable subset only, like every other figure in the item, and
  only the Verified arm says so in as many words.

### Gate after the fifth round

All fourteen checks green. 500 core Rust tests (483 before Phase 4), 60 tauri, 125 frontend
tests (118 before), 19 contract-gate tests.

## Sixth Round: the deferred list, worked through - 2026-07-30

Four read-only workers mapped the seven items carried as "knowingly not fixed" — the three from
Phase 4 and the four from round four — against current code. Four items were fixed, three were
kept with sharper reasons, and the mapping turned up two defects that were on nobody's list.

### A raw value could leave the crate through the public transform

`transform_row_values` looked its metadata up by row index and returned `value.clone()` when
there was none, so a row longer than its metadata published verbatim cells that no strategy
chose and no privacy figure counted. The extras are not in `counted`, so the joint measure is
blind to them; they reach only the all-columns histogram.

What decided the fix is that the crate already has a policy for this data and states it in
`csv_io::normalize_data_row`: a row with non-empty cells beyond the header is refused outright,
"non-empty data beyond the header cannot be safely modeled or written". That refusal is what
made the defect unreachable through the app — and `transform_row_with_state` is `pub`, returns
no `Result`, and `state.rs` documents callers driving their own rows through it. It cannot
refuse, so it now writes the only cell that leaks nothing. Blank rather than truncated, so the
row keeps the arity the caller handed in.

### The readiness status was a constant dressed as a decision

Not on the list; found while mapping the unreachable `Blocked` arm. `build_readiness` computed
`if review_items.is_empty() { Verified } else { Review }`, and the "not a formal anonymity
guarantee" caveat is pushed unconditionally — so the list is never empty and `Verified` is
unreachable. Replacing the whole conditional with the constant `Review` left all 501 tests
green, which is how it was confirmed rather than argued.

It is now written as the constant it is, and a test pins the stance on the most favourable
fixture in the suite: a file that verifies its row-uniqueness item still comes back `Review`.
Anyone who makes that caveat conditional has to come back and decide, out loud, whether this
tool may now certify a file as anonymous.

### A part only some rows carry was claimed of all of them

The round-four item, and the one that needed a new fact rather than new wording.
`LinkableProjection::for_column` decides a column's `MatchedPart` from its strategy and
detected type alone — no cell value can change it — so a `Timestamp` column where one value in
a hundred parses is `DateDecadeAndTime`, and the finding said the rows "share the decade and
time of birth_date" of ninety-nine rows carrying no decade.

The counts were never wrong, and that decided the wording. Those rows project to nothing and
land in one class together, which is exactly what an outsider holding the originals gets. So
the caveat qualifies the phrase and says the arithmetic already accounts for it, rather than
casting doubt on a sound figure.

**The first attempt at it was wrong, and the suite said so.** Counting rows where the
projection was non-empty reported every blank-pattern column in the file as partial:
`BlankCellPattern` returns the empty string for a cell with something in it, and "not blank" is
the projection succeeding. `WholeValue` has the same shape on a genuinely empty cell. Encoding
that in a second method beside `apply` would have let the two drift, so `apply` now returns
`Option<Cow<'_, str>>` — `None` for the three shape-gated fallbacks, `Some("")` where empty is
an answer. It hashes identically, so no count moved.

### The panel disagreed with itself about number formatting

The recorded item said the panel groups digits and the release report does not. Mapping it
found the frontend already internally consistent — 25 of 25 production sites group — but that
`build_utility_metrics` sends `reused_pseudonym_values.to_string()` as a `UtilityMetric.value`,
and `formatMetricValue` passed strings through untouched. So the Utility grid rendered a bare
`999999` beside a neighbouring grouped figure, in the same grid, from the same helper.
`formatMetricValue` now groups a value that is entirely digits and inside the safe-integer
range, leaving ratios like `3/7` alone.

### The counterfactual now states its own scope

`drop_column_advice` is the one arm that hands the reader a number to act on, and it was the
one arm that did not say what the number was counted over — the scope appeared only on the
Verified arm, which a reviewed file never reaches. Rewording it broke no test, which is its own
finding: the assertion stopped at the figures. It now says "counted over the same columns as
the figures above and no others", in both the report and the panel, with both pinned.

### Knowingly not fixed, and why the reasons are better than they were

- **The attribution budget.** The rehydration fix was mapped properly and is worse than it
  looked. The map to re-key is the *linkable* histogram, not the all-columns one — the latter
  hashes raw `released` cells across every position and shares no key material with the
  attribution, so it could never rehydrate it. Doing it to the linkable map collapses the joint
  and attribution key spaces *by construction*, which is exactly the domain separation
  `component_hash` documents; it invalidates `the_two_histograms_are_domain_separated` and
  moves what `field_boundaries_are_not_hashable_away` guards; `first_projection` is discarded on
  the same line `varied` is set, so the constant needed for the shift is destroyed at the row
  it is needed; and a bulk rehydration inserts up to 2,000,000 entries at once, making the
  documented once-per-row overshoot bound of twenty-four false. Still advice, not findings.
- **Panel-versus-report grouping.** Removing grouping from the frontend touches 25 sites and
  breaks two tests whose stated purpose is to pin that formatting happens; adding grouping to
  Rust changes a plain-text document meant to be diffable. The internal inconsistency is fixed;
  this one stays.
- **The double call of `row_uniqueness_evidence`.** Unchanged: both callers running the same
  function is what makes the evidence row and the readiness item unable to word the finding
  differently, and a test asserts it.
- **The unreachable `Blocked` arm.** Making it unrepresentable needs a second status enum
  narrower than `ReleaseEvidenceStatus`, but the item it wraps is a full `ReleaseEvidenceItem`
  that `build_evidence` consumes, so the parallel type would have to be converted at the
  boundary. Machinery for a net. The comment holds it; the newly-constant status beside it no
  longer pretends to be a decision.

### Gate after the sixth round

All fourteen checks green. 506 core Rust tests (500 after round five), 60 tauri, 127 frontend
tests (125 after round five), 19 contract-gate tests. The contract gate accepted
`matchedEveryRow` only once both sides declared it.

## Seventh Round: the last three, decided - 2026-07-30

Three workers designed fixes for the three items still carried as "knowingly not fixed", under
an explicit instruction to prefer simplicity. Two were fixed; one was decided against with a
better reason than the one on record. All three items had something wrong recorded about them.

### There is no exported release report

The formatting item said the panel groups digits while "the release report prints raw integers,
so one figure reads 4,123 in one document and 4123 in the other". There is only one document.
Nothing in the tree writes report text to disk or clipboard — every `fs::write` is inside a
test — and `release_report.rs` builds structured data that the React panel is the sole renderer
of. Comments in `PrivacyReportSummary.tsx` referring to "the exported report" were wrong and
have been corrected.

The real defect was worse and simpler. Rust builds whole sentences that the panel renders
verbatim: evidence details, readiness review items, notes. `drop_column_advice` is the sharp
case — its sentence lands in the readiness review list while the React `DropColumnAdvice`
renders *the same sentence* in the disclosure above it. With grouping on the React side a
reader saw "instead of 4,123" a few centimetres above "instead of 4123".

Fixed by removing `toLocaleString()` from the privacy-report surface, so React prints what Rust
printed. That deletes `formatMetricValue` rather than extending it — which means the round-six
change to that helper was right about the symptom and wrong about the direction: making the
Utility grid self-consistent made it disagree *harder* with the Rust sentences beside it. The
rule is tied to a real property of this surface, not to taste: Rust strings render here, Rust
has no locale, so the raw integer is the only rendering both sides can agree on. It also makes
the panel locale-invariant — two tests had been computing their expectations with
`toLocaleString()` at assertion time purely to survive being run on a Dutch machine, and now
assert plain literals.

The alternative — grouping on the Rust side with a locale-neutral separator — was rejected as
more surface to get wrong: ~31 interpolation clusters across two files, each needing a
count-versus-ratio judgement, plus a matching change in TypeScript.

### The `Blocked` arm's comment gave the wrong reason

`ReleaseEvidenceStatus::Blocked` is not dead crate-wide: `service::preflight` raises it for an
unwritable output path and for Local AI not being ready. Only this one match arm is unreachable.

More importantly, the comment claimed that surfacing a blocker as a review item "under-states
it". It does not — it is the only thing the return type can express. Readiness blocks on
`blockers`, and at both preflight sites that vector is pushed *alongside* the blocked evidence
item, never because of it; `build_readiness` constructs `blockers: Vec::new()` and has no such
vector. The two `Blocked`s are structurally decoupled, so whoever first makes this measure
block must add the blocker at the readiness level too — and finding this arm is how they learn
that. The comment now says so.

The narrower-enum option was rejected on a ground not previously identified: it needs a `From`
impl at the `build_evidence` boundary, and that impl is a wildcard in disguise — it would map a
future variant silently instead of breaking the build, which is exactly the failure that lost
`BlankPattern` from the report in round five. It could not change the wire type either;
`ReleaseEvidenceStatus` is contract-pinned to four strings in `frontend/src/types.ts`.

### The attribution fix is cheaper than recorded, and still not taken

The recorded reason was wrong. Rehydration does **not** require the joint histogram to be
keyed additively: a still-constant column's key is `total - c` for one fixed `c`, so a bounded
buffer of the prefix rows' `total` values reconstructs its map exactly, touching the joint
histogram not at all. Domain separation survives, both hash tests survive, and holding
`constant_component: Option<u128>` is cheaper than the `first_projection` string it replaces.
The design is about 45 lines and would move the six-column crossover from ~667,000 rows to
~1,000,000 while making any file inside the buffer always fully attributed.

Decided against anyway, on the cost of being wrong rather than the cost of building. One
three-line loop currently maintains "map *i* holds one entry per row processed"; lazy
rehydration splits that across two paths that must agree, and a map that comes out short lowers
`unique_rows_without` — printing a stronger, more actionable and *falsely reassuring* claim, in
the one direction this module forbids. The existing guard (`no_dropped_column_raises_the_unique_count`)
only checks effects are not too high, so it would not catch it. For a bound that costs advice
and never a finding, and whose current failure mode is an honest "not measured on this file",
that is a poor trade. The constant's doc now records the real design and the real reason.

### Gate after the seventh round

All fourteen checks green. 506 core Rust tests, 60 tauri, 127 frontend tests, 19 contract-gate
tests. No test count moved: the two grouping tests were rewritten to pin the new rule rather
than deleted.
