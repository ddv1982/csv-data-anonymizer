# Codebase Review - 2026-06-29

## Scope

Reviewed the CSV Anonymizer repository as a local-first Tauri desktop app with a React/Vite frontend, Rust anonymization core, Tauri command shell, release/package scripts, and CI metadata.

Validation run during the review:

- `npm run test` passed: contract check, 31 frontend tests, frontend production build, 4 CLI tests, 137 core tests, 36 Tauri tests, and core doc-tests.

Deployment context used for severity: single-user desktop app and local CLI smoke harness, not a shared multi-tenant service. Optional Local AI traffic is intended for local Ollama on loopback.

## Findings

### 1. Direct-input previews miss safety warnings shown in CSV-file previews

Severity: medium

Evidence: `crates/csv-anonymizer-core/src/direct_input/shared.rs:110` returns `PreviewData` with `warnings: Vec::new()`, while CSV-file previews build warnings from selected columns in `crates/csv-anonymizer-core/src/service.rs:128` and `crates/csv-anonymizer-core/src/service.rs:346`. Those warnings cover pass-through, Local AI, and currently no-op auto/pseudonymize types in `crates/csv-anonymizer-core/src/service.rs:347`.

Guards checked: direct-input transform reports still produce privacy notes later, and file previews have tests for pass-through/no-op warnings. The pre-write direct-input preview remains silent, so paste/quick workflows can show less warning context before output is generated.

Why it matters: paste workflows support CSV text, JSON, YAML, XML, plain text, and logs up to 5 MiB. Users can review a preview before writing/copying output, but direct-input previews do not surface the same unchanged-value warnings as the file workflow.

Fix shape: share the file-preview warning builder with direct-input preview generation and add tests covering pass-through, Local AI, and no-op type warnings for pasted data.

### 2. Formal tabular releases can still include unselected source columns unchanged

Severity: medium, product-policy risk

Evidence: unselected columns are coerced to `Attribute` in `crates/csv-anonymizer-core/src/privacy/mod.rs:62`, formal writes preserve unmatched roles through `_ => value` in `crates/csv-anonymizer-core/src/privacy/formal.rs:251`, and an existing test asserts unselected email values remain unchanged in `crates/csv-anonymizer-core/src/service/tests/privacy_releases/formal_privacy.rs:84` and `crates/csv-anonymizer-core/src/service/tests/privacy_releases/formal_privacy.rs:134`. Report notes do warn about unchanged high/medium-risk unselected columns in `crates/csv-anonymizer-core/src/report_notes.rs:15`.

Guards checked: explicit privacy roles that reference unselected columns are rejected in `crates/csv-anonymizer-core/src/privacy/mod.rs:75`, and synthetic releases reject unselected columns in `crates/csv-anonymizer-core/src/privacy/mod.rs:98`. Those guards do not prevent formal output from carrying unselected source columns unchanged.

Why it matters: the current behavior is intentional enough to be tested, but it may surprise users who read "formal tabular release" as applying to the whole output table. The risk is especially high when an unselected column is detector-risky but not part of the formal role plan.

Fix shape: decide the product contract explicitly: require all columns selected for formal releases, drop unselected columns, or require a stronger "release unchanged columns" acknowledgement before writing.

### 3. Deterministic CLI anonymization allows an empty seed

Severity: medium

Evidence: the CLI initializes `seed` to an empty string in `crates/csv-anonymizer-app/src/cli.rs:71`, `--deterministic` does not require `--seed` in `crates/csv-anonymizer-app/src/cli.rs:95`, and the seed is passed into `AnonymizeParams` in `crates/csv-anonymizer-app/src/cli.rs:217`. Deterministic hashing pads the seed bytes to the HMAC key in `crates/csv-anonymizer-core/src/hash.rs:31`, so an empty seed becomes a common all-zero padded key.

Guards checked: deterministic mode defaults to false, and the core privacy report warns that configured seeds are sensitive. No CLI/core validation rejects deterministic mode with an empty seed.

