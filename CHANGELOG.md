# Changelog

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
