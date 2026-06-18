# CSV Data Anonymizer

Tauri desktop application for anonymizing CSV data locally while preserving file structure and useful formats. The app keeps CSV processing local in Rust and ships a bundled Vite frontend for the desktop UI.

## Features

- Auto-detects column types such as email, UUID, timestamp, numeric ID, phone, and country codes.
- Classifies PII risk and auto-selects high/medium risk columns.
- Previews sample transformations before writing output.
- Streams CSV processing so large files do not need to be loaded fully into memory.
- Supports deterministic anonymization with a persisted seed in native app settings.
- Uses native desktop file pickers, remembers recent folders, and opens completed output in Finder/Explorer.

## Development

Install Rust stable, Node.js 22.13 or newer, and the frontend dependencies:

```bash
npm ci --prefix frontend
```

Run the active Tauri desktop app:

```bash
npm run tauri:dev
```

Useful commands:

```bash
npm run frontend:build
npm run frontend:typecheck
npm run frontend:audit
cargo fmt --all --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd src-tauri && cargo tauri build
node scripts/rust-smoke.mjs
node scripts/check-release-metadata.mjs
```

The root `package.json` exposes wrappers for the common frontend, Tauri, Rust validation, and packaging commands.

## Architecture

- `crates/csv-anonymizer-core` contains the CSV detection, metadata, preview, transformation, and file processing engine.
- `src-tauri` contains the active Tauri desktop shell and Rust commands used by the bundled app.
- `frontend` contains the Vite UI bundled into macOS, `.deb`, `.rpm`, and AppImage releases.
- `crates/csv-anonymizer-app` remains as a lightweight CLI and smoke-test harness for the shared Rust core.
- `tests/fixtures` contains CSV fixtures shared by Rust tests and smoke checks.
- `build/linux` and `build/macos` contain package metadata, icons, and signing assets.
- `scripts` contains release packaging, APT repository, installer validation, and metadata checks.

App settings are stored as versioned JSON in the platform user config directory.

## Anonymization Strategies

| Data Type | Strategy | Format Preservation |
|-----------|----------|---------------------|
| Email | Fake local part | Domain preserved |
| UUID | Deterministic hash | Valid UUID v4 format |
| Timestamp | Date offset | Precision preserved |
| Numeric ID | Random/hash | Exact digit count preserved |
| Phone | Generic replacement | Approximate format |
| Country Code | Pass-through | Unchanged |
| Enum | Pass-through | Unchanged |

## Packaging

```bash
npm run frontend:build
CSV_ANONYMIZER_USE_PREBUILT_FRONTEND=1 scripts/build_frontend_for_tauri.sh
(cd src-tauri && cargo tauri build --bundles app)
node scripts/package-tauri-linux.mjs
```

Release packaging stages uploadable artifacts in `dist/rust/artifacts/`. Direct Tauri builds write platform bundles under `target/release/bundle/`.

On Linux, the packaging script creates `.deb`, `.rpm`, and AppImage installers from the Tauri desktop app.

Release steps, Linux package signing, APT publishing, and macOS notarization prerequisites are documented in `docs/releasing.md`.

## Install From Releases

Desktop builds are published on [GitHub Releases](https://github.com/ddv1982/csv-data-anonymizer/releases).

### macOS

Download the `.dmg` for your Mac from the latest release. Use the `aarch64` build for Apple Silicon Macs and the `x64` build for Intel Macs, then drag CSV Anonymizer into Applications.

### Linux

Linux releases publish `.deb`, `.rpm`, AppImage, a signed APT repository, the APT repository setup `.deb`, its signed checksum, and `install-apt-repo.sh`.

Debian/Ubuntu users can enable the signed APT repository once:

```bash
bash <(curl -fsSL https://ddv1982.github.io/csv-data-anonymizer/install-apt-repo.sh)
```

The setup script downloads the repository setup package to a temporary file, verifies it against a GPG-signed SHA256 sidecar from GitHub Pages, installs the CSV Anonymizer archive keyring and APT source configuration, then removes the temporary files.

Refresh APT metadata:

```bash
sudo apt update
```

Install CSV Anonymizer:

```bash
sudo apt install csv-anonymizer
```

After the repository is enabled, normal `sudo apt update` and `sudo apt upgrade` runs handle package-manager updates. The standalone `.AppImage`, `.deb`, and `.rpm` release assets remain available as direct-download fallback options.
