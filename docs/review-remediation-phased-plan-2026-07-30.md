# Review Remediation Phased Plan - 2026-07-30

## Scope

This plan remediates four findings from the full-codebase review on 2026-07-30. It is the implementation checklist for that review, in the same relationship to it as [`cleanup-phased-plan-2026-07-01.md`](cleanup-phased-plan-2026-07-01.md) had to its own review.

One phase changes privacy behavior (Phase 1), one adds disclosure without changing detection (Phase 2), one changes frontend job-polling resilience (Phase 3), and one batches four small cleanups (Phase 4). Phases are independently shippable and ordered by severity, not by dependency: none blocks another.

All four phases were implemented on 2026-07-30. Per-phase status, a correction to what Phase 2 assumed, a decision Phase 4 left open, and the completion evidence are recorded below.

Two decisions were taken before planning and constrain what follows:

- **Detection stays as it is.** Phase 2 discloses sample coverage. It does not raise `sampleRowCount` above its current default of 100, and it does not make sampling adaptive. Detection results, detector fixtures, the locale/multilingual matrices, and the preflight cardinality math are all out of scope.
- **All four findings are in scope**, with the four smaller items batched into Phase 4.

Not in scope: the two documented accepted limitations reviewed and left as-is — the grant-time/write-time TOCTOU window at `src-tauri/src/path_access.rs:82-85` and the uninterruptible Local AI request at `src-tauri/src/local_ai/provider.rs:62-64`. Both are correctly reasoned and correctly scoped for a single-user desktop threat model.

## Current Status

The repository is clean at `v1.0.81`. Every gate passes on the pre-work baseline, so any failure during this work is caused by this work:

- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo test --workspace` — 398 tests pass
- `npm run frontend:test` — 99 tests pass across 15 files
- `npm run frontend:lint`, `npm run frontend:typecheck` — clean
- `npm run cargo:machete`, `npm run frontend:deadcode` — no unused dependencies or exports

Two findings were confirmed by execution rather than by reading, and those confirmations are the regression targets for Phases 1 and 2.

**Finding 1 (high), cross-value leak.** `validated_replacements` accepts a replacement that is another row's real value. Driving the function directly with a two-value batch produced:

```
accepted   = [("alice@corp.com", "bob@corp.com"), ("bob@corp.com", "carol@example.com")]
rejections = []
```

`alice@corp.com`'s row receives a real address from the same column, and the privacy report records zero rejections, so nothing signals it.

**Finding 2 (medium), undisclosed detection coverage.** The same column, same detector, differing only in how much of the file detection sampled:

| Input | `notes` column verdict |
| --- | --- |
| 100 rows, one email at row 80 | High risk, auto-selected |
| 5,000 rows, one email at row 4,000 | Low risk (`Enum`), not selected |

In the second case the email is written to the output unchanged, and nothing tells the user detection examined a fraction of the rows.

## Phase 1: Close The Smart Replacement Cross-Value Leak

Status: completed on 2026-07-30.

Goal: no accepted Smart replacement may be, or contain, any source value from its batch — not only its own original. Rejections must be visible in the privacy report under their own reason.

Targets:

- `crates/csv-anonymizer-core/src/smart.rs:434-457` (`invalid_replacement_reason`) and `:382-432` (`validated_replacements`), which already builds `expected_by_key` over every original in the batch.
- `crates/csv-anonymizer-core/src/types.rs:1093-1102` (`SmartReplacementRejectionReason`).
- `crates/csv-anonymizer-core/src/release_report.rs:474-475`, which maps each reason to a report label.
- `frontend/src/types.ts:53-61`, the mirrored union type.
- `crates/csv-anonymizer-core/src/service/tests/smart_replacement.rs` and `crates/csv-anonymizer-core/src/direct_input/tests.rs:1124`.

Recommended shape:

- Add one variant — `MatchesOtherOriginal` is the suggested name — rather than widening `ContainsOriginal`. The two say different things in a report: `ContainsOriginal` means the model echoed the value it was asked to replace, `MatchesOtherOriginal` means it emitted a *different* person's value, which is the more serious event and deserves its own count.
- Give `invalid_replacement_reason` access to the batch's originals so it can check all of them. Passing `expected_by_key` (or its key set) is the smaller change; `validated_replacements` already owns it, and no caller outside that function needs the wider signature.
- Keep the existing self-comparison checks in place and ordered first, so a replacement equal to its own original still reports `SameAsOriginal` rather than the new reason.
- Apply the same `original_key.len() >= 3` guard the containment check already uses, so short generic values such as `id` or `NL` do not reject every plausible replacement.
- No new fallback path is needed. A rejected replacement already falls through to `record_smart_fallback` and the pseudonymizing transformers, and the new reason inherits that unchanged.

Both entry points are covered by fixing this one function: the model path through `build_replacement_map`, and `SmartReplacementMap::from_entries`, which runs the same validation over `preview_smart_replacements` arriving from IPC.

Validation:

- `cargo test -p csv-anonymizer-core smart`
- `npm run contracts:check` — the enum is contract-checked, so a missed frontend variant fails here rather than silently
- `npm run frontend:typecheck`
- `cargo clippy --workspace --all-targets -- -D warnings` — the label match in `release_report.rs` is exhaustive, so a missing arm fails the build

Exit criteria:

- A regression test asserts the exact case above: a batch of two originals where the model returns the second original as the first's replacement yields zero accepted values for that pair and one `MatchesOtherOriginal` rejection.
- A companion test proves the same input arriving as `preview_smart_replacements` through `from_entries` is rejected identically.
- A test proves a legitimate replacement that merely shares a short substring with another original is still accepted, pinning the guard against over-rejection.
- Existing Smart replacement tests pass unchanged, confirming no accepted-value behavior regressed.

## Phase 2: Disclose Detection Sample Coverage

Status: completed on 2026-07-30, wider than planned — see Correction below.

Goal: when detection classified a fraction of the file, say so — before the run as a preflight review item, and after it as a privacy report note. Detection behavior is unchanged.

Targets:

- `crates/csv-anonymizer-core/src/service.rs:113-117` (`preflight_anonymization`), which already computes the figure and discards it.
- `crates/csv-anonymizer-core/src/service/preflight.rs:21-63` (`run_preflight`, `PreflightState`), alongside the existing `add_mapping_memory_review`.
- `crates/csv-anonymizer-core/src/report_notes.rs`, next to `push_unselected_column_note`, which is the closest existing note in intent.
- `crates/csv-anonymizer-core/src/service/privacy_report.rs:10-48` (`build_privacy_report`) and `crates/csv-anonymizer-core/src/release_report.rs` (`standard_notes`).
- `crates/csv-anonymizer-core/src/service/tests/preflight.rs`, `crates/csv-anonymizer-core/src/service/tests/anonymize.rs`.

Recommended shape:

- **Do not add a file pass.** `analyze_csv_with_sample_rows` already returns `HeadersData.row_count`, set from `ParsedSample.data_rows_scanned` at `service.rs:95-96`. Because detection uses `SampleWindow::Spread`, which reads every data row and keeps `row_count` of them, that figure is the file's *exact* total row count, already paid for. `preflight_anonymization` has it in hand at `service.rs:115` and passes only `headers.columns` to `run_preflight`; thread the row count through as well. This mirrors `preview_anonymization_with_smart_provider`, which already threads `detection_sample.data_rows_scanned` for the cardinality warning and documents why at `service.rs:146-149`.
- Compare that total against `detection_sample_rows(input.sample_row_count)` from `service.rs:55-57` — the rows actually kept for classification, and the floor every entry point shares. Emit the review item only when the total exceeds it; a fully sampled file has nothing to disclose and must stay silent.
- Make it a **review item, never a blocker**. Sampling is a legitimate design choice and every existing large-file workflow relies on it; blocking would break them. This matches how `add_mapping_memory_review` treats a projected-memory concern.
- Word it as a bound on evidence, not a defect, and name the remedy the way the memory review item does. The substance to convey: detection examined N of M rows, values occurring in few rows may not have been detected or auto-selected, and raising "Sample rows" increases coverage. Follow the house habit of stating the figures rather than a vague qualifier.
- For the post-run note, `build_privacy_report(columns, transform_report)` currently has no row or sample counts, so this is a signature change with call sites in the CSV and direct-input paths. Pasted and quick-generate workflows classify their whole input, so they must pass a value that keeps the note silent rather than a fabricated one — prefer an explicit "fully sampled" representation over `0`, which reads as missing data.

Validation:

- `cargo test -p csv-anonymizer-core preflight`
- `cargo test -p csv-anonymizer-core service`
- `cargo test -p csv-anonymizer-core direct_input` — proves paste and quick workflows stayed silent
- `npm run contracts:check`
- `npm run docs:rustdoc`

Exit criteria:

- A preflight test over a file whose row count exceeds the detection sample asserts the review item is present, and that `readiness.status` is `Review` rather than blocked.
- A preflight test over a fully sampled file asserts the item is absent.
- A privacy report test asserts the note appears with the right figures after a sampled run, and is absent after a fully sampled one.
- No detection test, locale matrix, multilingual matrix, or held-out corpus expectation changes. Any diff in those files means detection behavior moved and the change has exceeded this phase.
- The 5,000-row reproduction from Current Status still classifies `notes` as `Enum`/Low — this phase discloses that outcome, it does not fix it — and now carries the review item saying so.

## Phase 3: Make Job Status Polling Survive Transient Failures

Status: completed on 2026-07-30.

Goal: a brief loss of contact with a running job must not cancel it.

Targets:

- `frontend/src/hooks/useAnonymizeJob.ts:86-125` (`pollJob`, `consecutivePollFailuresRef`).
- `frontend/src/hooks/useAnonymizerWorkflow.test.tsx`.

Recommended shape:

- Remove the `cancelAnonymizeJob` call from the failure path. Two failed polls 300 ms apart is roughly 600 ms of trouble, and it currently discards a run that may have been streaming for an hour — the exact long run the mapping-budget work at `crates/csv-anonymizer-core/src/strategies/state.rs:96-125` exists to support.
- Keep polling with backoff instead of a fixed 300 ms retry, so a backend under load is not hammered while it recovers.
- Surface the trouble without ending the run: report lost contact through the existing `setError` channel while leaving `busy` as `running` and `activeJobId` set, so the poll loop stays alive and the user keeps the working Cancel button they already have.
- Cancelling stays a user action. The job is already cancel-safe and leaves no partial output — `replace_file_atomically` in `crates/csv-anonymizer-core/src/file_ops.rs:8-45` discards the temporary file on any `Err` — so there is no correctness reason for the client to cancel on the user's behalf.
- Leave `handleJobStatus` alone. It is correct, and the terminal-state handling is not what this phase is about.

Validation:

- `npm run frontend:test`
- `npm run frontend:lint`
- `npm run frontend:typecheck`
- `npm run frontend:e2e`

Exit criteria:

- A test proves that two or more consecutive `getAnonymizeJobStatus` rejections followed by a success let the job reach `succeeded`, with `cancelAnonymizeJob` never called.
- A test proves the user-initiated cancel path still works during a polling failure.
- Backend job retention is untouched: `MAX_RETAINED_TERMINAL_JOBS` and `TERMINAL_JOB_TTL` in `src-tauri/src/jobs.rs:13-14` already keep a terminal job readable across dropped polls, and this phase must not need them changed.

## Phase 4: Batched Cleanups

Status: completed on 2026-07-30.

Goal: four small, independent corrections. Behavior-preserving except where noted for the temporary-file permissions.

Targets and recommended shape:

- **Dead parameter.** `src-tauri/src/jobs.rs:98-102` — `create_job_for_output` takes `_output_path: PathBuf` and ignores it, because admission collapsed from a per-output-path lease to a single global boolean. Drop the parameter and update the call site in `src-tauri/src/commands/job_commands.rs:48`. Rename or re-comment the tests at `jobs.rs:322-341` (`rejects_different_output_while_anonymization_is_active`), which currently imply per-path admission that no longer exists — the behavior they assert is correct and worth keeping, only the name misleads.
- **Double sanitization.** `src-tauri/src/settings/store.rs:48` and `:94` both call `sanitize_settings` on the save path. Keep the one in `save_settings_to_path`, since it is the lower boundary every writer passes through, including `load_settings_from_path`'s migration rewrite at `:82`. Removing the outer call means `save_settings` must still return the sanitized value it promises its caller, so have it read back the sanitized result rather than returning its own pre-sanitization clone.
- **No-overwrite portability.** `crates/csv-anonymizer-core/src/file_ops.rs:19-25` publishes via `fs::hard_link` to get atomic no-clobber semantics. The primitive is right; the failure mode is not. On a filesystem without hard-link support the user sees a generic I/O error. Map that failure to a message naming the cause and the remedy — enable overwrite for this destination — keeping `AnonymizerError::OutputExists` for the genuine already-exists case.
- **Temporary file permissions.** `crates/csv-anonymizer-core/src/file_ops.rs:47-69` creates the temporary output with default umask permissions while it holds transformed data. Set `0600` at creation on Unix via `OpenOptionsExt::mode`, before any row is written. This is a deliberate behavior change and the one item here that is not purely cosmetic. The published file keeps its current permissions: `fs::rename` preserves the source file's mode, so tightening the temporary file would silently tighten the user's output too. Decide explicitly whether the published output should inherit `0600` or be relaxed after the rename, and record the choice where the code makes it.

Validation:

- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `npm run deadcode:required`
- `npm run smoke:rust`

Exit criteria:

- A unix-gated test asserts the temporary output file's mode is `0600` while the write is in flight, and a test pins whatever was decided for the published file's mode.
- Settings save and load round-trip tests pass unchanged, including `concurrent_settings_saves_leave_valid_json` at `src-tauri/src/settings/store.rs:252-272`.
- `cargo-machete` and `knip` stay clean.
- No behavior change is observable in the job store beyond the narrowed signature.

## Root Gate

Run before proposing any phase as complete. This is the same sequence `.github/actions/validate-build/action.yml` runs, in the order that fails cheapest first:

```bash
npm run docs:check
npm run contracts:check
npm run frontend:lint
npm run frontend:typecheck
npm run frontend:test
npm run deadcode:required
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run docs:rustdoc
npm run smoke:rust
```

`npm run frontend:e2e` and `npm run frontend:a11y` are required for Phase 3 and optional elsewhere.

## Verification Notes

The two reproductions in Current Status were run as throwaway harnesses and deliberately not committed — the Phase 1 and Phase 2 exit criteria call for permanent tests in the crate's own suites instead. To recreate them:

- Finding 1: call `validated_replacements` from a `#[cfg(test)]` module inside `crates/csv-anonymizer-core/src/smart.rs` with the two-value batch shown above and print `accepted` and `rejection_reasons`.
- Finding 2: generate a 5,000-row CSV whose second column is constant except for one row holding an email address, then compare `cargo run -p csv-anonymizer-app -- analyze <file>` against the same shape truncated to 100 rows. The verdict moves from Low to High on identical data.

