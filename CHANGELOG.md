# Changelog

## v1.0.72 - 2026-07-06

- Show every privacy-report readiness item and column decision so review blockers and later columns are not hidden behind compact truncation.
- Keep Local AI model downloads tied to the active job, disable model changes while a download is running, and preserve the cancel affordance during download state changes.
- Reject duplicate Smart replacement outputs across Local AI provider chunks, recompute privacy evidence and risk for type overrides, and gate releases on matching `Cargo.lock` workspace package versions.

## v1.0.71 - 2026-07-06

- Make column detection value-first for checksum-backed national IDs, VAT IDs, phones, postal codes, and street addresses so sensitive values are detected even when headers are missing, generic, or localized.
- Add locale inference, per-country postal formats, broader phone-region parsing, idsmith-backed national-ID validation, and a per-locale fixture matrix while keeping name detection header-gated after withdrawing bundled name datasets.
- Fix address keyword matching so embedded substrings in product/model codes such as `AVIA TURBO 1000` no longer trigger high-risk address detection, and archive the completed value-first implementation plan.

## v1.0.70 - 2026-07-02

- Supersede the failed v1.0.69 tag without moving it, keeping the Local AI topbar and `quick-xml` audit hardening while restoring release builds to the published Tauri CLI 2.11.4 line.
- Preserve the scoped Tauri/plist RustSec audit exception and the app-owned `quick-xml 0.41.0` parser updates from v1.0.69.

## v1.0.69 - 2026-07-02

- Supersede the failed v1.0.68 tag without moving it, keeping the Local AI topbar release while upgrading app-owned `quick-xml` paths and scoping the remaining Tauri/plist RustSec audit exception.
- Preserve the global Local AI opt-in, readiness pill, shared settings modal, blocked-state shortcut coverage, and browser accessibility checks from v1.0.68.

## v1.0.68 - 2026-07-02

- Move the Local AI opt-in, readiness pill, and settings entry point into the global top bar so every input mode can reach setup without hunting through workflow-specific panels.
- Keep Smart replacement blocked-state alerts focused on the selected field while routing their settings shortcut to the shared Local AI modal, with React and Playwright coverage updated for the new button location.

## v1.0.67 - 2026-07-02

- Supersede the failed v1.0.66 draft without moving the tag, keeping the release hardening while passing the public Linux signing fingerprint explicitly to the APT installer staging steps after temporary GPG material is cleaned up.
- Preserve the safer release shape: tag builds queue instead of canceling, signing inputs stay scoped to first-party shell steps, and macOS/Linux assets upload through explicit `gh release upload` lists.

## v1.0.66 - 2026-07-02

- Harden the release workflow follow-up by queueing tag builds instead of canceling in-progress releases, keeping macOS identity values scoped to the signing step, and removing the remaining third-party GitHub release upload action from platform build jobs.
- Build and sign the Linux APT repository/checksums inside one first-party shell step, clean temporary GPG material before Pages actions run, and upload release assets with explicit `gh release upload` asset lists that fail on empty outputs.
- Verify the Fable 5 CI findings with the root local gate, release metadata checks, Linux release helper dry-runs, workflow YAML parsing, and CI/release invariant assertions.

## v1.0.65 - 2026-07-02

- Make Email, Timestamp, and Phone transforms total: values that do not match the detected column format now become generic pseudonyms instead of passing through unchanged, with a Format fallbacks counter surfaced in the privacy report, readiness review items, and notes; fix a crash on multibyte timestamp values; align preview and final-run type detection on one sample basis; and trim cell values consistently between detection and processing.
- Protect original data in the shell: the output path can no longer equal the input file, analyze no longer grants write access silently, the Local AI runtime probe runs off the main thread, unused dialog permissions and a stale CSP source are removed, plain negative numbers are no longer altered by formula neutralization, and email-domain preservation is disclosed in report notes.
- Keep frontend workflows consistent and resilient: tab switches preserve paste/quick state, Select Detected Risk gating matches across CSV and paste, paste auto-selection uses the shared core policy, and download/job polling recovers from failures with new failure-path test coverage.
- Harden the release pipeline: actions pinned by commit SHA, signing secrets scoped to their steps, APT publishing gated on all platform builds, shared CI/release validation via a composite action, and a serde casing assertion in the contract checker.
- Simplify without behavior change: remove dead core APIs, the unused sensitive-columns metric, and dead frontend plumbing; consolidate shell helpers and the default Local AI model constant; share Debian/AppStream parsing helpers; delete legacy macOS packaging; and make local quality gates build the frontend once.

## v1.0.64 - 2026-07-01

