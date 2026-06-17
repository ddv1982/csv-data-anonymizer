# Changelog

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
