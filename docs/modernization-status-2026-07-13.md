# Modernization Status - 2026-07-13

## Status And Evidence Boundary

This is the dated current-state companion to the historical reviews in [`code-quality-assessment-2026-07-01.md`](code-quality-assessment-2026-07-01.md) and [`full-codebase-review-and-improvement-plan-2026-07-01.md`](full-codebase-review-and-improvement-plan-2026-07-01.md). Those documents remain evidence of what their reviews observed; this document does not rewrite their original findings.

Six modernization features completed targeted implementation, validation, and review. The final broad root-gate sweep and detailed whole-project review also completed on this date; their results and residual limits are recorded below.

## Delivered Feature Map

### 1. Privacy Evidence Integrity

Type overrides now change the effective type without discarding previously detected sensitive-data findings or evidence. `service/controls.rs` merges unique findings and evidence and computes risk with `max_pii_risk`, so a low-risk override cannot lower an already observed risk. `service/privacy_report.rs` classifies each selected column from the strongest class represented by its effective type or retained evidence; direct identifiers take precedence over quasi-identifiers.

Regression coverage exercises CSV analysis/preview, preflight detector-risk evidence, and direct-input reporting, including sensitive evidence found beyond the five displayed sample values.

Implementation evidence:

- `crates/csv-anonymizer-core/src/service/controls.rs`
- `crates/csv-anonymizer-core/src/service/privacy_report.rs`
- `crates/csv-anonymizer-core/src/service/tests/analysis_preview.rs`
- `crates/csv-anonymizer-core/src/service/tests/preflight.rs`
- `crates/csv-anonymizer-core/src/direct_input/tests.rs`

### 2. Frontend Async Correctness

`useLocalAi` assigns a monotonically increasing sequence to status refreshes. Only the latest request may publish success or failure, and unmount cleanup invalidates pending refreshes. This applies to automatic, manual, and download-triggered refreshes. The clipboard utility now treats a false or throwing `document.execCommand('copy')` fallback as failure instead of showing false success, while still removing its temporary textarea.

Implementation evidence:

- `frontend/src/hooks/useLocalAi.ts`
- `frontend/src/hooks/useLocalAi.test.tsx`
- `frontend/src/utils/clipboard.ts`
- `frontend/src/utils/clipboard.test.ts`

### 3. Native Lifecycle Safety

The Tauri backend deliberately serializes anonymization writers: only one anonymization job may hold the active-writer lease, regardless of output-path spelling or platform path identity. Normal, canceled, failed, and panic terminal paths release the lease before publishing observable terminal status, after output processing has ended.

Settings loads and saves share a lock. Saves use unique same-directory temporary names followed by replacement, and loads rewrite migrated or sanitized state. Disabling remembered paths clears input/output directories from both the returned settings and persisted JSON. IPC commands enforce the configured sample-row and preview-sample limits rather than relying on frontend controls alone.

Implementation evidence:

- `src-tauri/src/jobs.rs`
- `src-tauri/src/commands/job_commands.rs`
- `src-tauri/src/commands/csv.rs`
- `src-tauri/src/settings/model.rs`
- `src-tauri/src/settings/store.rs`

### 4. Core Module Boundaries

`crates/csv-anonymizer-core/src/service.rs` is now a 221-line public facade. Focused internal modules own control application, path validation, preflight, preview, and privacy-report construction:

- `service/controls.rs`
- `service/path_validation.rs`
- `service/preflight.rs`
- `service/preview.rs`
- `service/privacy_report.rs`

File previews and CSV/JSON/YAML/XML/plain-text/log direct-input previews now share preview orchestration through `service/preview.rs` and `direct_input/shared.rs`. Public service calls, detector ordering, serialization, and frontend DTO names remain unchanged.

`crates/csv-anonymizer-core/src/types.rs` intentionally remains centralized. This is a deliberate contract boundary, not unfinished extraction: `scripts/check-contracts.mjs` directly reads that file and `frontend/src/types.ts` to compare the current 12 enums and 24 structs. Splitting it remains deferred until the checker is redesigned or DTO churn supplies a stronger reason.

### 5. Frontend Module Boundaries

Pasted-data state, guards, analysis, preview, transformation, copy, and selection orchestration now live in `usePasteDataWorkflow`. The CSV workflow remains in the existing `useAnonymizerWorkflow`; no generic workflow framework was introduced. `RiskBadge` accepts the domain `PiiRisk` type rather than a broad string.

Repository-wide selector checks and Knip supported removal of orphaned rules from the control, data, and responsive stylesheets. These changes were not an intended visual redesign: mounted tab state, focus behavior, modal/Escape behavior, responsive behavior, and accessibility were kept within the existing product contract.

Implementation evidence:

- `frontend/src/hooks/usePasteDataWorkflow.ts`
- `frontend/src/hooks/usePasteDataWorkflow.test.tsx`
- `frontend/src/hooks/useAnonymizerWorkflow.ts`
- `frontend/src/components/PasteDataWorkflowView.tsx`
- `frontend/src/components/RiskBadge.tsx`
- `frontend/src/styles/controls.css`
- `frontend/src/styles/data.css`
- `frontend/src/styles/responsive.css`

### 6. Quality-Gate Hardening

Documentation commands are checked against `package.json` and direct repository script references, with tests for failure cases. Linux DEB metadata validation has synthetic regressions that do not require hosted artifacts. CI/release ordering and command contracts have static regression tests. Shared validation installs exact cargo-audit `0.22.2` and cargo-machete `0.9.2` versions.