- Refactor quick-value generation and backend preflight readiness into smaller helpers while preserving detector behavior, Smart replacement behavior, output-path validation, and readiness wording.
- Move Linux release checksum signing and APT installer staging into testable helper scripts while keeping signing inputs, workflow permissions, upload wiring, and release gates explicit.
- Expand Rust-to-TypeScript contract coverage for nested detection/privacy DTOs, share Local AI frontend test fixtures, and refresh cleanup docs so completed phases, deferred work, and manual wiki freshness are clear.

## v1.0.63 - 2026-07-01

- Consolidate auto-selection policy into the Rust core so CLI and Tauri use the same high/medium-risk selection rule.
- Simplify frontend column selection, share the CSV/Paste selection panel and Local AI setup alert, and centralize frontend DTO test builders.
- Add DTO serialization coverage, docs command validation, a reusable Tauri prebuilt frontend check, and docs-only CI coverage for release instructions.

## v1.0.62 - 2026-07-01

- Refresh the stable dependency line with current Tauri 2, Vite, ESLint, Knip, TypeScript ESLint, `open`, `unicode-segmentation`, and Criterion updates while keeping Tauri 3 and GTK4/WebKitGTK 6 migration work out of this release.
- Clear frontend outdated drift by moving to `lucide-react` 1.x and Node 26 typings, then align CI, release, and scheduled dependency scans on Node 26.
- Add scheduled RustSec audit coverage to the maintenance workflow and document the remaining transitive GTK3/GLib, `unic-*`, and `atomic-polyfill` warnings with their upstream paths.

## v1.0.61 - 2026-07-01

- Preserve the Rust PII library benchmark conclusion in docs while removing the dev-only external detector comparison dependencies and keeping the production detector local, deterministic, and table-aware.
- Keep the detector module split, explicit candidate/decision scoring, multilingual fixture matrix, and internal detector quality/performance gate so multilingual VAT/BTW and sensitive-field behavior stays measurable without dead benchmark weight.
- Polish review-table alignment and simplify the Privacy Report into a compact overview with collapsible details, shared report components, and frontend regression coverage.

## v1.0.60 - 2026-06-30

- Remove repeatable replacements and private seed settings end-to-end so rule-based pseudonyms are randomized per run without storing or accepting a seed.
- Preserve in-run replacement reuse for repeated source values across CSV, pasted data, quick generation, Tokenize, and Smart replacement while simplifying CLI, Tauri, settings, and frontend contracts.
- Clarify preview behavior for randomized rule-based replacements and ensure timestamp pseudonymization never leaves a selected timestamp unchanged.

## v1.0.59 - 2026-06-30

- Replace literal English-only header detection with a Unicode-normalized taxonomy that covers multilingual contact, address, date, name, postal code, account, and tax/VAT terms with conservative fuzzy matching.
- Back sensitive value detection with focused validators for email, URL, phone, payment cards, IBAN, VAT, Dutch BTW/omzetbelastingnummer, US SSN, and EIN while rejecting shape-only false positives.
- Split detection into validator, span, privacy evidence, taxonomy, and focused test modules, document dependency-audit follow-ups, and reduce the remaining detection-specific complexity warnings.

## v1.0.58 - 2026-06-30

- Supersede v1.0.57 without moving the failed tag, keeping the frontend type coverage, DataType policy, Tauri command registration, and automation helper cleanup.
- Add a direct Node 24 type dependency for frontend E2E/config typechecking so clean CI installs can build without relying on transitive types.
- Ship the v1.0.57 cleanup release with a clean-install frontend build, release metadata validation, and root quality gates passing.

## v1.0.57 - 2026-06-30

- Add frontend type coverage for E2E/config files and share CSV/Paste column-selection controls so fixture and selection drift are caught earlier.
- Centralize Rust DataType policy and Tauri command registration so report categories, redaction defaults, quick-generation support, permissions, and invoke handlers stay in sync.
- Align automation and docs around canonical root gates with shared script helpers, desktop artifact matching, and reusable macOS notarization retries.

## v1.0.56 - 2026-06-30

- Reduce the product surface to CSV File, Paste Sample, and Quick by Data Type by removing formal tabular, DP aggregate, synthetic data, column roles, DP budget ledger, and permanent release-readiness UI.
- Simplify Review Sensitive Columns around detected risk, replacement methods, compact help, and focused warnings while keeping Quick by Data Type first-class.
- Default medium/high-risk CSV and pasted fields to Redact and remove remembered seed/keychain storage so repeatable seeds stay session-only.

## v1.0.55 - 2026-06-30

