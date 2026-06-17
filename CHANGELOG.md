# Changelog

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
