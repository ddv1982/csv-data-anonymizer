# Code Quality Assessment - 2026-07-01

## Status Notice - 2026-07-13

This document remains a dated assessment and historical review record. Its file references, line counts, candidate findings, and recommendations describe the repository at the time of the review (plus the explicitly dated updates already embedded below); they are not a new 2026-07-13 inventory.

Six later modernization features delivered privacy-evidence integrity, frontend async correctness, native lifecycle safety, focused Rust and React module boundaries, and stronger quality gates. In particular, `service.rs` is now a 221-line facade over focused service modules, while `types.rs` remains intentionally centralized because the contract checker consumes it directly. See [`modernization-status-2026-07-13.md`](modernization-status-2026-07-13.md) for the current implementation map, preserved invariants, targeted validation evidence, and residual risks.

The broad project-convergence gate and passing detailed final review are recorded in that status document. Retain the assessment below as evidence of what earlier reviews actually observed.

## Scope

Reviewed CSV Anonymizer as a local-first desktop app with a Rust anonymization core, Tauri command shell, React/Vite frontend, release/package scripts, and GitHub Actions gates.

Deployment context used for risk: single-user desktop application plus a local CLI smoke harness. Optional Smart replacement traffic is intended for local Ollama on loopback, not a shared hosted service.

This assessment is review-first. It does not change behavior or refactor code. Apparent smells were treated as candidates until surrounding guards, tests, or workflow context were checked.

Update: a later 2026-07-01 behavior-preserving cleanup pass implemented the local follow-up plan in `docs/cleanup-phased-plan-2026-07-01.md`. Treat the detailed candidate list below as historical evidence; the phased plan is now the fresher status source for completed cleanup, deferred work, and manual wiki freshness.

## External Calibration

- Rust API Guidelines emphasize predictable naming, meaningful error types, Serde-compatible data structures, argument validation, useful `Debug`, common metadata, and type distinctions for domain constraints. Source: Rust API Guidelines checklist, https://rust-lang.github.io/api-guidelines/checklist.html.
- React's official guidance says effect dependencies should match the code, dependency lint suppression creates a high bug risk, and Effect Events should not be used to hide real dependencies. Sources: https://react.dev/learn/removing-effect-dependencies and https://react.dev/reference/react/useEffectEvent#pitfall-skip-dependencies.
- Cleanup rubric used locally: duplicate logic is only actionable when it must change together; verbose guards protecting data loss, security, lifecycle ordering, or error observability are not smells until proven otherwise.

## Overall Assessment

The project is already operating at a high code-quality baseline. It has stronger-than-average gates for a desktop app, meaningful separation between Rust core, Tauri shell, frontend, packaging, and release concerns, and tests around privacy-sensitive behavior.

The main risk is no longer basic hygiene. The next quality ceiling is controlling incremental complexity: repeated selection policy, duplicated frontend workflow panels and fixtures, a broad DTO/type surface, and large release/CI shell blocks. None of these are emergency defects. They are the kind of moderate DRY and maintainability risks that become expensive if more workflows, formats, or release targets are added without consolidation.

## Strong Standards Already In Place

### 1. The automated quality gate is broad

Evidence:

- Root scripts expose canonical checks for frontend lint/test/typecheck/audit/dead-code, contract checks, release metadata, Rust fmt/clippy/test/build/check, cargo-audit/machete wrappers, and smoke tests in `package.json:8-48`.
- Frontend scripts include Vite build, ESLint, Vitest, Playwright e2e/a11y, TypeScript typecheck, and Knip scans in `frontend/package.json:7-16`.
- CI runs frontend audit, Rust audit, release metadata validation, frontend/backend contract checks, dead-code checks, frontend lint/test/e2e/a11y/build, Rust fmt/test/clippy/build, and smoke tests in `.github/workflows/ci.yml:115-216`.
- The weekly dead-code workflow runs the required dead-code gate in `.github/workflows/dead-code.yml:3-47`.

Assessment: this is a real standards system, not just a README list. It covers style, type shape, unit behavior, browser behavior, accessibility, dead code, dependency drift, supply-chain audit, packaging, and release metadata.

### 2. Rust layering and domain modeling are strong

Evidence:

- The workspace separates `csv-anonymizer-core`, `csv-anonymizer-app`, and `src-tauri` in `Cargo.toml:1-6`.
- App and Tauri crates depend on the core by path rather than duplicating the engine in `crates/csv-anonymizer-app/Cargo.toml:14-15` and `src-tauri/Cargo.toml:13-14`.
- Core exports service/types/smart interfaces without Tauri dependencies in `crates/csv-anonymizer-core/src/lib.rs:1-34`.
- Core domain enums use explicit serde casing and typed variants in `crates/csv-anonymizer-core/src/types.rs:4-29`.

Assessment: the architecture fits Rust API guidance better than many application codebases. The core is reusable, the shell is thin, and data contracts are explicit.

### 3. Privacy-sensitive CSV handling has concrete safety guards

Evidence:

- Ragged rows with non-empty fields beyond headers are rejected before output in `crates/csv-anonymizer-core/src/csv_io.rs:310-335`.
- Spreadsheet formula prefixes, including full-width variants, are neutralized in `crates/csv-anonymizer-core/src/csv_io.rs:338-384`.
- Tauri path access requires granted canonical input files and normalized output files in `src-tauri/src/path_access.rs:19-57`.
- Output suffix validation rejects separators and control characters in `src-tauri/src/commands/shared.rs:87-117`.

Assessment: these are the kinds of guards that should not be "simplified" away. They add code, but they protect data loss, path access, and privacy/report correctness.

### 4. Frontend hook discipline is better than average

Evidence:

- ESLint includes TypeScript recommended rules, React Hooks recommended rules, unused-var enforcement, and React Refresh checks in `frontend/eslint.config.js:11-36`.
- Job polling updates through a ref and cleans up timers in `frontend/src/hooks/useAnonymizeJob.ts:63-107`.
- Settings load/save sequencing guards stale and in-flight saves in `frontend/src/hooks/usePersistentSettings.ts:12-19` and `frontend/src/hooks/usePersistentSettings.ts:68-97`.
- No `eslint-disable`, `@ts-ignore`, `@ts-expect-error`, `TODO`, `FIXME`, or `HACK` markers were found under `frontend/src` during this review.

Assessment: this aligns well with React's current guidance. The code is not suppressing dependency warnings to make effects pass, and the polling/settings hooks show deliberate lifecycle handling.

### 5. Release/package checks are unusually strong

Evidence:

- Release metadata validation checks semver and version sync across package, frontend lockfile, Tauri config, Cargo workspace, required icons, forbidden model/runtime artifacts, Linux desktop/AppStream metadata, changelog tag/date, and template fields in `scripts/check-release-metadata.mjs:76-209`.
- CI validates the prebuilt frontend contract inline in `.github/workflows/ci.yml:162-201`.
- The 2026-06-29 review records that previous release integrity and privacy findings were implemented rather than left stale in `docs/codebase-review-2026-06-29.md:18-20` and `docs/codebase-review-2026-06-29.md:117-145`.

Assessment: release quality is treated as product quality, which is appropriate for a downloadable privacy tool.

## DRY And Complexity Risks

### 1. Auto-selection policy is duplicated across app and Tauri

Status update: completed after this assessment. The core now owns `should_auto_select_column`, and CLI/Tauri callers consume that shared policy.

Severity: medium.

Evidence:

- CLI app policy: `crates/csv-anonymizer-app/src/app_logic.rs:3-5`.
- Tauri policy: `src-tauri/src/commands/shared.rs:83-85`.
- Both implementations require sample values and high/medium risk.

Why it matters: this is a product policy, not UI wiring. If the rule changes for one entrypoint and not the other, CLI and desktop behavior can drift.

Refutation checked: the duplication is tiny, and both copies currently match exactly. This is not a current bug. It is still a good consolidation candidate because the owning boundary is the core metadata policy.

Best fix shape: move `should_auto_select` into `csv-anonymizer-core` near `ColumnMetadata` or service metadata helpers, then call it from CLI and Tauri.

### 2. Frontend workflow panels repeat selection and Local AI blocking patterns

Status update: completed after this assessment for the confirmed shared seams. CSV and paste workflows share `ColumnSelectionPanel`, Local AI blocked messaging is centralized, and the later cleanup reused existing detected-risk selection state without adding a generic workflow framework.

Severity: medium.

Evidence:

- CSV workflow bulk actions and table wiring are in `frontend/src/components/workflow/AnonymizerWorkflowView.tsx:130-192`.
- Paste workflow has very similar bulk actions and table wiring in `frontend/src/components/PasteDataWorkflowView.tsx:265-316`.
- Local AI blocked messaging appears in job logic and workflow UI, for example `frontend/src/hooks/useAnonymizeJob.ts:140-146` and `frontend/src/components/PasteDataWorkflowView.tsx:308-316`.