- Add Redact as a first-class manual strategy for CSV and pasted-data columns, with typed placeholders, privacy-report accounting, and rendered dropdown coverage.
- Default high and medium-risk direct JSON/YAML/XML/text/log fields to Redact while preserving CSV Auto defaults and explicit user overrides.
- Tighten sensitive-field detection for phones, usernames, postal codes, private dates, and user-linked timestamps, and scope scalar type-change warnings to structured JSON/YAML output.

## v1.0.54 - 2026-06-29

- Gate direct Paste Data and Quick by Data Type actions on loaded settings, and keep CSV output creation disabled while selected Smart replacement columns still need Local AI setup.
- Harden backend privacy and path boundaries with bounded Local AI download reads, surfaced seed-vault deletion failures, safe output suffix validation, and clearer deterministic CLI seed help.
- Add direct Linux installer provenance by signing checksum sidecars for `.deb`, `.rpm`, and AppImage downloads, publishing the archive keyring, and documenting verification steps.

## v1.0.53 - 2026-06-29

- Add local privacy detector evidence for contact values, secrets, account and government identifiers, network/device identifiers, URLs, and private-date cues across CSV and direct text/log review paths.
- Add Detector Review with Balanced/Strict controls, column evidence chips, highlighted sample spans, and placeholder-redacted sample output so users can audit why a column was flagged.
- Harden detector contracts so embedded span evidence no longer changes transform types, redaction offsets are UTF-16 safe for the frontend, low-confidence date cues stay out of default risk, and evidence counts mean matched sampled rows.

## v1.0.52 - 2026-06-29

- Supersede v1.0.51 without moving the failed tag, keeping the Synthetic generator and release-mode UX changes.
- Fix the Playwright Synthetic data workflow assertion so CI and release validation expect the new table-linked explanatory copy.
- Ship the v1.0.51 Synthetic improvements with passing e2e coverage for the locked all-column release mode.

## v1.0.51 - 2026-06-29

- Improve Synthetic data generation so numeric, timestamp, MAC address, and placeholder values are keyed by column identity instead of sharing row-level synthetic values across columns.
- Move Privacy Release selection next to the column table, lock Synthetic data into its all-column dataset behavior, and keep Type Override plus Role controls available for generated values.
- Expand Synthetic data help and regression coverage around determinism, column independence, disabled preview behavior, stale Smart replacement strategies, and structured placeholder formats.

## v1.0.50 - 2026-06-29

- Make Synthetic data a self-correcting dataset-level release mode by selecting every CSV column, locking partial selection controls, and disabling the misleading row-level preview path.
- Normalize Synthetic release controls so stale Smart replacement strategies no longer require Local AI, while preserving Type overrides and Role settings for generated output.
- Expand Privacy Release help and regression coverage so users can see that Synthetic output includes every column, ignores row-level Strategy, and is deterministic for the same schema, settings, row count, roles, types, and seed.

## v1.0.49 - 2026-06-29

- Supersede v1.0.48 without moving the failed tag, keeping the backend preflight, Smart rejection telemetry, and private seed vaulting changes.
- Fix the Playwright workflow mock for backend preflight so Linux E2E preview, paste, and cancel flows exercise the new command path correctly.
- Ignore Playwright result directories in frontend linting so local verification remains stable when browser tests have just run.

## v1.0.48 - 2026-06-29

- Add backend-owned release preflight checks with Tauri command wiring, generated permissions, DP budget state injection, and frontend readiness integration for preview and anonymize flows.
- Expand Smart replacement audit evidence with validated preview reuse plus Local AI rejection reason counts for empty, copied, leaking, duplicate, and malformed candidates.
- Move remembered repeatable seeds out of settings JSON into the local OS credential store with migration, clear/save behavior, and private seed vault documentation.

## v1.0.47 - 2026-06-29

- Harden privacy safeguards by warning on selected direct-input columns, rejecting blank deterministic seeds across CLI/core/direct-input paths, and requiring all columns in formal tabular releases.
- Make Tauri background work fail terminally on panic, bound Local AI model download time, and keep failed anonymize/download jobs observable through terminal status snapshots.
- Improve frontend privacy and state correctness with typed DP budget reset confirmation, paste/quick privacy reports, model-specific Local AI readiness, and settings-load guards that prevent stale defaults from overwriting saved preferences.

## v1.0.46 - 2026-06-26

- Make dead-code gates explicit with a root strict Knip scan and intentional system-binary ignores while preserving the existing required Rust and frontend checks.
- Extract shared core and domain helpers for cancellation/progress checks, atomic CSV writes, preview sampling, privacy-report notes, UUID generation, XML text samples, and header-term detection.
- Share Paste and Quick copy-result handling through a frontend hook and centralize Tauri job lifecycle storage for IDs, snapshots, cancelation flags, removal, and terminal TTL pruning.

