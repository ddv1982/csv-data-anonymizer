# Full Codebase Review And Improvement Plan - 2026-07-01

## Status Notice - 2026-07-13

This review and phased plan remain historical evidence. The findings and source locations below record what the 2026-07-01 review observed; their dated phase-status paragraphs record the follow-up known when those paragraphs were added. They should not be rewritten into a retrospective current-state review.

Six subsequent modernization features strengthened privacy-evidence retention, frontend async behavior, native writer/settings lifecycles, Rust and React module ownership, and locally reproducible quality gates. The dated current-state map is [`modernization-status-2026-07-13.md`](modernization-status-2026-07-13.md). It also records intentional deferrals and platform validation limits rather than silently erasing them from this plan.

The status document records the final broad project-convergence gate and passing detailed final review. Earlier targeted evidence remains historical context, not a replacement for that final review.

## Scope

Independent full review of the Rust core (`crates/`), Tauri shell (`src-tauri/`), React frontend (`frontend/`), and release/CI tooling (`scripts/`, `.github/workflows/`). This review deliberately excludes findings already completed or explicitly deferred in `docs/code-quality-assessment-2026-07-01.md` and `docs/cleanup-phased-plan-2026-07-01.md`; it hunts for what those passes missed.

External calibration used: Tauri 2 security guidance (least-privilege capabilities, CSP, minimal IPC surface, https://v2.tauri.app/security/) and EDPB Guidelines 01/2025 on pseudonymisation plus ICO anonymisation guidance (consistent pseudonyms increase linkability risk; transformations should be protected against silent reversal or bypass).

## Honest Overall Assessment

The baseline is genuinely high: layered detection with explanation traces, collision-aware consistency mapping, atomic output writes, loopback-only Local AI, no value logging, tight CSP, broad CI gates, and the 2026-07-01 cleanup verifiably landed. This is well above average for a solo desktop app.

The gap that matters is thematic, not scattered: **the privacy report can overstate what the output actually anonymized.** Type-specific transforms are not total functions (shape-outlier values pass through as original PII), preview and final-run detection use different sample sizes, and detection reads trimmed values while processing reads untrimmed ones. For a privacy tool, report integrity is the product; Phase 1 exists to close exactly that gap. Secondary themes: two data-safety holes in the shell (input==output overwrite, silent output-path auto-grant), frontend cross-workflow parity drift, and release-pipeline supply-chain exposure.

## Findings

### Rust core (`crates/csv-anonymizer-core`, `csv-anonymizer-app`)

- **CORE-1 (high)** Shape-outlier values in pseudonymized columns silently pass through unchanged. `transform_email` returns the original when the value has no `@` (`strategies/structured.rs:31-33`), `transform_timestamp_candidate` returns the original when the first 10 bytes are not `%Y-%m-%d` (`strategies/structured.rs:72-79`), `transform_phone_candidate` preserves all non-digit text (`strategies/structured.rs:116-127`). Detection accepts a type at ~50% sample match on the first 100 rows, so real columns hit this. No report counter or note records the fallback; `build_privacy_report` still counts the column as pseudonymized (`service.rs:659-665`). Verified by test during review.
- **CORE-2 (medium)** Byte-slice panic on multibyte timestamp values: `value.len() < 10` is a byte check followed by `&value[..10]` (`strategies/structured.rs:73-77`); one CJK/accented date cell aborts the whole job. Verified by test.
- **CORE-3 (medium)** Preview re-detects on `sample_count * 2` rows (`service.rs:142`) while analyze/anonymize use 100 rows (`service.rs:225-226`), and nothing pins the detected type between calls (frontend prunes default controls; CLI sends none). Enum needs >10 samples (`detection/value.rs:146-155`) and is pass-through under Auto, so the preview can show scrambled values while the written file keeps originals.
- **CORE-4 (medium)** Sampling readers trim (`csv_io.rs:25-29,36-40`) but processing readers do not (`csv_io.rs:151-154,224-228`): `" 2024-06-15"` becomes malformed output, `" null "` is transformed though detection treated it as empty, and padded duplicates get distinct pseudonyms (keys include raw `value.len()`). Verified by test.
- **CORE-5 (medium, DRY)** The pass-through-under-Auto type list exists twice: transform dispatch (`strategies/mod.rs:68-69`) and `DataType::uses_default_pass_through` (`types.rs:104-113`, consumed by report/preview/readiness). Divergence produces report-integrity bugs.
- **CORE-6 (low)** Dead/speculative public API: `transform_quick_values`, `quick_anonymize_values`, `preview_rows`, `anonymize_rows(_with_smart_provider)`, `missing_smart_replacement_values_from_rows` have no production callers.
- **CORE-7 (low)** `PrivacyReport.sensitive_columns` is always 0 (`service.rs:621`) yet lives in the TS contract and the summary component.
- **CORE-8 (low)** `direct_input/text.rs:179-203` re-implements `select_non_overlapping_spans` on already non-overlapping input; only the cap is live.
- **CORE-9 (low)** Dead `Blocked` branch in `release_report.rs:19,53-54` (empty `blockers` never pushed).
- **CORE-10 (low, product decision)** Formula neutralization prefixes every `-`/`+` cell in output, including unselected numeric columns, and the preview never shows it (`csv_io.rs:351-384`). Deliberate guard, but negative numbers in pass-through data are altered invisibly.
- **CORE-11 (low, product decision)** Email pseudonymization keeps the full original domain (`structured.rs:34-41`, pinned by test); identifying for personal domains and undisclosed in report notes.

### Tauri shell (`src-tauri/`)

- **SHELL-1 (medium)** No input==output guard anywhere (shell or `service.rs:482-509`); with overwrite enabled, choosing the input file as output atomically replaces and destroys the original.
- **SHELL-2 (medium)** `analyze_csv` silently auto-grants write access for the suggested output path (`commands/csv.rs:108-109`), bypassing the explicit-confirmation model every other grant uses, and the `?` hard-fails analysis when the suggested path is ungrantable (e.g. a directory with that name).
- **SHELL-3 (medium)** `start_local_ai_model_download` is a sync command doing a blocking reqwest call with a 120s timeout on the main thread (`commands/local_ai_commands.rs:15-19`, `local_ai/mod.rs:28-34`); UI can freeze for up to 2 minutes. `get_local_ai_status` already wraps the same call in `run_blocking` — an inconsistency, not a style choice. Related: `open_local_ai_setup_url` uses blocking `open::that` where `files.rs:73` uses `open::that_detached`.
- **SHELL-4 (medium/low)** Cancellation cannot interrupt in-flight Ollama requests (`local_ai/provider.rs:62-79`); cancel of a model download drops the stream but Ollama keeps pulling server-side (`download.rs:188-190`).
- **SHELL-5 (low)** Unused `dialog:allow-open`/`dialog:allow-save` permissions in `capabilities/default.json:30-31` (all dialogs open from Rust); CSP `img-src` still allows `asset:` though `assetProtocol` is disabled.
- **SHELL-6 (low)** One-shot `snapshot_job` deletes terminal jobs on first read (`job_registry.rs:86-89`), making polling non-idempotent; a discarded terminal response shows an error for a job that succeeded.
- **SHELL-7 (low, DRY)** Small duplications: three identical sanitize wrappers (`settings/model.rs:55-65`), duplicate `service()` constructors, duplicated control-character check plus error string in `commands/shared.rs`, terminal-state lists re-enumerated in both cancel commands, diverging "Local AI required" predicates between preflight and provider construction.

### Frontend (`frontend/`)

- **FE-1 (high)** Tab switch unmounts paste/quick workflows (`App.tsx:51,63,81` conditional render inside sections that already set `hidden`), silently destroying pasted content, analysis, selection, and preview. The redundant `hidden` attribute shows persistent panels were the intent. No test covers persistence.
- **FE-2 (medium)** CSV "Select Detected Risk" is gated on `selectableColumns.length` instead of `detectedRiskColumns.length` (`workflow/AnonymizerWorkflowView.tsx:148-152` vs paste's correct gating), so with zero risk columns the enabled button wipes a manual selection and preview.
- **FE-3 (medium)** Auto-selection policy is still duplicated and divergent in paste: `PasteDataWorkflowView.tsx:84-88` re-implements it risk-only (ignores the non-empty-samples condition the core policy requires). The backend should return selected columns for paste as `analyze_csv` does.
- **FE-4 (medium)** Local AI download poll failure wedges the UI permanently: `useLocalAi.ts:104-108` clears `downloadJobId` but not `downloadStatus`, so `downloadRunning` stays true, Smart flows stay blocked, and Cancel no-ops. Only restart recovers.
- **FE-5 (medium)** Failure paths have zero test coverage: job `state: 'failed'`, non-empty preflight blockers, poll error paths, entire model download start/poll/cancel flow.
- **FE-6 (low/medium)** `unselectedRiskColumns` derivation duplicated within `AnonymizerWorkflowView.tsx:117-124,266-273`, both re-implementing the hook's `detectedRiskColumns` predicate; the `selectableColumns` alias is a vestige of the removed abstraction.
- **FE-7 (low/medium)** Anonymize poll failure clears `activeJobId` while the Rust job keeps running (`useAnonymizeJob.ts:92-98`); it can no longer be canceled and a second job can target the same output.
- **FE-8 (low)** Dead plumbing: `updateColumnType` threads through three layers with no caller; `strategyControlsDisabled(Reason)` props are never passed by any call site. Same class of speculative branch as the removed `isSelectableColumn`.
- **FE-9 (low)** `'gemma3:4b'` hardcoded in 5+ places; `LocalAiPanel` re-derives hook state; `useLocalAi` duplicates its own `refresh`.
- **FE-10 (low)** Polish: raw enum tokens in "Detected: plainText"; stale "Copied" pill next to a copy error; misleading `!canAnonymize` fallback message; paste transform runs no preflight while CSV does (worth a deliberate decision).

### Scripts and CI (`scripts/`, `.github/workflows/`)

- **CI-1 (high)** Release-signing secrets (Apple cert/notarization keys, APT GPG key) are job-level env visible to third-party actions pinned by mutable refs (`release.yml:153-160,326-330`; `Swatinem/rust-cache@v2`, `dtolnay/rust-toolchain@stable`, `softprops/action-gh-release@v3`, `dorny/paths-filter@v3`). A hijacked tag exfiltrates the signing identity.
- **CI-2 (high)** `publish-apt-repository` needs only `build-linux-release` (`release.yml:517-531`), so a macOS notarization flake publishes the new `.deb` to every APT user while the GitHub release stays an unpublished draft; no rollback.
- **CI-3 (medium)** `check-contracts.mjs` camelCases Rust names without verifying `#[serde(rename_all = "camelCase")]` is present; a new DTO without the attribute passes the check while serializing snake_case. Field regex also misses `r#`-prefixed identifiers.
- **CI-4 (medium)** Root `knip.json` matches no CI path filter (`ci.yml:41-75`), so edits to a gate config merge with zero CI.
- **CI-5 (medium)** `validate-release` hand-duplicates the CI rust job and has already drifted (omits prebuilt-frontend check and rust-smoke that CI runs).
- **CI-6 (medium)** Full local gate rebuilds the frontend three times: root `lint`, `test`, and `typecheck` each run `frontend:build` (tsc x2 + full Vite production build) because `tauri_build` needs `frontend/dist` to exist; root `typecheck` never uses `frontend:typecheck`.
- **CI-7 (medium)** macOS DMG naming/arch mapping triplicated (`release.yml:290-296`, `package-rust-macos.mjs:41,148`, `command-utils.mjs:92-108`); ~120 lines of Debian/AppStream parsing duplicated between `build_apt_repository.py` and `validate_linux_package_metadata.py`; ~120 lines of legacy native macOS packaging dead behind an env flag in `package-rust-macos.mjs`.
- **CI-8 (low)** Assorted: `cancel-in-progress` applies to `main` pushes; re-running a published release flips it back to draft before building; parallel jobs racing `action-gh-release` draft lookup; `copyTree` drops symlinks when staging the .app; notarization retries deterministic rejections; byte-identical alias scripts (`dist:linux` = `dist:linux:ci` = `linux:packages`, `smoke:packaged` = `smoke:rust`).

## Phased Improvement Plan

Ordering principle: report integrity and data safety first (the product promise), then user-visible correctness, then supply chain, then simplicity. Each phase is independently shippable; do not mix phases with release version churn in one PR.

### Phase 1: Make transforms total and reports truthful (Rust core)

Status: completed on 2026-07-01. Typed transforms fall back to generic pseudonyms with a `shapeFallbackValues` counter in the report, readiness review items, and notes; the multibyte timestamp panic is fixed; preview and anonymize share the DEFAULT_SAMPLE_ROWS detection basis (smart replacement and preview samples stay on the display window); cell values are trimmed consistently; transform dispatch consults `uses_default_pass_through()`.

Targets: CORE-1, CORE-2, CORE-3, CORE-4, CORE-5.

- Make Email/Timestamp/Phone transforms total: on shape mismatch fall back to `transform_generic_string` (or typed placeholder), never the original value. Add a per-column fallback counter surfaced in the privacy report and preview warnings.
- Replace the byte slice with `value.get(..10)`/`is_char_boundary` handling; add multibyte fixtures (CJK dates, accented text) to the strategy tests.
- Unify detection sample size between preview and anonymize, or pin detected types from analyze through the run (shell passes explicit type overrides for all columns).
- Trim cell values consistently between sampling and processing (cell-level trim before transform, preserving row fidelity), with padded-duplicate consistency tests.
- Collapse the pass-through type list: transform dispatch calls `uses_default_pass_through()`.

Validation: `cargo test --workspace`, new fixtures for each fixed path, `npm run test`, `npm run contracts:check`.

Exit criteria: no code path writes an original value for a selected column without a report-visible counter; preview, report, and final output agree on a shared detection basis.

### Phase 2: Data-safety and shell consistency (Tauri)

Status: completed on 2026-07-01. Output==input is a preflight blocker and anonymize error; analyze_csv no longer grants write access silently; the download command runs its probe off the main thread; unused dialog permissions and the stale asset: CSP source are gone; plain signed numbers are exempt from formula neutralization; email-domain preservation is disclosed in report notes.

Targets: SHELL-1, SHELL-2, SHELL-3, SHELL-5, plus CORE-10/CORE-11 as product decisions.

- Reject `output_path == input_path` (canonicalized) as a preflight blocker and at job start.
- Remove the silent `grant_output_file` from `analyze_csv`; make the suggested path a plain suggestion granted through the existing confirm/authorize flow, and stop failing analysis when the suggestion is ungrantable.
- Make `start_local_ai_model_download` async/`run_blocking` like its sibling commands; use `open::that_detached` in `open_local_ai_setup_url`.
- Drop unused `dialog:allow-open`/`dialog:allow-save` permissions and the stale `asset:` CSP source.
- Decide and document: exempt strict-numeric negatives from formula neutralization (and/or show neutralization in preview); disclose email-domain preservation in report notes.

Validation: `cargo test --workspace`, manual overwrite/self-target attempt, `npm run frontend:e2e`.

Exit criteria: no write grant exists that the user did not explicitly confirm; the original input file cannot be destroyed by the app.

### Phase 3: Frontend parity and failure-path robustness

Status: completed on 2026-07-01. Panels stay mounted across tabs; Select Detected Risk gates on detected-risk columns; paste auto-selection is backend-driven via should_auto_select_column; download and job polling recover from failures; failure paths are tested.

Targets: FE-1, FE-2, FE-3, FE-4, FE-5, FE-7.

- Keep workflow panels mounted (rely on the existing `hidden` attribute); add a test asserting paste state survives tab switches.
- Fix "Select Detected Risk" gating to `detectedRiskColumns.length === 0` in the CSV workflow (matching paste).
- Move paste auto-selection to the backend (return selected columns from the paste analyze command using the core `should_auto_select_column` policy); delete the TS re-implementation.
- On download poll failure, clear/patch `downloadStatus` so `downloadRunning` releases; on anonymize poll failure, retry once and keep `jobId` available for cancel.
- Add failure-path tests: job failed state, non-empty preflight blockers, poll errors, download start/poll/cancel.

Validation: `npm run frontend:test`, `npm run frontend:typecheck`, `npm run frontend:e2e`, `npm run frontend:a11y`.

Exit criteria: CSV and paste workflows share one selection policy sourced from the core; every polling loop has a tested recovery path; no tab switch loses user input.

### Phase 4: Release pipeline hardening

Status: completed on 2026-07-01. Actions SHA-pinned, secrets step-scoped, APT publish gated on all builds, shared validate steps in a composite action, contract checker asserts serde camelCase, knip.json triggers CI, cancel-in-progress skips main, published releases cannot be flipped back to draft.

Targets: CI-1, CI-2, CI-3, CI-4, CI-5, CI-8 (selected).

- Pin all third-party actions by commit SHA; move signing secrets from job-level to step-level env on only the steps that need them.
- Gate `publish-apt-repository` on all build jobs (or run it inside/after `publish-release`).
- Teach `check-contracts.mjs` to require `#[serde(rename_all = "camelCase")]` on matched types and to parse `r#` identifiers.
- Add root `knip.json` to the CI path filters.
- Extract the shared validate steps into a reusable workflow/composite action so `validate-release` cannot drift from CI.
- Scope `cancel-in-progress` to non-main refs; guard the re-run-flips-published-release-to-draft path.

Validation: `npm run release:check`, `npm run linux:release-helpers:check`, a dry-run tag on a fork or workflow-dispatch test, `actionlint` if available.

Exit criteria: a compromised action tag cannot see signing material; no publish channel can outrun a failed release.

### Phase 5: Simplicity and DRY sweep

Status: completed on 2026-07-02. Shell DRY helpers consolidated, terminal-job polling made idempotent, dead core branches and paste-text re-filtering removed, local gates build the frontend once, Debian/AppStream parsing shared in deb_common.py, legacy macOS packaging deleted, DMG naming single-sourced, aliases pruned, notarization fails fast on Invalid, DMG staging preserves symlinks, dead core APIs and the sensitive_columns metric removed, frontend dead plumbing deleted, default model constant centralized.

Targets: CORE-6..CORE-9, SHELL-4 (document), SHELL-6, SHELL-7, FE-6, FE-8, FE-9, FE-10, CI-6, CI-7, CI-8 (rest).

- Delete dead core APIs, the always-zero `sensitive_columns` metric (Rust + TS contract + summary component), the redundant span re-filter, and the dead `Blocked` branch.
- Delete dead frontend plumbing (`updateColumnType` chain, `strategyControlsDisabled` props, `selectableColumns` alias); centralize the default-model constant; dedupe `unselectedRiskColumns` and `useLocalAi.refresh`.
- Shell DRY: single sanitize entrypoint, shared `service()` constructor, shared suffix-validation helper, shared `is_terminal()`; document the accepted Ollama cancellation latency and remaining TOCTOU windows.
- Script graph: root `typecheck` becomes `frontend:typecheck` + `cargo check` with a shared ensure-`frontend/dist` helper so the full local gate builds the frontend once; extract `deb_common.py` for the duplicated Debian/AppStream parsing; delete legacy native macOS packaging; single source for DMG naming; prune byte-identical aliases or document their purpose.

Validation: `npm run deadcode:required`, `npm run test`, `npm run lint`, `npm run docs:check`, `npm run contracts:check`.

Exit criteria: knip/machete stay green after deletions; the full local gate runs one frontend build; no policy or naming rule exists in more than one place.

## Not Recommended

- Do not weaken formula neutralization, path-access, or release-signing guards while simplifying around them.
- Do not build a generic workflow framework to fix FE parity; fix the two concrete divergences.
- Do not adopt a schema-generation layer for contracts yet; the serde-attribute assertion closes the realistic hole.
- Do not reorganize `types.rs` or the APT channel wholesale; extract only the shared parsing helpers.

## Suggested Order

1. Phase 1 (report integrity is the product promise).
2. Phase 2 and Phase 3 in either order or in parallel; both are small, user-visible correctness work.
3. Phase 4 before the next release tag.
4. Phase 5 opportunistically alongside the phases that touch the same files.