Why it matters: selection behavior and Local AI readiness are privacy-path UX. Repeating them across CSV, paste, and quick modes makes small policy changes prone to shotgun edits.

Refutation checked: some separation is justified because CSV, paste, and quick workflows have different step labels and enablement rules. Tests cover several blocked Local AI paths, so this is maintainability risk rather than exposed incorrectness.

Best fix shape: extract only the shared seam, such as a `ColumnSelectionPanel` and a small Local AI blocker/message helper. Avoid a generic workflow framework.

### 3. A current no-op selectable-column abstraction adds branches without current value

Status update: completed after this assessment. The speculative frontend selectable-column abstraction and disabled/non-selectable branches are gone from the current selection/table flow.

Severity: low/medium.

Evidence:

- `isSelectableColumn` always returns `true` in `frontend/src/utils/columns.ts:5-7`.
- Selection hook filters and guards on it in `frontend/src/hooks/useColumnSelection.ts:25` and `frontend/src/hooks/useColumnSelection.ts:89-97`.
- Column table renders muted/non-selectable branches in `frontend/src/components/ColumnTable.tsx:84-116`.

Why it matters: if there is no product concept of unselectable columns, this is speculative generality. It makes the table and hook look more capable than the current domain requires.

Refutation checked: it may be a deliberate compatibility seam for future columns with no sample data or unsupported transform types. No current source in this review showed a non-selectable column case.

Best fix shape: either document and test the intended unselectable condition, or remove the abstraction and branches until the product actually needs them.

### 4. Test fixtures are duplicated across frontend tests

Status update: completed after this assessment for broad DTO fixtures. Shared builders now cover column metadata, privacy reports, verified preflight data, and Local AI status while local wrappers remain for scenario intent.

Severity: low.

Evidence:

- `columnFixture` exists in `frontend/src/App.test.tsx:475-492` and `frontend/src/hooks/useAnonymizerWorkflow.test.tsx:296-313`, with more copies in e2e support code.
- `privacyReportFixture` exists in `frontend/src/App.test.tsx:494-526`, `frontend/src/hooks/useAnonymizerWorkflow.test.tsx:346-378`, and `frontend/src/components/PrivacyReportSummary.test.tsx:93-125`.

Why it matters: contract DTOs are broad. When a report field is added, many tests need fixture updates, and inconsistent defaults can hide or create noise.

Refutation checked: local fixture duplication can keep tests readable, and the fixture values differ slightly by test intent. Do not centralize everything. Centralize only stable DTO builders that represent contract completeness.

Best fix shape: add small test-data builders for `ColumnMetadata`, `PrivacyReport`, and preflight data under frontend test utilities, while allowing per-test overrides.

### 5. Core DTO surface is broad and centralized

Status update: intentionally deferred after this assessment. `types.rs` remains explicit and centralized; the later cleanup expanded contract-check coverage for existing nested DTOs instead of splitting modules prematurely.

Severity: medium, trend risk.

Evidence:

- `crates/csv-anonymizer-core/src/types.rs` currently holds many public enums/structs and is 722 lines long.
- `crates/csv-anonymizer-core/src/lib.rs:22-34` re-exports a wide set of DTOs and report/readiness types.
- `scripts/check-contracts.mjs:10-69` guards enum/struct names and fields across Rust and TypeScript.

Why it matters: broad DTO files make cross-feature changes easier to start but harder to reason about as privacy reports, release readiness, direct input, and CSV workflows grow.

Refutation checked: the file is still explicit and typed, and the contract check catches missing/extra enum variants and fields. Splitting too early could add navigation overhead.

Best fix shape: defer large movement. When the next DTO-heavy feature lands, split by domain only if it reduces reasons-to-change, for example `privacy_report`, `release_readiness`, and `workflow_params` modules.

### 6. CI/release workflow shell blocks are powerful but hard to maintain

Status update: partially completed after this assessment. The CI prebuilt-frontend contract and Linux release checksum/APT installer staging are now delegated to scripts with local checks. macOS signing/notarization remains visible in YAML unless it becomes a recurring review hotspot.

Severity: medium.

Evidence:

- CI embeds a long prebuilt frontend contract shell block in `.github/workflows/ci.yml:162-201`.
- Release workflow contains large signing/staging/upload sections, and the gates/scripts slice found release-side complexity around macOS/Linux artifact handling.
- Root aliases duplicate exact commands for convenience, including `dev`/`tauri:dev` in `package.json:8` and `package.json:20`, plus Linux distribution aliases in `package.json:27-29`.

Why it matters: long inline shell in workflows is harder to test locally and easier to break with YAML quoting or environment changes. Alias duplication is low cost, but undocumented aliases make it unclear which command is canonical.

Refutation checked: some duplication is intentional because local, CI, scheduled, and release contexts have different strictness. For example, cargo audit/machete wrappers skip locally when tools are absent but become required in CI via `scripts/cargo-audit.mjs:5-15` and `scripts/cargo-machete.mjs:5-15`.

Best fix shape: move long workflow shell contracts into scripts that CI calls, keep aliases only when they serve a named user or CI purpose, and document canonical commands.

### 7. Contract checking is useful but shallow

Status update: improved after this assessment. The checker now includes nested detection/privacy DTO surfaces while remaining lightweight; type/default-aware generation remains deferred until DTO churn proves the need.

Severity: medium.

Evidence:

- `scripts/check-contracts.mjs:10-69` compares selected Rust enum variants and struct fields to TypeScript union/interface names.
- The parser is regex-based in `scripts/check-contracts.mjs:71-140`.

Why it matters: this catches missing or extra names, but it does not prove full type compatibility, optionality semantics, nested shape, serde casing behavior, or runtime serialization.

Refutation checked: for the current DTO style, this lightweight check is valuable and cheap. It is not a reason to add a heavy schema generator immediately.

Best fix shape: if DTO churn continues, consider generated bindings or schema-based validation. Until then, keep this gate and add targeted serialization tests for high-risk contract changes.

### 8. Docs-only changes do not appear to trigger the main CI gates

Status update: completed after this assessment. Docs-only changes now have a lightweight `npm run docs:check` path in CI.

Severity: low/medium.

Evidence:

- CI path filters include workflows, Cargo, crates, Tauri, frontend, tests, build icons/assets, scripts, README, LICENSE, CHANGELOG, and package metadata, but not `docs/**` in `.github/workflows/ci.yml:36-68`.
- Code/package jobs run only when those filter outputs are true in `.github/workflows/ci.yml:69-72` and `.github/workflows/ci.yml:218-224`.

Why it matters: docs-only PRs may skip everything except change detection. That is acceptable for cost control, but it means docs freshness, links, and command examples rely on human review.

Best fix shape: add a lightweight docs gate only if docs churn increases. A simple docs/link/example-command check is enough; do not run full Rust/frontend CI for every docs-only change unless needed.

## Things I Would Not Do Next

- Do not launch a broad refactor of the Rust core. The current shape is explicit, well-tested, and safety-oriented.
- Do not abstract all workflows into a generic framework. CSV, paste, and quick workflows are related but not identical.
- Do not remove defensive path, CSV, Local AI, or release guards to reduce line count. Most of that complexity is justified by privacy, data-loss, or packaging risk.
- Do not replace the existing gates with a single mega-command. Separate local, CI, release, and scheduled checks are useful as long as their purpose is clear.

## Best Next Steps

1. Keep broad DTO module splitting deferred until real DTO churn creates a stronger reason to split by domain.
2. Keep macOS release signing/notarization YAML visible unless it starts consuming repeated review time.
3. Consider type/default-aware contract validation only if regex name/field checks plus targeted serialization tests become insufficient.
4. Manually update or link the GitHub wiki cleanup hotspot page to `docs/cleanup-phased-plan-2026-07-01.md` so it does not rediscover completed local work.
5. Use the full quality gate before releases: `npm run fmt`, `npm run lint`, `npm run test`, `npm run typecheck`, `npm run deadcode:required`, `npm run release:check`, `npm run frontend:e2e`, `npm run frontend:a11y`, `npm run frontend:audit`, and `npm run cargo:audit:required`.

## Bottom Line

This project is already in good shape. The code quality bar is materially above average for a cross-platform desktop app: typed Rust core, explicit React/TypeScript contracts, broad CI, accessibility/e2e coverage, dead-code checks, supply-chain audits, release metadata validation, and previous review findings closed out.

To reach a very high bar, focus less on adding new process and more on keeping growth boring: consolidate tiny policy duplication, avoid speculative frontend branches, share only stable test fixtures, move fragile workflow shell into scripts, and strengthen contract validation only where DTO churn proves the need.
