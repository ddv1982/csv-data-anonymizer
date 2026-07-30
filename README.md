# CSV Anonymizer

CSV Anonymizer is a local-first desktop app for reducing sensitive CSV and pasted-data exposure before sharing, testing, demos, or support work. It detects likely personal data, previews transformations, and writes protected output while preserving the original structure where possible.

All non-LLM detection and transformation runs locally in Rust. Optional local LLM replacement also runs on your machine through Ollama.

Read the generated project wiki at [github.com/ddv1982/csv-data-anonymizer/wiki](https://github.com/ddv1982/csv-data-anonymizer/wiki).

## What It Does

- Detects common sensitive fields: emails, names, phone numbers, UUIDs, timestamps, numeric IDs, addresses, postal codes, IPs, URLs, MAC addresses, tax IDs, VAT/BTW numbers, and more.
- Auto-selects high and medium risk columns while still letting you choose exactly which columns to transform.
- Shows a preview before writing output. Rule-based preview replacements are examples; final output gets its own randomized run.
- Streams CSV file transformations instead of loading the whole file into memory, though peak memory still grows with the number of distinct values in the columns you transform — see [Memory and large files](#memory-and-large-files).
- Supports lightweight paste workflows for CSV, JSON, XML, YAML, plain text, and logs up to 5 MiB; larger CSV inputs should use the streaming file workflow.
- Includes Quick by Data Type generation for creating protected sample values without first providing input data.
- Keeps repeated source values consistent within each run.
- Offers optional Smart replacement with a local LLM for selected columns.
- Produces a privacy report with transformed column counts, redaction counts, reused values, token counts, Local AI replacement counts, and fallbacks.

## Detection Language Coverage

The app UI is currently English. CSV and pasted values are read as UTF-8, and detector rules are Unicode-aware. Files in other encodings are refused with the encoding named rather than converted, because a wrongly guessed encoding produces values that look plausible and are wrong. Re-save such a file as UTF-8 and run it again. Detection coverage is fixture-backed, but it is not a claim of full multilingual parity.

Header-based sensitive-field detection includes a maintained taxonomy for English, Dutch, German, French, Spanish, Portuguese, and Italian, plus a small Japanese pilot for unambiguous phone, address, name, and date headers. Header matching handles Unicode normalization, word segmentation, accent folding for Latin terms, camelCase splitting, compact aliases such as `apikey`, `homephone`, and `person_id`, and conservative fuzzy matching for longer taxonomy terms with sample-value confirmation.

Value validators run independently of header language for structured values such as email, UUID, IP address, URL, MAC address, IBAN, payment cards, VAT IDs, Dutch BTW/omzetbelastingnummer, US SSN/EIN, and formatted phone numbers. Dutch BTW values without an `NL` prefix are detected only under Dutch BTW header context.

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

The app batches unique values per selected column, asks the local model for realistic fake replacements, validates the response, reuses accepted replacements for repeated source values within the current run, and falls back to rule-based pseudonymization when the model output is missing or unsafe.

Model weights and local runtime binaries are not bundled in the repository or desktop release. The first model download uses network access through Ollama. CSV values selected for Smart replacement are sent only to the configured local Ollama endpoint.

## Privacy Boundary

The standard workflow transforms selected values in place: CSV file output keeps the source rows and columns, while pasted structured or text workflows keep the original shape where possible. It redacts, masks, pseudonymizes, tokenizes, or locally replaces selected values. It reduces exposure, but the output is still transformed source data, not guaranteed anonymous data.

It does not produce formal anonymity, differential privacy aggregates, or synthetic datasets. Review previews and privacy reports before sharing generated files.

## Strategies

| Strategy | Use | Keeps repeats linkable |
| --- | --- | --- |
| Redact | Replace values with typed placeholders such as `[EMAIL]`, `[PERSON]`, or `[DATE]`. | No |
| Mask | Replace values with simple masked output. | No |
| Pseudonymize | Generate readable or shape-preserving fake values. | Yes |
| Tokenize | Replace values with opaque `tok_...` tokens that stay consistent within the current run. | Yes |
| Label with column name | Replace values with a placeholder naming the column and numbering each distinct value, such as `[CUSTOMER_NOTES_1]`. Useful when detection cannot identify the values but you still want to see which rows shared one. Columns sharing a header carry their position, as `[NOTES_0_1]`, so unrelated values never share a label. | Yes |
| Smart replacement (Local AI) | Use a local LLM through Ollama for more realistic fake replacements. | Yes |
| Pass through | Leave values unchanged. | n/a |

Examples of format preservation include email domains, UUID shape, timestamp precision, numeric width and decimals, phone separators, and full-name token count.

### Pseudonymized is not anonymized

The strategies in the last column above give the same source value the same replacement every time it appears. That is what keeps a dataset useful — you can still tell that two rows referred to one person — but it also means records stay linkable to each other, so the output is *pseudonymized* rather than anonymized and remains personal data under the GDPR. Redaction and masking do not preserve that link.

Consistent replacement also preserves the shape of a column's value distribution, which is enough to work against the mapping: if a column holds only a handful of distinct values, anyone who knows how the real field is distributed can match the replacements back by how often each one occurs. The column table warns before a run when a column you have put on one of these strategies repeats few enough values for that to be practical, and the privacy report names those columns after it.

High and medium risk columns default to Redact, so this only arises for columns you deliberately move onto a linkable strategy.

### Memory and large files

File transformation streams rows — the reader holds one row at a time and detection keeps a bounded sample — so memory does not grow with file size. It grows with the number of **distinct** values in the columns you transform, because keeping repeated values consistent means remembering every distinct value and its replacement until the run ends.

Measured with one selected column over 1,000,000 rows from a 20.9 MB input, peak resident memory:

| Column contents | Redact / Mask | Label | Pseudonymize / Tokenize |
| --- | --- | --- | --- |
| 1,000 distinct values | 9 MB | 9 MB | 10 MB |
| every value distinct | 9 MB | 164 MB | 487 MB |

That is roughly 500 bytes per distinct value per column on the strategies that keep repeats linkable, and nothing at all on Redact and Mask, which keep no mapping. Four all-distinct columns in a 63 MB file reach about 1.8 GB. So the expensive case is not a large file, it is a large number of distinct values on a linkable strategy; Redact and Mask stay flat at any cardinality.

Re-measured independently at 11 MB, 162 MB and 477 MB for the second row of that table, which agrees with it to within a few percent. Broken down per mapping entry — one entry per distinct value on Label, three on Pseudonymize and Tokenize, since those also store the replacement in both directions — it is a consistent 158 to 163 bytes each.

Preflight projects this before the run. It scales each selected column's sampled distinct count to the file's real row count, sums the mapping entries the selected strategies would hold, and reports the total as a review item once it passes about 3,000,000 entries — roughly 480 MB — naming the column that contributes most. Projected from a sample of about a hundred values per column, so it is an upper bound on what the run will really hold rather than a measurement: a column whose values repeat in a way a sample that size cannot see is projected high, never low.

Nothing is capped behind your back, and that is deliberate: dropping mapping entries part-way through a run would break two things invisibly — repeated source values would stop keeping one replacement, and the privacy report's distinct and singleton counts would stop being the real ones. So the mapping is allowed to grow and the run is warned about instead.

There is a hard ceiling, and it refuses rather than degrades. A run that passes 32,000,000 mapping entries — about 5 GB, and roughly 2.7 times the largest case measured above — stops with an error naming the figure it reached, the ceiling, and the remedy, instead of being killed by the operating system with no message. Below that ceiling a machine with less memory than the run needs can still run out, which is what the preflight review item is for: moving the widest columns to Redact or Mask removes the cost entirely, and selecting fewer columns reduces it proportionally.

A run that fails part-way through leaves no half-written output. File output is written to a temporary file beside the destination and renamed into place only once the run has finished, so a failure — for any reason — deletes the temporary file and leaves the destination as it was.

Re-measure the throughput side with:

```bash
cargo bench -p csv-anonymizer-core --bench csv_streaming -- cardinality
```

Peak memory is measured separately, by the ignored harness in `crates/csv-anonymizer-core/src/strategies/tests/mapping_budget.rs`, which reads `VmHWM` after a full run and prints the bytes per mapping entry. Each case has to run in its own process, since peak resident memory is a process-wide high water mark:

```bash
cargo test -p csv-anonymizer-core --release strategies::tests::mapping_budget::peak_rss_pseudonymize_all_distinct -- --ignored --exact --nocapture
```

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

The frontend uses TypeScript 7's native compiler for builds and type checks. TypeScript 6 is installed through Microsoft's `@typescript/typescript6` compatibility package only because `typescript-eslint` still requires the TypeScript 6 programmatic API. The frontend `postinstall` rebuilds the native alias so npm consistently links `tsc` to TypeScript 7 instead of the compatibility package's transitive compiler. `npm run frontend:typecheck:compat` keeps both compilers aligned until the lint ecosystem supports the TypeScript 7 API.

Setup:

```bash
npm ci --prefix frontend
```

Run the desktop app:

```bash
npm run tauri:dev
```

Canonical local gates:

```bash
npm run fmt
npm run lint
npm run test
npm run typecheck
npm run frontend:typecheck:compat
npm run deadcode:required
npm run docs:check
npm run docs:rustdoc
```

Focused release, browser, packaging, and supply-chain checks:

```bash
npm run tooling:test
npm run release:check
npm run tauri:prebuilt:check
npm run artifacts:rust:check
npm run linux:package-manager:check
npm run frontend:build
npm run frontend:e2e
npm run frontend:a11y
npm run frontend:audit
npm run cargo:audit:required
npm run smoke:rust
```

Performance checks:

```bash
cargo bench -p csv-anonymizer-core --bench csv_streaming
cargo bench -p csv-anonymizer-core --bench detector_matrix -- --sample-size 10
```

The root `fmt`, `lint`, `test`, `typecheck`, `frontend:typecheck:compat`, `deadcode:required`, `docs:check`, and `docs:rustdoc` scripts are the canonical local gates. `docs:check` validates the commands documented in markdown; `docs:rustdoc` builds the workspace's rustdoc with warnings denied, so a doc comment cannot keep linking to an item that was renamed, made private, or deleted. It builds twice on purpose: the public pass also reports links from public docs into private items, which render as dead text for a reader, and the `--document-private-items` pass checks links *inside* private items, which the public pass never looks at. Neither pass alone catches both. The native TypeScript 7 compiler is authoritative; the compatibility check is temporary and exists only for TypeScript 6 API consumers. The dead-code scans use Knip for the frontend and cargo-machete for Rust dependency drift. CI installs exact versions of cargo-audit (`0.22.2`) and cargo-machete (`0.9.2`); use `cargo:audit:required` when a missing audit tool must fail rather than skip. The detector matrix benchmark measures the built-in detector only; the external PII library comparison is archived in `docs/detector-library-evaluation.md`.

## Architecture And Lifecycle Boundaries

- **Rust core:** `AnonymizerService` in `crates/csv-anonymizer-core/src/service.rs` is the stable facade. Control merging, path validation, preflight, preview, and privacy-report construction live in focused `service/` modules. File and direct-input previews use the shared preview orchestration so format entrypoints do not reimplement privacy behavior.
- **Contracts:** `crates/csv-anonymizer-core/src/types.rs` remains intentionally centralized. `scripts/check-contracts.mjs` reads that source directly to compare the Rust and TypeScript enum/DTO surfaces.
- **Tauri lifecycle:** `src-tauri` owns IPC validation, path authorization, background jobs, settings persistence, and Local AI integration. It conservatively permits one active anonymization writer, releases that lease before publishing terminal status, bounds backend sample requests, and serializes settings replacement through unique same-directory temporary files.
- **React workflows:** CSV orchestration lives in `useAnonymizerWorkflow`; pasted-data orchestration lives in `usePasteDataWorkflow`; rendering components consume those hooks. Local AI status refreshes are latest-request-wins and unmount-safe, and clipboard fallback reports success only when the browser copy operation succeeds.

The dated modernization evidence and remaining validation limits are recorded in [`docs/modernization-status-2026-07-13.md`](docs/modernization-status-2026-07-13.md).

## Project Layout

- `frontend` - React/Vite desktop UI.
- `src-tauri` - Tauri shell, app settings, commands, background jobs, and Ollama integration.
- `crates/csv-anonymizer-core` - CSV detection, preview, transformation, reporting, and tests.
- `crates/csv-anonymizer-app` - lightweight CLI smoke harness for the shared core.
- `build` - package metadata, icons, and platform assets.
- `scripts` - release, packaging, metadata, APT, and smoke-test tooling.

Release steps and signing requirements are documented in `docs/releasing.md`.
