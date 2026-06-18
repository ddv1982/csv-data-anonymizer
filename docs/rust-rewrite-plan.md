# Rust Rewrite Status

CSV Anonymizer now uses the Rust workspace as the active app and release path.

## Implemented Phases

### Core

- `crates/csv-anonymizer-core` owns detection, metadata, deterministic hashing, strategies, sample reading, preview, and streaming CSV anonymization.
- Rust tests cover fixture behavior, deterministic output, selected-column transforms, BOM handling, metadata, and output safety.

### Native Desktop

- `crates/csv-anonymizer-app` provides the native `eframe/egui` shell.
- The app supports input/output selection, manual paths, settings persistence, remembered folders, high/medium PII auto-selection, preview, overwrite handling, non-blocking anonymization, Cmd+Q close handling, and opening the output folder after success.
- The same binary exposes CLI entrypoints used by CI smoke checks.

### Linux Packaging

- `scripts/package-rust-linux.mjs` packages the Rust binary as a portable `.tar.gz`, `.deb`, `.rpm`, and AppImage.
- `scripts/build_apt_repository.py`, `scripts/install-apt-repo.sh`, `scripts/check-apt-repository.mjs`, and `scripts/check-apt-installer.mjs` remain the signed APT publishing path.
- Linux package metadata validation uses the existing Python validator against `dist/rust/artifacts`.

### macOS Packaging

- `scripts/package-rust-macos.mjs` creates `CSV Anonymizer.app`, `.app.tar.gz`, and `.dmg` artifacts from the Rust binary.
- GitHub Actions signs, notarizes, staples, and verifies macOS release artifacts.

### Removal

- The old Bun/Electrobun/Vue runtime and TypeScript test surface have been removed.
- CI and release workflows no longer install Bun or build webview artifacts for the app.
- Release artifacts are written under `dist/rust`.

## Active Verification

```bash
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node scripts/rust-smoke.mjs
node scripts/check-release-metadata.mjs
```

Platform package checks:

```bash
node scripts/package-rust-macos.mjs
node scripts/check-rust-artifacts.mjs --platform macos --require-dmg
```

```bash
node scripts/package-rust-linux.mjs
python3 scripts/validate_linux_package_metadata.py "dist/rust/artifacts/*.deb" "dist/rust/artifacts/*.rpm"
node scripts/check-apt-repository.mjs
node scripts/check-apt-installer.mjs
```