## v1.0.45 - 2026-06-26

- Reuse Paste Data preview Smart replacement entries when anonymizing pasted input so preview-covered Local AI values are confirmed instead of generated again.
- Share the same smart-replacement merge path used by CSV file output across pasted CSV, JSON, YAML, XML, plain-text, and log transforms, generating only values missing from the preview.
- Extend backend and frontend regression coverage for preview reuse, missing-value generation, and Paste Data Tauri argument forwarding.

## v1.0.44 - 2026-06-26

- Move Local AI controls into the global top bar so the opt-in toggle, readiness status, and settings entry point are available from every input mode.
- Add an accessible Local LLM settings modal that reuses the existing Local AI panel for model selection, status refresh, downloads, cancelation, and Ollama setup.
- Remove duplicated Local AI settings panels from CSV configuration, Paste Data, and Quick by Data Type while keeping Smart replacement blocked-state alerts with a direct settings shortcut.
- Share the modal focus-trap and Escape/backdrop behavior with help dialogs so Local AI settings restore focus cleanly after close.
- Extend React, Playwright, and accessibility coverage for the global Local AI controls, settings modal, blocked Smart replacement flows, keyboard focus, and cross-tab availability.

## v1.0.43 - 2026-06-26

- Extend optional Local AI Smart replacement into Paste Data previews, pasted-data transforms, and Quick by Data Type generation while keeping the CSV file workflow unchanged.
- Reuse the core smart-replacement provider boundary across CSV, pasted CSV/JSON/YAML/XML/text/log inputs, and quick generated values so Ollama setup remains isolated to the Tauri layer.
- Add contextual Local AI setup panels in Paste and Quick only when Smart replacement is selected, with shared panel wiring to avoid duplicated settings code.
- Add regression coverage for direct-input smart replacement, quick Local AI generation, and Paste/Quick Local AI UI gating.
- Apply document theme state before first paint so automated accessibility checks do not see mismatched light-theme metadata and dark first-paint controls.

## v1.0.42 - 2026-06-26

- Rebuild light mode around expanded semantic tokens for app surfaces, raised panels, borders, focus rings, status colors, and data-selection states.
- Separate visual roles so primary navigation stays blue, selected data rows use a privacy-oriented teal treatment, and risk/status feedback uses distinct red, amber, and green containers.
- Refine cards, tables, inputs, preview frames, popovers, progress bars, alerts, disabled sections, and mobile column cards for clearer hierarchy and less washed-out gray-blue repetition.
- Preserve accessibility with stronger input borders, visible selected-row accent stripes, full-opacity light disabled sections, and passing automated accessibility checks.

## v1.0.41 - 2026-06-26

- Add direct pasted-data workflows for CSV text, JSON, YAML, XML, logs, documents, and quick generated sample values while keeping the existing file CSV path intact.
- Split direct-input parsing into focused Rust modules with typed source-path IDs, escaped display labels, bounded payload analysis, and regression coverage for dotted keys, XML attributes, scalar types, and overlapping text tokens.
- Harden paste commands with explicit Tauri permissions, frontend/backend contract checks, generated command DTO validation, and CI dead-code scanning.
- Add paste performance benchmarks plus Playwright keyboard-focus and automated accessibility checks for the input tabs and privacy dialogs.
- Extend spreadsheet formula-injection protection and privacy wording so CSV releases handle Unicode formula prefixes and avoid overstating anonymization guarantees.

## v1.0.40 - 2026-06-25

- Complete the quality-plan rollout with required Rust audits, frontend lint/unit/e2e guardrails, dead-code scans, and CI/release workflow coverage.
- Split the frontend workflow and Local AI backend into focused modules while preserving the existing user workflow and Tauri command surface.
- Harden large-file privacy releases with memory/cardinality warnings, input caps, terminal job/download pruning, and conservative privacy messaging.
- Add browser workflow coverage for file loading, privacy validation, glossary/help behavior, preview recovery, job polling, cancellation, and settings persistence.
- Refresh release documentation and metadata so the full validation order, required tools, and maintenance checks match the shipped build.

## v1.0.39 - 2026-06-25

- Supersede v1.0.38 without moving the failed tag, replacing the RustSec GitHub Action with the repository `cargo:audit:required` gate so CI and release validation do not require Checks API write access.
- Keep the v1.0.38 quality hardening: frontend lint and unit-test guardrails, explicit Tauri app-command capabilities, bounded background job retention, privacy release input caps, Local AI batch limits, and the `quinn-proto` security update.
- Preserve the release notes, documentation, and obsolete Linux packaging script cleanup from v1.0.38 while rerunning the full release workflow on a fresh tag.

