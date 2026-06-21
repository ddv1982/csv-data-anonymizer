# Changelog

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
