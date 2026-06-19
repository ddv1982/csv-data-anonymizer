# CSV Data Anonymizer

Tauri desktop application for anonymizing CSV data locally while preserving file structure and useful formats. The app keeps CSV processing local in Rust and ships a bundled Vite frontend for the desktop UI.

## Features

- Auto-detects column types such as email, UUID, timestamp, numeric values/IDs, phone, names, addresses, postal codes, IPs, URLs, MAC addresses, tax IDs, booleans, currency, percentages, and country codes.
- Classifies PII risk and auto-selects high/medium risk columns.
- Previews sample transformations before writing output.
- Streams CSV processing so large files do not need to be loaded fully into memory.
- Supports repeatable replacements with a persisted private seed in native app settings.
- Tracks pseudonym mappings during each run so repeated source values stay consistent and readable name replacements avoid reuse while capacity remains.
- Offers optional Local AI smart replacement through Ollama and Gemma 3 4B for selected columns, with model download/status controls in the app.
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

Local AI is off by default. The first implementation uses a user-installed Ollama runtime on `localhost` and the `gemma3:4b` model. The app can open the Ollama setup page, check runtime/model readiness, and ask Ollama to download Gemma 3 4B. Model weights and local runtime binaries are not bundled in the repository or release source.

## Anonymization Strategies

The app performs local masking and pseudonymization for selected CSV columns. It preserves useful file structure and some value formats, but it does not currently claim formal anonymization models such as k-anonymity, l-diversity, t-closeness, differential privacy, or synthetic data generation.

| Data Type | Strategy | Format Preservation |
|-----------|----------|---------------------|
| Email | Fake local part | Domain preserved |
| UUID | Hash/random UUID | Valid UUID v4 format |
| Timestamp | Date offset | Precision preserved |
| Numeric ID | Random/hash | Exact digit count preserved |
| Numeric Value | Random/hash | Sign, integer width, decimal point, and decimal precision preserved |
| Postal Code, Address, IP, URL, MAC, Tax ID | Generic replacement | Similar length, not format-specific yet |
| Boolean, Currency, Percentage | Pass-through | Unchanged |
| Phone | Digit replacement | Separators and digit count preserved |
| First/Last/Full Name | Plausible replacement names | Name token count preserved for full names; first and last name domains reuse mappings and avoid duplicate readable names while pool capacity remains |
| Country Code | Pass-through | Unchanged |
| Enum | Pass-through | Unchanged |
| String/Unknown | Generic replacement | Similar length, not semantic type |
| Any selected type with Tokenize strategy | Opaque token | Stable `tok_...` value for repeated sources within the run or deterministic seed |
| Any selected type with Smart replacement (Local AI) strategy | Local AI replacement map | Batches unique values per selected column, validates model output, reuses replacements for repeated sources, and falls back to rule-based pseudonymization when output is missing or invalid |

Current detection treats empty strings and case-insensitive `null` values as empty. General number-looking values are preserved as numeric shapes when selected, while integer columns inferred from headers containing terms such as `id`, `identifier`, `code`, `customer number`, or `account number` are treated as numeric IDs. Repeatable pseudonyms use keyed HMAC-SHA256 with the configured seed, so treat shared seeds as sensitive additional information. Local AI smart replacements are generated locally through Ollama when enabled; first-time model downloads use network access, and AI output should be reviewed in preview before writing output.

Completed runs include a privacy report that counts direct identifiers, quasi-identifiers, pseudonymized columns, smart replacement columns, opaque token columns, masked columns, generalized columns, pass-through/no-op columns, unique pseudonym values, repeated-source reuses, avoided candidate collisions, exhausted pseudonym pools, opaque token values, smart replacement values, and smart replacement fallbacks. Generalized columns currently report as zero because no generalization strategy is implemented yet. Treat the output as lower-risk transformed data, not as guaranteed anonymous data. Stronger privacy models remain roadmap items:

- k-anonymity and l-diversity for grouped quasi-identifiers.
- t-closeness for sensitive attribute distribution checks.
- Differential privacy for aggregate releases, not raw row replacement.
- Synthetic data generation for statistically similar replacement datasets.

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