## v1.0.38 - 2026-06-25

- Add frontend linting and unit-test guardrails, wire them into local and GitHub release validation, and cover settings, glossary, and path utilities.
- Tighten Tauri command permissions with generated app-command capabilities and add bounded retention for anonymization and Local AI download jobs.
- Harden privacy and Smart replacement paths by capping large in-memory privacy releases, limiting Local AI unique-value batches, and updating the vulnerable `quinn-proto` dependency.
- Simplify the frontend workflow by extracting persistent settings and app settings UI, while keeping app settings usable before a CSV is loaded.
- Remove obsolete Linux packaging scripts and refresh release documentation around the current validation gates.

## v1.0.37 - 2026-06-23

- Fail Local AI model downloads before creating a job when Ollama is not running, matching the setup panel's friendly install/start guidance.
- Reuse the Ollama runtime availability check between status refresh and download preflight so stale UI state does not surface low-level connection errors.
- Add regression coverage for unavailable-Ollama status and download preflight handling.

## v1.0.36 - 2026-06-23

- Harden CSV output safety by rejecting non-empty ragged rows, padding short rows consistently, and neutralizing spreadsheet formula payloads in headers and released data.
- Tighten privacy release behavior with safer synthetic attribute generation, stricter DP grouped-output validation, protected DP budget reset confirmation, and sanitized frontend error messages.
- Strengthen release validation with current-version artifact checks, privacy-focused Rust smoke coverage, and an optional `cargo:audit` release gate.

## v1.0.35 - 2026-06-23

- Refresh light mode with a cooler neutral surface stack, stronger text and border contrast, and more defined card, input, button, and focus affordances.
- Add clearer table hierarchy with stronger header, hover, and selected-row states for checked columns.
- Tune light-mode panel, alert, status, and disabled-section treatments while keeping the existing dark theme scoped to its original token stack.

## v1.0.34 - 2026-06-23

- Replace privacy-report metric glossary highlights with compact help icons so result cards stay readable while keeping term popovers available.
- Add glossary popovers for Pseudonymize, Mask, Tokenize, and Pass through in the Select Data help article.
- Rewrite Standard CSV transform result notes so formal privacy release guidance is mode-specific and pseudonym/token mapping notes appear only when relevant.

## v1.0.33 - 2026-06-23

- Tighten the Local AI panel layout so the installed-model badge stays attached to the model field and action controls wrap without horizontal overflow.
- Add glossary-linked help article text for Local AI and privacy concepts such as Quasi-ID, Direct ID, Sensitive, k-anonymity, l-diversity, t-closeness, model, localhost, and fallback.
- Keep glossary popovers usable inside help modals by layering them above the modal and making Escape close the innermost popover before closing the article.

## v1.0.32 - 2026-06-22

- Add a persistent Theme mode selector with System, Light, and Dark choices in the top bar.
- Follow the operating system appearance in System mode while syncing the Tauri native app theme and browser color scheme.
- Add light-mode semantic tokens for cards, controls, risk badges, alerts, popovers, and shadows while preserving the existing dark theme.

## v1.0.31 - 2026-06-22

- Tighten the privacy-release workflow around practical local use with clearer DP aggregate validation, local epsilon budget history, grouped-output rules, synthetic-data limits, and report notes.
- Split the privacy release backend, Tauri settings ledger, frontend privacy settings panel, and privacy release tests into smaller focused modules for easier maintenance.
- Refresh in-app explanations and glossary popovers for DP aggregate caveats, formal k/l/t settings, synthetic row-count limits, and privacy report terminology.

## v1.0.30 - 2026-06-22

- Replace busy per-section help pills with scoped help-article modals for column settings, Local AI, Privacy Release, and Privacy Report explanations.
- Turn glossary labels into subtle highlighted terms while keeping direct help icons only for high-consequence or ambiguous controls such as model, suppression, epsilon, and DP bounds.
- Reduce column-table help clutter on desktop and mobile with a single table-level "How does this work?" entry, plain mobile row labels, modal focus handling, Escape close, and responsive verification.

## v1.0.29 - 2026-06-22

- Add section-level "How does this work?" help popovers across the workflow, Local AI, Privacy Release, App Settings, Preview, and Privacy Report areas.
- Expand end-user explanations for Strategy, Role, release modes, masking, DP aggregate budget tracking, synthetic-data limitations, and sensitive-role privacy checks.
- Improve help-popover accessibility and responsive behavior with reusable dialog-style section help, viewport-bounded scrolling, keyboard close/focus return, and usable help inside disabled workflow sections.

