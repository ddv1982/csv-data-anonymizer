# Cleanup Phased Plan - 2026-07-01

## Scope

This plan starts from the GitHub wiki page `cleanup-opportunities--complexity-hotspots`, then reconciles it with the current repository state. The goal is behavior-preserving cleanup, not new product behavior.

Deployment context: local-first desktop app plus CLI smoke harness. Privacy, path-access, release-signing, and Local AI readiness guards are not cleanup targets unless a later change proves a narrower equivalent.

## Current Status

Several wiki candidates are already complete in the current repository and should not be re-planned as fresh cleanup work:

- Auto-selection policy now lives in the Rust core as `should_auto_select_column` in `crates/csv-anonymizer-core/src/metadata.rs:47-49`, is re-exported from `crates/csv-anonymizer-core/src/lib.rs:18`, and is consumed by both CLI and Tauri in `crates/csv-anonymizer-app/src/cli.rs:1-3` and `src-tauri/src/commands/csv.rs:9-14`.
- The speculative `isSelectableColumn` helper is gone from current frontend selection code; `frontend/src/hooks/useColumnSelection.ts:105` now returns all current columns as selectable, and `frontend/src/components/ColumnTable.tsx:84-159` no longer renders disabled/non-selectable branches.
- CSV and paste workflows share `ColumnSelectionPanel` in `frontend/src/components/ColumnSelectionPanel.tsx:12-84`, with call sites in `frontend/src/components/workflow/AnonymizerWorkflowView.tsx` and `frontend/src/components/PasteDataWorkflowView.tsx`.
- Local AI blocked messaging is centralized in `frontend/src/components/LocalAiBlockedAlert.tsx` and reused by CSV, paste, and quick workflows.
- Frontend DTO test builders exist in `frontend/src/test-utils/builders.ts`, with local per-test wrappers kept where they encode test intent.
- Docs-only changes now have a lightweight CI path through `run_docs_ci` in `.github/workflows/ci.yml:56-61` and the docs job in `.github/workflows/ci.yml:77-93`.
- High-risk DTO serialization tests already cover `ColumnMetadata`, `PreflightParams`, `PrivacyReport`, defaulted privacy report fields, and selected enum wire values in `crates/csv-anonymizer-core/src/types.rs:730-956`.
- The Tauri prebuilt frontend contract has already been moved into `scripts/check-tauri-prebuilt-frontend.sh`, which CI calls at `.github/workflows/ci.yml:188-189`.

This document was later used as the implementation checklist for a behavior-preserving cleanup Flow on 2026-07-01. Local repository work completed Phases 1 through 5. The GitHub wiki page was not edited from this local workspace; link or copy this status there manually if wiki freshness is required.

Completed cleanup in that Flow:

- Phase 1 split quick generated-value construction into data-type family helpers and split preflight readiness assembly into focused evidence helpers.
- Phase 2 extracted Linux checksum signing and APT installer staging into `scripts/sign-linux-checksums.sh`, `scripts/stage-apt-installer-assets.sh`, and `scripts/check-linux-release-helpers.sh`, with `.github/workflows/release.yml` delegating to them.
- Phase 3 expanded `scripts/check-contracts.mjs` to cover existing nested detection/privacy DTO contracts, reaching 12 enum contracts and 24 struct contracts without splitting `types.rs`.
- Phase 4 added a shared `localAiStatusFixture`, reused existing detected-risk selection state in the CSV workflow view, and kept workflow seams narrow.
- Phase 5 refreshed these docs to distinguish completed local cleanup from deferred/manual wiki freshness work.

## Phase 1: Stabilize Remaining Rust Complexity Hotspots

Status: completed locally on 2026-07-01.

Goal: reduce long-function review risk in core paths without changing detector behavior or release-readiness semantics.

Targets:

- `crates/csv-anonymizer-core/src/direct_input/quick.rs:189-263` (`generated_quick_value`).
- `crates/csv-anonymizer-core/src/service.rs:63-202` (`preflight_anonymization`).
- Existing tests in `crates/csv-anonymizer-core/src/direct_input/tests.rs` and `crates/csv-anonymizer-core/src/service/tests/preflight.rs`.