## Correction To Phase 2 As Planned

The plan asserted that "pasted and quick-generate workflows classify their whole input, so they must pass a value that keeps the note silent". That was wrong for every paste format and right only for quick generation.

Pasted CSV classifies through `read_csv_detection_sample_from_str` at the same floored row count as the file workflow, and the field-based formats (plain text, logs, JSON, YAML, XML) bound each field's sample through `FieldSampleLimits::detection_only`. A 5 MiB paste can hold far more rows than the sample keeps, so those workflows have exactly the gap the finding described and were given the same note rather than a silent one. Only `direct_input::quick` passes `DetectionCoverage::complete()`, and it does so truthfully: it generates values from nothing and has no source input to sample.

This widened the phase. `analyze_csv_text`, `analyze_xml` and `analyze_value_document` each gained a `_with_coverage` sibling so the transform paths can report coverage while the analyze command keeps returning the DTO alone, and `shared::detection_coverage` aggregates the field-based formats the same way `fields_to_rows` already lays them out.

## Decision Recorded For Phase 4 Permissions

The plan left open whether the published output should inherit the temporary file's owner-only mode. Both publish paths carry the temporary file's permissions to the destination — `rename` moves the inode and `hard_link` gives it a second name — so the choice could not be avoided.

Resolved by splitting the two cases in `adopt_destination_permissions`:

- A destination being **created** keeps `0600`. Output derived from data the user brought here to protect should not be world-readable by default, and tightening the temporary file alone would have bought almost nothing while the published file sat at `0644` in the same directory.
- An **existing** destination keeps exactly the permissions it already had. Silently changing who can read a file the user had already placed and possibly shared is a surprise worth more than the hardening.

## Completion Evidence

Root gate, all passing on 2026-07-30 after Phase 4:

- `npm run docs:check`, `npm run contracts:check`
- `npm run frontend:lint`, `npm run frontend:typecheck`, `npm run frontend:test` (100 tests)
- `npm run deadcode:required`
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace` (455 tests)
- `npm run docs:rustdoc`, `npm run smoke:rust`
- `npm run frontend:e2e` (4 tests), `npm run frontend:a11y` (1 test)

Negative gate for Phase 2 held: no file under `crates/csv-anonymizer-core/src/detection/` and no fixture under `tests/` was modified, and the 5,000-row reproduction still classifies `notes` as `Enum`/Low. Detection behaviour did not move; only what the app says about it did.
