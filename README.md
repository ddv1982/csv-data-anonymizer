# CSV Data Anonymizer

Desktop-only Electron application for anonymizing CSV data locally while preserving file structure and useful formats.

## Features

- Auto-detects column types such as email, UUID, timestamp, numeric ID, phone, and country codes.
- Classifies PII risk and auto-selects high/medium risk columns.
- Previews sample transformations before writing output.
- Streams CSV processing so large files do not need to be loaded fully into memory.
- Supports deterministic anonymization with a persisted seed in app settings.
- Uses native desktop file pickers and opens completed output in Finder/Explorer.

## Development

```bash
pnpm install
pnpm run dev
```

Useful commands:

```bash
pnpm run build          # Type-check and build the Electron app
pnpm test               # Run unit and integration tests
pnpm run test:coverage  # Run tests with coverage
pnpm run test:e2e       # Build and run Electron smoke tests
pnpm run dist           # Package installers with electron-builder
pnpm run dist:linux     # Package Linux AppImage, deb, and rpm artifacts
pnpm run release:check  # Validate package version and changelog metadata
```

## Architecture

- `src/main` owns Electron windows, IPC handlers, native dialogs, shell actions, app settings, and filesystem access.
- `src/preload` exposes the typed `window.csvAnonymizer` bridge through `contextBridge`.
- `src/shared/contracts.ts` contains the Zod schemas, inferred TypeScript types, defaults, and renderer-facing API contract.
- `src/renderer` is the Vue renderer.
- `src/core`, `src/strategies`, `src/types`, and `src/utils` contain the reusable CSV anonymization engine.

App settings are stored as versioned JSON under Electron `app.getPath('userData')`. YAML config files and the previous command-line interface have been removed.

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
pnpm run dist:dir
pnpm run dist
```

Packaged artifacts are written to `release/<version>/`.

Release steps, Linux package signing, and macOS notarization prerequisites are documented in `docs/releasing.md`.

## Linux Install And Updates

On Debian/Ubuntu systems, enable the signed APT repository once:

```bash
bash <(curl -fsSL https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/install-apt-repo.sh)
sudo apt update
sudo apt install csv-anonymizer
```

After that, normal `sudo apt update` and `sudo apt upgrade` runs handle CSV Anonymizer updates through the package manager.

Direct `.deb`, `.rpm`, and AppImage downloads remain available from GitHub Releases for users who do not want to enable the repository.
