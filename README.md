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

## Linux Install And Updates

The active Electrobun release path publishes Linux `.tar.zst`, setup `.tar.gz`, `.deb`, `.rpm`, AppImage, update metadata, and detached signatures through GitHub Releases.

Debian/Ubuntu users can enable the signed APT repository once:

```bash
bash <(curl -fsSL https://github.com/ddv1982/csv-data-anonymizer/releases/latest/download/install-apt-repo.sh)
sudo apt update
sudo apt install csv-anonymizer
```

After that, normal `sudo apt update` and `sudo apt upgrade` runs handle package-manager updates.