Why it matters: deterministic mode is meant for repeatable private pseudonyms across files. An accidental empty seed makes outputs reproducible by anyone using the same empty-seed path.

Fix shape: reject `deterministic=true` with a blank seed at the CLI and core boundary, and add a CLI test for the error.

### 4. Local AI model-download cancellation can wait on a stalled stream read

Severity: medium

Evidence: cancellation only sets a job flag through `cancel_local_ai_model_download` in `src-tauri/src/commands/local_ai_commands.rs:37`, the download loop checks that flag before each streamed line in `src-tauri/src/local_ai/download.rs:179`, and the download client has a connect timeout but no read/overall timeout in `src-tauri/src/local_ai/mod.rs:34`.

Guards checked: normal success/error paths finish the job in `src-tauri/src/local_ai/download.rs:153`, and the endpoint is fixed to local Ollama in `src-tauri/src/local_ai/mod.rs:15`. A stalled response body can still delay observing cancellation.

Why it matters: a user-visible cancel action should not depend on Ollama producing another streamed line. In a local desktop app this is not a service availability issue, but it can leave the UI feeling stuck during model pulls.

Fix shape: add a read or total request timeout, or move download cancellation to an abortable request mechanism that can interrupt the blocking read.

### 5. Background jobs can remain running forever if a worker panics

Severity: medium

Evidence: anonymize and model-download workers discard their `spawn_blocking` handles in `src-tauri/src/commands/job_commands.rs:47` and `src-tauri/src/commands/local_ai_commands.rs:22`. Terminal statuses are set only by the normal worker paths in `src-tauri/src/jobs.rs:176` and `src-tauri/src/local_ai/download.rs:153`. The registry prunes terminal jobs but retains running jobs in `src-tauri/src/job_registry.rs:108`.

Guards checked: many ordinary error paths convert to failed job states, and tests cover pruning terminal jobs plus retaining running jobs. There is no supervisor/catch-unwind path that marks a panicked worker as failed.

Why it matters: panics should be rare, but a stuck running job can leave the frontend polling indefinitely and prevent users from understanding the failure.

Fix shape: keep/supervise join handles or wrap worker bodies so panics transition the job to a terminal failed state.

### 6. DP budget reset UI bypasses the human confirmation that the backend API models

Severity: medium

Evidence: the reset button calls `onResetBudget` directly in `frontend/src/components/privacy-settings/DpBudgetSettings.tsx:53`, while the workflow sends the hardcoded confirmation phrase in `frontend/src/hooks/useAnonymizerWorkflow.ts:207` using `DP_BUDGET_RESET_CONFIRMATION_PHRASE` from `frontend/src/tauri.ts:40`. The backend does validate the phrase, but the frontend supplies it automatically.

Guards checked: backend tests cover accepting and rejecting confirmation phrases, and the button is disabled when spent epsilon is zero. There is no user-entered confirmation or modal in the frontend path.

Why it matters: the ledger is backend-owned and preserves DP spend across settings saves, so resetting it is a sensitive privacy-budget action. The current UI makes that reset a single click.

Fix shape: require an explicit modal/typed confirmation in the UI before passing the phrase, and add a frontend test for accidental-click protection.

### 7. Local AI readiness can be stale for a changed model

Severity: medium

Evidence: `useLocalAi` exposes `ready` as `Boolean(settings.localAiEnabled && status?.ready)` in `frontend/src/hooks/useLocalAi.ts:116` without checking `status.model`. The panel itself computes `statusMatchesModel` before showing readiness in `frontend/src/components/LocalAiPanel.tsx:32`, but CSV preview/anonymize gating uses the hook-level `ready` through `frontend/src/hooks/useAnonymizerWorkflow.ts:100`.

Guards checked: the UI panel tries to display "Checking ..." when status and selected model differ, and backend Local AI calls use a fixed loopback endpoint. The workflow gate can still treat the previous model's status as ready during settings changes until refresh settles.