Temporary RustSec exceptions are fail-closed: each is bound to the raw advisory finding's crate and version, the sole immediate dependency parent is checked with `cargo tree`, and both current exceptions expire on 2026-10-01. The required audit still reports the remaining warning inventory instead of hiding it.

The obsolete `build/entitlements.mac.inherit.plist` was removed only after a repository consumer check found no use. Release signing, notarization, draft publication, artifact signing, and APT publication semantics were not intentionally changed by this feature.

Implementation evidence:

- `.github/actions/validate-build/action.yml`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`
- `scripts/check-docs.mjs`
- `scripts/cargo-audit.mjs`
- `scripts/__tests__/check-docs.test.mjs`
- `scripts/__tests__/cargo-audit.test.mjs`
- `scripts/__tests__/workflow-hardening.test.mjs`
- `tests/test_linux_package_metadata.py`
- `docs/releasing.md`

## Preserved Invariants And Contracts

- **Privacy remains conservative:** overrides cannot erase retained evidence or lower observed risk, and reporting uses the strongest effective/evidence identifier class.
- **Local-first processing remains the boundary:** rule-based detection/transformation stays in Rust; optional Smart replacement targets configured local Ollama.
- **Wire and public contracts remain stable:** service/module extraction did not intentionally change serde casing, DTO field names, optionality, public service entrypoints, or detector order. The contract checker still covers 12 enums and 24 structs.
- **CSV and direct-input behavior converge:** shared preview orchestration covers file, CSV paste, JSON, YAML, XML, plain text, and logs while preserving format-specific parsing/rendering.
- **Native writes remain conservative:** one active anonymization writer prevents ambiguous concurrent output ownership; settings replacement is serialized and same-directory.
- **Frontend behavior remains recognizable:** orchestration moved behind hooks without an intended UI redesign or mounted-state/accessibility regression.
- **Release integrity remains intact:** quality-gate changes validate the existing signing and publication graph rather than weakening or replacing it.

## Intentional Deferrals And Residual Risks

1. `types.rs` remains centralized because the current contract checker consumes it directly. A domain split is deferred until contract tooling or DTO churn justifies the migration.
2. Hosted DEB/RPM packaging and macOS signing/notarization were not reproduced in this local environment. Synthetic Linux metadata tests and static workflow checks reduce risk but do not replace hosted release execution.
3. The latest required cargo-audit run checked 532 dependencies and still reported 18 warnings. Passing the wrapper means the current policy was enforced; it does not mean the warning inventory is zero.
4. The two temporary `quick-xml` `0.39.4` RustSec exceptions are limited to the sole `plist` dependency path and expire on 2026-10-01. They require removal, replacement, or an explicitly reviewed renewal before that date.
5. `npm run artifacts:rust:check` correctly rejected pre-existing generated v1.0.11 artifacts in `dist/rust/artifacts` because they did not match the then-current source version. Those local artifacts were not deleted or treated as current release evidence.
6. The detailed whole-project review passed without a blocking privacy, contract, lifecycle, frontend, or release-integrity finding. The platform and dependency risks above remain explicit follow-up work.

## Canonical Validation Matrix

Run commands from the repository root. The results below combine the completed feature checks with the final broad convergence sweep.

| Surface | Canonical command | Evidence available on 2026-07-13 |
| --- | --- | --- |
| Rust formatting | `npm run fmt` | Passed across the workspace. |
| Lint/clippy | `npm run lint` | Frontend ESLint and workspace all-target clippy passed with warnings denied. |
| Contracts and tests | `npm run test` | 7 CLI, 247 core, 46 Tauri, and 51 frontend tests passed; the 12-enum/24-struct contract check also passed. |
| Type compatibility | `npm run typecheck` | Frontend application/e2e TypeScript and workspace `cargo check` passed. |
| Dead code/dependencies | `npm run deadcode:required` | Full, production, and strict Knip plus required cargo-machete passed. |
| Documentation | `npm run docs:check` | 4 documentation checker tests and validation of all 14 current Markdown files passed. |
| Tooling regressions | `npm run tooling:test` | 6 audit-policy, 3 workflow-integrity, and 4 synthetic Linux metadata tests passed. |
| Release metadata | `npm run release:check` | Release metadata and synthetic Linux metadata regressions passed before the v1.0.76 release bump. |
| Frontend production build | `npm run frontend:build` | TypeScript and a 111-module Vite production build passed. |
| Browser workflow | `npm run frontend:e2e` | 4 Chromium workflow tests passed. |
| Accessibility | `npm run frontend:a11y` | 1 accessibility check passed with no automated Axe violations. |
| Frontend dependencies | `npm run frontend:audit` | Passed with 0 vulnerabilities. |
| Rust dependencies | `npm run cargo:audit:required` | Passed policy checks over 532 dependencies; 18 warnings remained visible. |
| Tauri prebuilt contract | `npm run tauri:prebuilt:check` | Passed its expected incomplete-dist rejection cases and final built-dist acceptance. |
| Rust artifact contract | `npm run artifacts:rust:check` | Correctly failed on stale local v1.0.11 generated artifacts; no current release artifacts were available. |
| Linux package-manager path | `npm run linux:package-manager:check` | Synthetic metadata coverage passed; complete hosted DEB/RPM execution remains environment-limited. |
| Rust smoke | `npm run smoke:rust` | Passed with all five fixture email values removed from output. |

The final review should treat the stale local artifact mismatch and hosted platform limits as explicit residual evidence; it should not backfill those unavailable checks as passed.
