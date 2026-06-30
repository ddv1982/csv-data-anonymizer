# Codebase Review - 2026-06-29

## Scope

Reviewed the current CSV Anonymizer repository as a local-first Tauri desktop app with a React/Vite frontend, Rust anonymization core, Tauri command shell, local settings persistence, release/package scripts, and CI metadata.

Deployment context used for severity: single-user desktop app and local CLI smoke harness, not a shared multi-tenant service. Optional Local AI traffic is intended for local Ollama on loopback.

Validation run during the review:

- `npm run test` passed: contract check for 17 enums and 28 structs, 39 frontend unit tests, frontend production build, 6 CLI tests, 164 core tests, 47 Tauri tests, and core doc-tests.

Implementation validation after the follow-up pass:

- `npm run release:check` passed: release metadata validation and Linux package metadata tests.
- `npm run test` passed: contract check for 17 enums and 28 structs, 41 frontend unit tests, frontend production build, 7 CLI tests, 164 core tests, 50 Tauri tests, and core doc-tests.

Implementation update: the follow-up Flow implementation pass addressed all findings below. This section is retained as the trace from original finding to implemented fix.

## Implemented Findings Trace

### 1. Local AI model-download cancellation could not promptly interrupt a stalled stream read

Original severity: medium, UX/reliability

Evidence: cancellation sets the model-download job cancellation flag through `cancel_local_ai_model_download` in `src-tauri/src/commands/local_ai_commands.rs:44`, while the download worker checks `job.should_cancel()` only after the blocking iterator yields a streamed line in `src-tauri/src/local_ai/download.rs:182`. The download client now has an overall one-hour timeout in `src-tauri/src/local_ai/mod.rs:20` and `src-tauri/src/local_ai/mod.rs:35`, so the old no-timeout problem is fixed, but a stalled response body can still delay a user-visible cancel action until a line read returns or the long timeout expires.

Guards checked: the endpoint is fixed to local Ollama at `src-tauri/src/local_ai/mod.rs:15`; normal success, error, cancel, and panic paths transition the job to terminal states in `src-tauri/src/local_ai/download.rs:157`; Local AI download panics are now caught by the command worker. Those guards do not make cancellation abort the in-flight blocking read.

Why it matters: a Cancel button should be responsive even when Ollama or the local network stack stalls mid-pull. In this desktop context the impact is user trust and recoverability, not service availability.

Implemented fix: Local AI downloads now use async chunk streaming with a bounded per-read timeout, so cancellation is observed after the next chunk or timeout instead of waiting on an unbounded blocking read.

### 2. Direct paste and quick workflows could invoke processing before persisted settings load

Original severity: medium

Evidence: persistent settings initialize from defaults while `settingsLoaded` starts false in `frontend/src/hooks/usePersistentSettings.ts:12`. The CSV workflow computes `settingsDisabled` from that flag in `frontend/src/hooks/useAnonymizerWorkflow.ts:121`, but `App` passes only `workflow.settings` and `workflow.localAi` into paste and quick views in `frontend/src/App.tsx:70` and `frontend/src/App.tsx:87`. Paste enables analysis/preview/transform based on content, selection, busy state, and Local AI readiness in `frontend/src/components/PasteDataWorkflowView.tsx:83`, then sends current deterministic, seed, sample, and Local AI settings in `frontend/src/components/PasteDataWorkflowView.tsx:129` and `frontend/src/components/PasteDataWorkflowView.tsx:157`. Quick generation similarly enables on count/busy/Local AI state in `frontend/src/components/QuickDataTypeWorkflowView.tsx:37` and sends current deterministic/seed settings in `frontend/src/components/QuickDataTypeWorkflowView.tsx:51`.

Guards checked: the earlier settings overwrite race is fixed because `persistSettings` returns before load in `frontend/src/hooks/usePersistentSettings.ts:68`, settings controls are gated by the workflow, and tests cover the topbar/Browse disabled state before load. The remaining gap is direct processing actions using default settings if the user switches to paste or quick before persisted settings resolve.

Why it matters: deterministic defaults, private seed, preview sample count, and Local AI configuration affect generated output. A fast user action at launch can run with defaults rather than the user's saved privacy settings.