Why it matters: preview/anonymize actions can be enabled against the wrong model readiness state, causing avoidable backend errors or unexpected fallback behavior.

Fix shape: make hook readiness require `status.model === request.model`, and add a test for changing models while the previous status is ready.

### 8. Settings can be saved from defaults before the initial load resolves

Severity: medium

Evidence: persistent settings start from `defaultSettings` in `frontend/src/hooks/usePersistentSettings.ts:13`; the initial `loadSettings()` result is accepted only when `settingsSaveSequenceRef.current === 0` in `frontend/src/hooks/usePersistentSettings.ts:23`. Any setting change calls `persistSettings` with `latestSettingsRef.current` from `frontend/src/hooks/useAnonymizerWorkflow.ts:161`, increments the save sequence in `frontend/src/hooks/usePersistentSettings.ts:58`, and can cause the later loaded settings to be ignored.

Guards checked: stale save responses are reconciled, and authoritative refresh exists. That does not protect the first-load race where a user changes a setting before persisted settings are accepted.

Why it matters: a quick toggle at launch can overwrite previously saved preferences with defaults plus one changed value.

Fix shape: expose a settings-loaded flag and disable/save-gate settings controls until the initial load is accepted, or merge early edits into the loaded settings instead of replacing them.

### 9. Paste and quick workflows do not display privacy reports they receive

Severity: low

Evidence: paste and quick result types include `privacyReport` in `frontend/src/types.ts:165` and `frontend/src/types.ts:173`, but paste output renders only the anonymized text and stats in `frontend/src/components/PasteDataWorkflowView.tsx:366`, and quick output renders only generated values/count in `frontend/src/components/QuickDataTypeWorkflowView.tsx:154`. CSV-file results render privacy metrics, formal model reports, and notes in `frontend/src/components/ResultDisplay.tsx:105`.

Guards checked: the backend result shape already carries the report for these workflows. The gap is presentation, not missing computation.

Why it matters: non-file workflows are a prominent path for copied output, and users get less privacy-review context than in the file workflow.

Fix shape: add a compact shared privacy-report component for paste/quick outputs and cover it with UI tests.

## Positive Findings

- CSV-file processing is streaming and uses atomic replacement paths rather than materializing standard transforms in memory.
- Privacy release modes that do need materialization have explicit row caps and cancellation checks.
- CSV output neutralizes spreadsheet formula prefixes in headers and cells, with tests for ASCII and full-width variants.
- Ragged rows with non-empty fields beyond headers are rejected without committing partial output.
- DP aggregate releases include important safeguards: deterministic DP output is rejected, grouped output requires public group labels/allowed values, and budget tracking can block or warn on over-budget releases.
- Tauri file/path access has explicit in-memory grants, canonicalization, and symlink/leaf checks.
- DP budget ledger state is backend-owned, serialized, and preserved across normal settings saves.
- CI and release workflows are present and cover frontend audit, Rust audit, contracts, dead-code checks, frontend tests/e2e/a11y, build, Rust fmt/test/clippy, packaging, and smoke checks.
- Release metadata scripts validate cross-file versions, Linux desktop/AppStream metadata, icons, changelog tag expectations, and local model/runtime artifact exclusions.

## Refuted Candidate

- A worker reported that `.github/workflows/release.yml` and `.github/workflows/dead-code.yml` were absent. Direct inspection refuted this: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, and `.github/workflows/dead-code.yml` are present and contain the expected gates.

## Recommended Follow-up Order

1. Fix warning and confirmation gaps that can affect user privacy decisions: direct-input preview warnings, DP budget reset confirmation, and paste/quick privacy-report display.
2. Tighten deterministic seed validation for CLI/core to prevent common-key deterministic output.
3. Decide and document the formal tabular release contract for unselected columns, then enforce it in code and tests.
4. Harden job lifecycle behavior: abortable Local AI downloads and terminal failure states for panicked workers.
5. Fix frontend state race conditions around initial settings load and model-specific Local AI readiness.
6. Add regression tests for every accepted finding before or alongside implementation.
