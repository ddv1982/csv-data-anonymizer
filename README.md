# CSV Anonymizer

CSV Anonymizer is a local-first desktop app for reducing sensitive CSV exposure before sharing, testing, demos, or support work. It detects likely personal data, previews transformations, and writes a new CSV while preserving the original structure.

All normal CSV processing runs locally in Rust. Optional local LLM replacement also runs on your machine through Ollama.

## What It Does

- Detects common sensitive columns: emails, names, phone numbers, UUIDs, timestamps, numeric IDs, addresses, postal codes, IPs, URLs, MAC addresses, tax IDs, and more.
- Auto-selects high and medium risk columns while still letting you choose exactly which columns to transform.
- Shows a preview before writing output.
- Streams standard masking/pseudonymization runs instead of loading the whole file into memory.
- Supports lightweight paste workflows for small CSV text or JSON snippets up to 5 MiB; larger CSV inputs should use the streaming file workflow.
- Keeps repeated source values consistent within a run.
- Supports repeatable replacements with a private seed, useful when multiple files need matching pseudonyms.
- Offers optional Smart replacement with a local LLM for selected columns.
- Produces a privacy report with transformed column counts, reused values, token counts, Local AI replacement counts, and fallbacks.

## Local LLM Smart Replacement

Smart replacement is optional and off by default. It is designed for columns where rule-based masking is too mechanical and you want more realistic fake values.

The first implementation uses:

- [Ollama](https://ollama.com/) running on `localhost`
- `gemma3:4b` as the lightweight default model
- In-app status checks, setup link, model download, progress, and cancel controls

Usage:

1. Install or start Ollama.
2. In CSV Anonymizer, open Local AI setup when Smart replacement prompts for it.
3. Download `gemma3:4b` from the app if it is not already available.
4. Select `Smart replacement (Local AI)` for the columns that should use the model.
5. Review the preview, then run the transformation.

The app batches unique values per selected column, asks the local model for realistic fake replacements, validates the response, reuses accepted replacements for repeated source values, and falls back to rule-based pseudonymization when the model output is missing or unsafe.

Model weights and local runtime binaries are not bundled in the repository or desktop release. The first model download uses network access through Ollama. CSV values selected for Smart replacement are sent only to the configured local Ollama endpoint.

## Privacy Boundary

The standard workflow is row-level transformation: it masks, pseudonymizes, tokenizes, or locally replaces selected values in the source rows. It reduces exposure, but the output is still transformed source data, not guaranteed anonymous data.

It does not produce formal anonymity, differential privacy aggregates, or synthetic datasets. Review previews and privacy reports before sharing generated files.

## Strategies

| Strategy | Use |
| --- | --- |
| Mask | Replace values with simple masked output. |
| Pseudonymize | Generate readable or shape-preserving fake values. |
| Tokenize | Replace values with stable opaque `tok_...` tokens. |
| Smart replacement (Local AI) | Use a local LLM through Ollama for more realistic fake replacements. |
| Pass-through | Leave values unchanged. |

Examples of format preservation include email domains, UUID shape, timestamp precision, numeric width and decimals, phone separators, and full-name token count.

## Install

Download desktop builds from [GitHub Releases](https://github.com/ddv1982/csv-data-anonymizer/releases).

macOS:

- Download the `.dmg` for your Mac.
- Use `aarch64` for Apple Silicon and `x64` for Intel.
- Drag the app into Applications.

Linux:

- Download the `.AppImage`, `.deb`, or `.rpm` from the latest release.
- For direct downloads, also download the matching `.sha256` and `.sha256.asc` files and verify them with the release signing key (`csv-anonymizer-archive-keyring.pgp`) before installing.
- Debian/Ubuntu users can enable the signed APT repository:

```bash
bash <(curl -fsSL https://ddv1982.github.io/csv-data-anonymizer/install-apt-repo.sh)
sudo apt update
sudo apt install csv-anonymizer
```

After the repository is enabled, normal `sudo apt update` and `sudo apt upgrade` runs handle updates.

## Development

Requirements:

- Rust stable
- Node.js 22.13 or newer
- Frontend dependencies from `frontend/package-lock.json`
- Playwright Chromium for browser e2e checks: `cd frontend && npx playwright install chromium`

Setup:

```bash
npm ci --prefix frontend
```

Run the desktop app:

```bash
npm run tauri:dev
```

Useful checks:

```bash
npm run typecheck
npm run lint
npm run test
npm run fmt
npm run deadcode:required
npm run release:check
npm run artifacts:rust:check
npm run linux:package-manager:check
npm run frontend:e2e
npm run frontend:a11y
npm run frontend:audit
npm run cargo:audit
cargo bench -p csv-anonymizer-core --bench csv_streaming
node scripts/rust-smoke.mjs
```

The root `lint`, `test`, `typecheck`, `fmt`, and `deadcode:required` scripts are the canonical local gates. The dead-code scans use Knip for the frontend and cargo-machete for Rust dependency drift, and the weekly GitHub Actions maintenance workflow runs the same required dead-code gate.

## Project Layout

- `frontend` - React/Vite desktop UI.
- `src-tauri` - Tauri shell, app settings, commands, background jobs, and Ollama integration.
- `crates/csv-anonymizer-core` - CSV detection, preview, transformation, reporting, and tests.
- `crates/csv-anonymizer-app` - lightweight CLI smoke harness for the shared core.
- `build` - package metadata, icons, and platform assets.
- `scripts` - release, packaging, metadata, APT, and smoke-test tooling.

Release steps and signing requirements are documented in `docs/releasing.md`.