Implemented fix: `settingsLoaded` is passed into paste and quick workflows, and their processing actions stay disabled until persisted settings have loaded.

### 3. Direct Linux release downloads lacked independent integrity or provenance artifacts

Original severity: medium, release trust

Evidence: the README tells Linux users to download `.AppImage`, `.deb`, or `.rpm` files directly from GitHub Releases in `README.md:80`. The release workflow uploads those direct Linux assets in `.github/workflows/release.yml:563`, `.github/workflows/release.yml:564`, and `.github/workflows/release.yml:565`, but only uploads checksum and signature sidecars for the APT repository setup package in `.github/workflows/release.yml:566`, `.github/workflows/release.yml:567`, and `.github/workflows/release.yml:568`.

Guards checked: the APT repository path is signed, the setup package checksum is signed, macOS artifacts are signed and notarized, and release validation is otherwise broad. This finding is limited to direct Linux installers outside the signed APT path.

Why it matters: users who download a Linux installer directly from a release do not get a documented checksum/signature or artifact attestation path comparable to the APT install route.

Implemented fix: the release workflow now publishes signed checksum sidecars for direct `.deb`, `.rpm`, and AppImage assets, uploads the archive keyring, and documents verification in `README.md` and `docs/releasing.md`.

### 4. Remembered seed persistence was superseded by session-only seeds

Original severity: low/medium

Evidence: the reviewed seed-vault path kept repeatable-replacement seeds in the platform credential store. That was more persistent than the simplified product now needs.

Guards checked: disk settings cleared the seed before JSON persistence, but the platform credential-store path still created a long-lived local secret lifecycle.

Why it matters: repeatable seeds are sensitive enough to avoid durable storage when the product no longer needs cross-session seed recall.

Implemented fix: remembered seed storage was removed. The app keeps a repeatable seed only in the current session, settings load clears legacy persisted seeds, and settings save always writes an empty seed without touching the OS credential store.

### 5. Suggested output auto-grant accepted arbitrary suffix text before constructing the output path

Original severity: low, defense in depth

Evidence: `analyze_csv` accepts `output_suffix` from IPC in `src-tauri/src/commands/csv.rs:87`, passes it into `default_output_path_with_suffix` in `src-tauri/src/commands/csv.rs:106`, and auto-grants the suggested output path in `src-tauri/src/commands/csv.rs:116`. The suffix helper trims empty text but otherwise formats it into the generated filename in `src-tauri/src/commands/shared.rs:87`.

Guards checked: output path authorization canonicalizes parents and rejects symlink or non-file existing leaves in `src-tauri/src/path_access.rs`, and non-granted output paths require explicit dialog confirmation. The residual issue is that backend IPC input should still be validated as a filename suffix before it participates in an auto-granted path.

Why it matters: Tauri IPC is not a public network API, so this is not high severity. It is still better for the backend to reject path separators, parent components, and control characters in fields that are supposed to be suffixes.

Implemented fix: output suffixes are validated at the Tauri command boundary before suggested output paths are constructed and auto-granted.

### 6. CSV file run button stayed enabled when Local AI readiness was blocked

Original severity: low, UX/test consistency

Evidence: the workflow computes `localAiBlocked` in `frontend/src/hooks/useAnonymizerWorkflow.ts:122` and includes it in review blockers in `frontend/src/hooks/useAnonymizerWorkflow.ts:436`. However, `canAnonymize` in `frontend/src/hooks/useAnonymizeJob.ts:70` does not include Local AI readiness, and the primary button disables only on `!workflow.canAnonymize` in `frontend/src/components/workflow/AnonymizerWorkflowView.tsx:699`. Backend preflight checks Local AI readiness before starting in `src-tauri/src/commands/csv.rs:174`.

Guards checked: preview blocks Local AI readiness, paste and quick block Local AI actions, the UI displays a readiness blocker, and backend preflight prevents unsafe output. The remaining issue is that the CSV file button can still look actionable and then fail via preflight.

Why it matters: this does not appear to produce unsafe output, but it creates inconsistent behavior between CSV, paste, and quick workflows and leaves an avoidable error path uncovered by the frontend enablement model.

