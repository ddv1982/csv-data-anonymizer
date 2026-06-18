# Rust Rewrite Status

CSV Anonymizer now uses Rust for core logic and Tauri for the active desktop
app and release path. The production UI is the bundled Vite frontend.

## Implemented Phases

### Core

- `crates/csv-anonymizer-core` owns detection, metadata, deterministic hashing, strategies, sample reading, preview, and streaming CSV anonymization.
- Rust tests cover fixture behavior, deterministic output, selected-column transforms, BOM handling, metadata, and output safety.

### Desktop Runtime

- `src-tauri` provides the Tauri desktop shell.
- `frontend` provides the bundled Vite UI used by macOS, `.deb`, `.rpm`, and
  AppImage builds.
- `crates/csv-anonymizer-app` is retained as a lightweight CLI and smoke-test
  harness for the shared Rust core. Legacy native packaging scripts require
  `CSV_ANONYMIZER_ALLOW_LEGACY_NATIVE_PACKAGING=1` so they cannot be mistaken for
  production packaging.
- The desktop app supports input/output selection, manual paths, settings
  persistence, remembered folders, high/medium PII auto-selection, preview,
  overwrite handling, non-blocking anonymization, Cmd+Q close handling, and
  opening the output folder after success.

### Linux Packaging

- `scripts/package-tauri-linux.mjs` packages the Tauri desktop app as `.deb`, `.rpm`, and AppImage installers.
- `scripts/build_apt_repository.py`, `scripts/install-apt-repo.sh`, `scripts/check-apt-repository.mjs`, and `scripts/check-apt-installer.mjs` remain the signed APT publishing path.
- Linux package metadata validation uses the existing Python validator against `dist/rust/artifacts`.

### macOS Packaging

- The release workflow creates signed and notarized `.dmg` installers from the Tauri app bundle.
- GitHub Actions signs, notarizes, staples, and verifies macOS release artifacts.

### Removal

- The old Bun/Electrobun/Vue runtime and TypeScript test surface have been removed.
- CI and release workflows no longer install Bun. They build the Vite frontend
  once and require Tauri to consume that prebuilt `frontend/dist`.
- Release and package jobs install the Tauri CLI at the pinned workflow version
  matching the Tauri crate version resolved in `Cargo.lock`.
- Release artifacts are written under `dist/rust`.

## Active Verification

```bash
npm run frontend:build
npm run frontend:audit
CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 scripts/build_frontend_for_tauri.sh
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
node scripts/rust-smoke.mjs
node scripts/check-release-metadata.mjs
```

Platform package checks:

```bash
(cd src-tauri && CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 cargo tauri build --bundles dmg)
```

```bash
node scripts/package-tauri-linux.mjs
python3 scripts/validate_linux_package_metadata.py "dist/rust/artifacts/*.deb" "dist/rust/artifacts/*.rpm"
node scripts/check-apt-repository.mjs
node scripts/check-apt-installer.mjs
```