## v1.0.28 - 2026-06-22

- Add glossary popovers for specialist privacy, anonymization, Local AI, and privacy-report terms so users can understand release modes, roles, strategies, metrics, and DP caveats in context.
- Add privacy-release helper copy and validate glossary definitions against the Rust release/report semantics, including the current synthetic-data DP limitation.
- Improve responsive usability with stacked mobile column controls, contained desktop table sizing, wrapped preview warnings and values, and mobile-safe privacy report cards.

## v1.0.27 - 2026-06-22

- Add opt-in formal tabular privacy releases that redact direct identifiers, generalize quasi-identifiers, report k-anonymity, and optionally evaluate l-diversity and t-closeness with small-class suppression.
- Add differentially private aggregate and synthetic data release modes with UI configuration, Tauri job support, backend validation, and enriched privacy report fields.
- Split the larger privacy, strategy, service-test, and frontend workflow modules into focused files so the new privacy release surface is easier to maintain.

## v1.0.26 - 2026-06-22

- Show app errors as a fixed dismissible toast so output-file conflicts and other failures remain visible without scrolling back to the top of the workflow.
- Keep existing error handling and overwrite behavior unchanged while improving long-message wrapping and mobile toast layout.

## v1.0.25 - 2026-06-22

- Fix the background Anonymize File workflow so successful jobs write the output CSV and reach the result page again.
- Defer Local AI provider setup until Smart replacement generation is actually needed, allowing fully preview-covered Smart replacement jobs to complete without a second Local AI readiness check.
- Add background-job regression coverage for standard anonymization with and without preview data, Smart preview-covered output writes, and clear failure when Local AI generation is required but unavailable.

## v1.0.24 - 2026-06-22

- Reuse Smart replacement values generated during preview when writing the final anonymized CSV so reviewed preview values do not require a second Local AI generation pass.
- Keep final output aligned with previewed Smart replacement samples while still generating replacements for values that were not covered by the preview sample.
- Add regression coverage for preview replacement reuse and missing-value generation in the Rust anonymization core.

## v1.0.23 - 2026-06-22

- Show locally installed Ollama models in the Local AI panel so users can select models already available on their machine.
- Keep `gemma3:4b` as the recommended lightweight default while hiding the model download action when the selected model is already installed.
- Make Local AI setup and download messages reflect the selected model instead of always referring to Gemma.

## v1.0.22 - 2026-06-19

- Rewrite the README into a shorter user-facing guide with clearer install, workflow, privacy-boundary, and development sections.
- Add concise Local LLM usage documentation for the optional Ollama and Gemma 3 4B Smart replacement flow, including setup, preview, fallback, and data locality notes.

## v1.0.21 - 2026-06-19

- Add optional Local AI smart replacement for selected columns through Ollama and Gemma 3 4B, including status checks, model download controls, and validated replacement maps with rule-based fallbacks.
- Rename the end-user deterministic setting to Repeatable replacements and clarify that the seed is private and useful for matching replacements across files.
- Extend privacy reporting with smart replacement columns, generated smart values, and fallback counts while preserving the existing local-first anonymization warnings.
- Keep model weights and local runtime binaries out of source and release metadata, with checks that block tracked `.gguf`, model-cache, Ollama-cache, and `llama-server` artifacts.

## v1.0.20 - 2026-06-19

- Add run-scoped pseudonym mapping so repeated source values reuse the same replacement and distinct readable names avoid duplicate assignments while pool capacity remains.
- Expand readable first/last-name pools, preserve first/last/full-name consistency through shared semantic domains, and report avoided candidate collisions plus pool-exhaustion fallbacks.
- Add a Tokenize strategy for opaque `tok_...` values and upgrade deterministic pseudonyms to keyed HMAC-SHA256 with seed-sensitivity notes in the privacy report.
- Improve the privacy report UI and serialized report fields with unique pseudonym counts, repeated-source reuses, opaque-token counts, avoided collisions, and exhausted pseudonym pools.

## v1.0.19 - 2026-06-19

- Improve name pseudonymization so generic `name` columns with single-token values use name-like replacements instead of generic alphanumeric strings.
- Make full-name replacements compose consistent first/last token pseudonyms and avoid preserving original full-name tokens where the local replacement pool allows it.
- Add a representative people-name CSV fixture and regression tests for name detection, preview output, token consistency, and replacement quality.

## v1.0.18 - 2026-06-19