Implemented fix: CSV output creation is disabled when selected Smart replacement columns require Local AI setup, with frontend regression coverage.

### 7. CLI help did not document the deterministic seed requirement

Original severity: low

Evidence: the CLI now correctly rejects `--deterministic` without a non-empty seed in `crates/csv-anonymizer-app/src/cli.rs:110`, but help still lists `csv-anonymizer anonymize ... [--deterministic] [--seed <seed>]` without explaining that `--seed` is required when deterministic mode is enabled in `crates/csv-anonymizer-app/src/cli.rs:253`.

Guards checked: this is not the old empty-seed privacy bug; parser and core validation now block blank deterministic seeds, and tests cover missing/blank seed. The remaining issue is user-facing CLI contract clarity.

Why it matters: command-line users can discover the requirement only by triggering an error.

Implemented fix: CLI help now shows `--deterministic --seed <seed>` together and documents that deterministic anonymization requires a non-empty seed.

## Resolved Prior Findings

The previous version of this report contained several valid findings that are now fixed in the current workspace:

- Direct-input previews now include safety warnings: `crates/csv-anonymizer-core/src/direct_input/shared.rs:114` builds warnings, and tests cover selected-column warnings.
- Deterministic empty seeds are now blocked at CLI and core boundaries: `crates/csv-anonymizer-app/src/cli.rs:110` and `crates/csv-anonymizer-core/src/service.rs:601` enforce the guard.
- Background job panics now transition to terminal failure states: anonymize and Local AI workers catch panics and call terminal failure handlers.
- Local AI readiness is now model-specific in the frontend hook.
- Settings persistence no longer overwrites stored settings with defaults before the initial load resolves.
- Paste and quick workflows now render privacy report summaries they receive.

## Implementation Status

- Local AI model downloads now use an async streaming client with a bounded per-read timeout so cancellation is observed after the next chunk or timeout instead of waiting on an unbounded blocking read.
- Paste and quick workflows now keep processing actions disabled until persisted settings have loaded.
- Direct Linux `.deb`, `.rpm`, and AppImage release downloads now get signed checksum sidecars in the release workflow, with verification guidance in `README.md` and `docs/releasing.md`.
- Remembered seed storage was removed; repeatable seeds are session-only and omitted from settings JSON.
- Suggested output suffixes are now validated before the backend constructs and auto-grants output paths.
- CSV output creation is now disabled when selected Smart replacement columns require Local AI setup.
- CLI help now documents that deterministic anonymization requires a non-empty seed.

## Positive Findings

- Standard CSV-file processing remains streaming and uses atomic replacement paths rather than materializing standard transforms in memory.
- CSV output neutralizes spreadsheet formula prefixes in headers and cells, including full-width variants.
- Ragged rows with non-empty fields beyond headers are rejected before committing partial output.
- Tauri file/path access uses in-memory grants, canonicalization, and symlink/leaf checks.
- Frontend unit/e2e/a11y coverage exists for key workflows, model-specific Local AI readiness, settings-load guards, paste/quick privacy report rendering, keyboard navigation, and axe checks.
- CI and release workflows are broad: frontend/Rust audits, contract checks, dead-code scans, frontend lint/test/e2e/a11y/build, Rust fmt/test/clippy/build, packaging checks, release metadata validation, APT repository checks, and smoke tests.
- Release metadata scripts validate cross-file versions, Linux desktop/AppStream metadata, icons, changelog tag expectations, and local model/runtime artifact exclusions.

## Implemented Follow-up Order

1. User-facing privacy decision gaps were fixed first: direct workflow settings-load gating and CSV Local AI button disable consistency.
2. Local AI download cancellation was hardened with bounded per-read timeout behavior.
3. Direct Linux installer provenance was added with signed checksum sidecars and verification docs.
4. Remembered seed storage was removed in favor of session-only repeatable seeds.
5. Output suffixes are validated at the Tauri command boundary before suggested output paths are auto-granted.
6. CLI help now documents that deterministic anonymization requires a non-empty seed.
7. The resolved-prior-findings list remains for the next review cycle so future work does not chase stale issues.