Recommended shape:

- Split `generated_quick_value` by stable data-type families or small generator helpers only where it removes match-arm density; keep generated examples and ranges equivalent.
- Split `preflight_anonymization` around existing responsibilities: input/column validation, output path evidence, Local AI readiness evidence, and release readiness assembly.
- Do not touch detector classification, Smart replacement behavior, output path validation, or readiness wording except where tests pin intended wording.

Validation:

- `cargo test -p csv-anonymizer-core direct_input`
- `cargo test -p csv-anonymizer-core preflight`
- `cargo clippy --workspace --all-targets -- -D warnings`

Exit criteria:

- Optional `clippy::too_many_lines` candidates are shorter or intentionally documented as not worth splitting further.
- Tests prove quick generation and preflight blocker/readiness behavior are unchanged.

Completion evidence:

- `cargo test -p csv-anonymizer-core direct_input`
- `cargo test -p csv-anonymizer-core preflight`
- `cargo clippy --workspace --all-targets -- -D warnings`

## Phase 2: Script Release Workflow Shell Blocks

Status: completed for Linux checksum/signature and APT installer staging on 2026-07-01. macOS signing/notarization blocks remain visible in YAML and can be revisited only if they become a review hotspot.

Goal: make release signing and artifact staging easier to test locally without weakening release gates.

Targets:

- macOS signing setup and notarization blocks in `.github/workflows/release.yml:209-303`.
- Linux signing setup and checksum/staging blocks in `.github/workflows/release.yml:409-545`.
- Existing release helper scripts under `scripts/`, especially `scripts/notarize-macos-artifact.sh`, `scripts/package-rust-macos.mjs`, `scripts/package-tauri-linux.mjs`, `scripts/build_apt_repository.py`, and `scripts/check-apt-installer.mjs`.

Recommended shape:

- Extract one cohesive script at a time, starting with the Linux checksum/signature loop because it is self-contained and has deterministic file inputs.
- Keep secret validation explicit and close to the script entrypoint; do not hide missing-secret failures behind generic wrappers.
- Leave short package-install and command-composition steps in YAML when the workflow context is clearer than a script.

Validation:

- `npm run release:check`
- `npm run linux:metadata:test`
- script-level dry-run or fixture mode for any extracted release helper where secrets are not available locally.
- Linux CI or builder validation for `npm run linux:package-manager:check` when package artifacts are involved.

Exit criteria:

- The release workflow delegates testable logic to scripts while keeping GitHub Actions permissions, secret wiring, and upload actions visible.

Completion evidence:

- `npm run linux:release-helpers:check`
- `npm run release:check`
- `npm run linux:metadata:test`
- `bash -n scripts/sign-linux-checksums.sh scripts/stage-apt-installer-assets.sh scripts/check-linux-release-helpers.sh`

## Phase 3: Keep DTO Contracts Explicit, Add Tests Only With Churn

Status: completed locally on 2026-07-01 for the confirmed contract-check coverage gap. No DTO module split or schema-generation layer was added.

Goal: avoid premature module splitting while preserving confidence in Rust-to-TypeScript contracts.

Targets:

- `crates/csv-anonymizer-core/src/types.rs` remains a broad DTO surface and is currently 957 lines.
- `scripts/check-contracts.mjs:49-59` compares selected enum/struct surfaces, with regex parsing in `scripts/check-contracts.mjs:71-140`.
- Current serialization tests in `crates/csv-anonymizer-core/src/types.rs:730-956`.

Recommended shape:

- Keep `types.rs` explicit until a feature creates a strong reason to split by domain, such as privacy reports, release readiness, direct input, or workflow params.
- For future high-risk DTO changes, add serialization or deserialization tests next to the Rust DTOs before considering generated bindings.
- Do not introduce a schema-generation layer until regex contract checks plus targeted serialization tests become insufficient.