- Improve anonymization quality with numeric-shape-preserving values, expanded sensitive type detection, type-specific phone/name strategies, per-column type and strategy controls, preview warnings, and a privacy report that distinguishes masking/pseudonymization from stronger anonymization guarantees.
- Fix Linux Debian package icon readiness by bundling standard hicolor PNG sizes and validating installed desktop `Icon=` resolution during release checks.

## v1.0.17 - 2026-06-18

- Add `esbuild` as an explicit frontend dev dependency so clean CI and release `npm ci` installs can run the Vite 8 production build.
- Supersede v1.0.16 without moving the failed tag, keeping the same background job, file access, cleanup, and CI hardening changes.

## v1.0.16 - 2026-06-18

- Add Tauri-managed background anonymization jobs with progress polling and cancel support so long CSV writes no longer block the UI.
- Gate CSV input/output access behind native picker grants or explicit path confirmations before Rust commands read, write, or reveal files.
- Improve CSV correctness with header-aware name detection, safer numeric ID versus phone classification, blank-row row count alignment, and sparse-column anonymization coverage.
- Split large frontend, Tauri command, core test, and stylesheet modules; retire the legacy egui desktop shell so the remaining Rust app crate is a lightweight CLI smoke harness.
- Harden CI and release validation with pinned Tauri CLI installs, frontend dependency audit, narrower release permissions, prebuilt frontend asset checks, and AppImage upload cleanup.

## v1.0.15 - 2026-06-18

- Make CSV opening feel immediate by sampling columns first and moving the exact row count into a follow-up background command.
- Move CSV analyze, preview, row-count, and anonymize work onto Tauri blocking tasks so synchronous file I/O does not tie up the async command runtime.
- Fix remembered file-picker directories so they seed the native dialogs without being shown as fake input or output file paths.
- Add clearer opening/loading button states and regression coverage for the sampled analysis path.

## v1.0.14 - 2026-06-18

- Supersede v1.0.13 with RPM metadata validation that can extract Tauri RPM payloads through `bsdtar` when `rpm2cpio` cannot.
- Install `libarchive-tools` in Linux packaging workflows so Debian/RPM metadata validation keeps inspecting package contents.
- Keep the v1.0.13 Linux package identity fix for `csv-anonymizer.desktop`.

## v1.0.13 - 2026-06-18

- Supersede v1.0.12 with Linux Tauri packages whose installed desktop file is `csv-anonymizer.desktop`.
- Keep the Linux package identity aligned with the AppStream launchable contract while preserving the visible app name as CSV Anonymizer.
- Add a Linux-specific Tauri configuration and desktop template so Debian/RPM metadata validation passes before APT repository publishing.

## v1.0.12 - 2026-06-18

- Ship the desktop app through a Tauri web frontend so Linux AppImage, Debian/RPM packages, and macOS builds share the same bundled UI styling.
- Add a React/Vite frontend for CSV selection, settings, detected columns, previews, and anonymization results.
- Add Tauri command wrappers over the Rust anonymization core while preserving the existing CLI smoke path.
- Update CI and release packaging to build `frontend/dist`, use Tauri/WebKitGTK dependencies on Linux, and package Tauri desktop artifacts.

## v1.0.11 - 2026-06-18

- Polish the native Rust desktop interface used by the Linux AppImage, Debian/RPM packages, tarball, and macOS app.
- Add app-level egui theme styling, structured sections, status chips, clearer column risk badges, and a stronger primary anonymization action.
- Keep the release packaging unchanged while making the shipped native UI look complete without relying on web CSS assets.

## v1.0.10 - 2026-06-18

- Rewrite the desktop app as a native Rust `eframe/egui` application with a shared Rust anonymization core.
- Replace Bun/Electrobun/Vue app builds with Rust CI, Rust smoke checks, and Rust release artifacts under `dist/rust`.
- Add Rust macOS `.app`/`.dmg` packaging and Linux `.deb`, `.rpm`, AppImage, portable tarball, and signed APT repository packaging.
- Persist native app settings, remembered file-picker directories, deterministic seed, overwrite behavior, sample counts, and output suffix.
- Keep Cmd+Q close handling in the native app and add output-folder reveal after successful anonymization.

## v1.0.9 - 2026-06-18

- Install the verified Linux APT repository setup package from an `_apt`-accessible staging directory to avoid Ubuntu's unsandboxed local package notice.
- Extend APT installer validation to prove the package path handed to `apt install` uses `0755` directory and `0644` file permissions.

## v1.0.8 - 2026-06-18

