# CSV Data Anonymizer

Desktop-only Electrobun application for anonymizing CSV data locally while preserving file structure and useful formats.

## Features

- Auto-detects column types such as email, UUID, timestamp, numeric ID, phone, and country codes.
- Classifies PII risk and auto-selects high/medium risk columns.
- Previews sample transformations before writing output.
- Streams CSV processing so large files do not need to be loaded fully into memory.
- Supports deterministic anonymization with a persisted seed in app settings.
- Uses native desktop file pickers and opens completed output in Finder/Explorer.

## Development

Install dependencies and run scripts with Bun.

```bash
bun install
bun run dev
```

Useful commands:

```bash
bun run build          # Type-check and build the Electrobun app
bun run test               # Run unit and integration tests
bun run test:coverage  # Run tests with coverage
bun run test:e2e       # Run the Electrobun smoke workflow
bun run dist           # Build stable Electrobun artifacts for the host platform
bun run release:check  # Validate package version and changelog metadata
```

## Architecture

- `src/bun` owns the Electrobun window, typed RPC handlers, native dialogs, shell actions, app settings, and filesystem access.
- `src/electrobun-view` exposes the typed `window.csvAnonymizer` bridge through Electrobun RPC.
- `src/services` contains runtime services shared by the Bun process and tests.
- `src/shared/contracts.ts` contains the Zod schemas, inferred TypeScript types, defaults, and renderer-facing API contract.
- `src/renderer` is the Vue renderer.
- `src/core`, `src/strategies`, `src/types`, and `src/utils` contain the reusable CSV anonymization engine.

App settings are stored as versioned JSON under Electrobun user data. YAML config files and the previous command-line interface have been removed.

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
bun run dist:dir
bun run dist
```

Electrobun artifacts are written to `dist/electrobun/artifacts/`.

On Linux, `bun run dist:linux` also creates `.deb`, `.rpm`, and AppImage artifacts from the Electrobun Linux output.

Release steps, Linux package signing, and macOS notarization prerequisites are documented in `docs/releasing.md`.

## Install From Releases

Desktop builds are published on [GitHub Releases](https://github.com/ddv1982/csv-data-anonymizer/releases).

### macOS

Download the `.dmg` for your Mac from the latest release. Use the `arm64` build for Apple Silicon Macs and the `x64` build for Intel Macs, then drag CSV Anonymizer into Applications.

### Linux

The active Electrobun release path publishes Linux `.tar.zst`, Electrobun setup `.tar.gz`, `.deb`, `.rpm`, AppImage, update metadata, APT repository setup `.deb`, `install-apt-repo.sh`, keyring, checksum sidecars, and detached signatures.

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

After the repository is enabled, normal `sudo apt update` and `sudo apt upgrade` runs handle package-manager updates. CSV Anonymizer can also be installed by searching for "CSV Anonymizer" in GNOME Software or Ubuntu Software after package/AppStream metadata has refreshed.

The standalone `.AppImage`, `.deb`, and `.rpm` release assets remain available as direct-download fallback options. Package-manager installs pull the declared GTK/WebKit/AppIndicator runtime libraries from your distribution when available.