Validation:

- `npm run contracts:check`
- `cargo test -p csv-anonymizer-core types`
- `npm run frontend:typecheck`

Exit criteria:

- DTO changes have a local test oracle for casing, optional/default fields, and nested report/readiness shapes.
- Any eventual split preserves public re-exports from `crates/csv-anonymizer-core/src/lib.rs` unless a separately approved API change says otherwise.

Completion evidence:

- `npm run contracts:check`
- `cargo test -p csv-anonymizer-core types`
- `npm run frontend:typecheck`

## Phase 4: Frontend Fixture And Selection Maintenance

Status: completed locally on 2026-07-01 for fixture completeness and detected-risk selection reuse. No visual/layout changes or generic workflow framework were added.

Goal: keep the already-consolidated frontend seams small instead of growing a generic workflow framework.

Targets:

- `frontend/src/components/ColumnSelectionPanel.tsx`
- `frontend/src/hooks/useColumnSelection.ts`
- `frontend/src/test-utils/builders.ts`
- Workflow tests such as `frontend/src/App.test.tsx` and `frontend/src/hooks/useAnonymizerWorkflow.test.tsx`.

Recommended shape:

- Keep `ColumnSelectionPanel` as a narrow shared seam for bulk actions, notices, the table, and footer content.
- Keep per-test fixture wrappers when they encode scenario-specific defaults, but route broad DTO completeness through `frontend/src/test-utils/builders.ts`.
- Do not add workflow-level abstractions unless CSV, paste, and quick modes gain another repeated policy that must change together.

Validation:

- `npm run frontend:test`
- `npm run frontend:typecheck`
- `npm run frontend:e2e` when selection, Local AI blocking, or workflow enablement changes.
- `npm run frontend:a11y` when shared workflow UI changes.

Exit criteria:

- Selection and Local AI setup behavior remain covered without duplicating broad DTO construction across tests.

Completion evidence:

- `npm run frontend:test`
- `npm run frontend:typecheck`
- `npm run frontend:lint`

## Phase 5: Housekeeping And Documentation Freshness

Status: completed locally on 2026-07-01 for repository docs. Wiki freshness remains a manual follow-up outside this local workspace.

Goal: keep cleanup tracking accurate as code moves faster than assessment docs or wiki pages.

Targets:

- `docs/code-quality-assessment-2026-07-01.md`
- `docs/dependency-audit-followups.md`
- GitHub wiki cleanup hotspot page.
- `docs/cleanup-phased-plan-2026-07-01.md`

Recommended shape:

- Update the wiki or link it to this plan after each phase so completed candidates are not rediscovered.
- Prefer a short status update over rewriting the full assessment unless the evidence set materially changes.
- Keep release metadata churn separate from cleanup PRs unless the cleanup is specifically about release scripts.

Validation:

- `npm run docs:check`

Exit criteria:

- The cleanup backlog distinguishes completed, active, deferred, and explicitly rejected work.

Completion evidence:

- `npm run docs:check`
- Final broad gates recorded by the Flow session.

Deferred/manual follow-up:

- Update or link the GitHub wiki cleanup hotspot page to this document so the wiki does not rediscover completed local cleanup.

## Not Recommended Now

- Do not split the entire Rust core DTO surface just because `types.rs` is long; current tests and contract checks already cover important wire behavior.
- Do not build a generic frontend workflow framework; the current shared seams are intentionally narrow.
- Do not remove privacy, path-access, Local AI, release-signing, or data-loss guards as line-count cleanup.
- Do not mix dependency major upgrades, release metadata version churn, and cleanup refactors in one PR.

## Suggested Order

1. Phase 1 first if the next core feature touches quick generation or preflight readiness.
2. Phase 2 first if the next release cycle spends review time on YAML shell blocks.
3. Phase 3 only when DTO churn creates a concrete compatibility risk.
4. Phase 4 opportunistically with frontend workflow changes.
5. Phase 5 after any phase completes, so the wiki and docs stay aligned with the repository.