- Fix the published Linux APT installer so the pinned setup signing fingerprint is not cleared after release-time rendering.
- Add APT installer rendering validation to CI and release publishing so staged and GitHub Pages installers keep the effective pinned fingerprint.

## v1.0.7 - 2026-06-18

- Route the macOS Quit menu item through an explicit Cmd+Q accelerator and `Utils.quit()` handler.
- Publish the Linux APT installer and repository setup assets through GitHub Pages for unauthenticated installs.
- Update the Debian/Ubuntu install command to use the public GitHub Pages bootstrap URL.
- Replace unsigned APT repository validation with a temporary-key signed repository check.
- Reduce Linux CI signing noise by filtering irrelevant Electrobun macOS codesign/notarization skip messages and quieting trusted verification output.

## v1.0.6 - 2026-06-18

- Restore the native macOS application menu for packaged Electrobun builds.
- Fix first-run Browse dialogs by omitting unset native dialog paths before they cross the Electrobun bridge.
- Treat canceled or blank file-picker selections as normal no-op results instead of RPC failures.
- Publish the release `install-apt-repo.sh` asset with the pinned APT signing fingerprint.
- Publish the archive keyring in the APT Pages root so Linux installer verification can fetch the trusted key.
- Expand macOS and Linux installation notes with the signed APT repository flow and direct-download fallback artifacts.

## v1.0.5 - 2026-06-18

- Migrate the desktop runtime and packaging pipeline from Electron to Electrobun.
- Replace Electron IPC with the Electrobun RPC bridge while preserving anonymization and settings behavior.
- Add Electrobun smoke and artifact validation scripts for local and CI release checks.
- Add Linux package-manager artifacts for Debian, RPM, AppImage, and signed APT repository publishing.
- Switch dependency installation and project scripts from pnpm to Bun with a committed `bun.lock`.
- Restore the app icon for macOS bundles, Linux packages, and the renderer favicon.
- Build a dev Electrobun app for release smoke checks while preserving stable artifact validation.
- Derive the macOS Developer ID signing identity from the imported certificate during release builds.
- Defer Windows release artifacts until Authenticode signing is configured.

## v1.0.4 - 2026-06-18

- Migrate the desktop runtime and packaging pipeline from Electron to Electrobun.
- Replace Electron IPC with the Electrobun RPC bridge while preserving anonymization and settings behavior.
- Add Electrobun smoke and artifact validation scripts for local and CI release checks.
- Add Linux package-manager artifacts for Debian, RPM, AppImage, and signed APT repository publishing.
- Derive the macOS Developer ID signing identity from the imported certificate during release builds.
- Defer Windows release artifacts until Authenticode signing is configured.

## v1.0.3 - 2026-06-18

- Migrate the desktop runtime and packaging pipeline from Electron to Electrobun.
- Replace Electron IPC with the Electrobun RPC bridge while preserving anonymization and settings behavior.
- Add Electrobun smoke and artifact validation scripts for local and CI release checks.
- Add Linux package-manager artifacts for Debian, RPM, AppImage, and signed APT repository publishing.
- Add Windows x64 release builds and update release documentation for the Electrobun artifact model.
- Derive the macOS Developer ID signing identity from the imported certificate during release builds.

## v1.0.2 - 2026-06-18

- Migrate the desktop runtime and packaging pipeline from Electron to Electrobun.
- Replace Electron IPC with the Electrobun RPC bridge while preserving anonymization and settings behavior.
- Add Electrobun smoke and artifact validation scripts for local and CI release checks.
- Add Linux package-manager artifacts for Debian, RPM, AppImage, and signed APT repository publishing.
- Add Windows x64 release builds and update release documentation for the Electrobun artifact model.

## v1.0.1 - 2026-06-17

- Add signed APT repository publishing to GitHub Pages, including a repository setup package and verified installer script.
- Add Linux package metadata validation for AppStream, desktop launcher, Debian copyright, and RPM license files.
- Document the Debian/Ubuntu repository install and update path plus maintainer release verification steps.

## v1.0.0 - 2026-06-17

- Initial Electron desktop release of CSV Anonymizer.
- Add GitHub Actions CI for release metadata, type-checking, unit/integration tests, production builds, and Electron e2e smoke tests.
- Add signed and notarized macOS release packaging for x64 and arm64 using Developer ID, hardened runtime, entitlements, and App Store Connect API notarization.
- Add Linux release packaging for AppImage, Debian, and RPM artifacts, with detached GPG signatures and release-time signature verification.
- Add reusable release metadata validation, generated Linux icon assets, and maintainer release documentation.
- Update CI runners/actions and package-manager tooling to current supported versions.
