# CSV Anonymizer

[![Latest release](https://img.shields.io/github/v/release/ddv1982/csv-data-anonymizer?display_name=tag&sort=semver)](https://github.com/ddv1982/csv-data-anonymizer/releases/latest)
[![CI](https://github.com/ddv1982/csv-data-anonymizer/actions/workflows/ci.yml/badge.svg)](https://github.com/ddv1982/csv-data-anonymizer/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Platforms](https://img.shields.io/badge/platforms-macOS%20%7C%20Linux-555.svg)](https://github.com/ddv1982/csv-data-anonymizer/releases/latest)

CSV Anonymizer is a desktop app for reducing sensitive-data exposure in CSV files and pasted
data. It helps you inspect columns, choose how values should be protected, preview the result,
and create a safer copy without changing the source file.

The standard detection and transformation engine runs locally on your computer. Optional Smart
replacement uses an Ollama model configured on the loopback interface and is disabled by default.

> CSV Anonymizer reduces exposure; it does not guarantee that output is anonymous. Always review
> the preview and privacy report before sharing a generated file.

## Download

Download the latest installer from [GitHub Releases](https://github.com/ddv1982/csv-data-anonymizer/releases/latest).

### macOS

- Choose `aarch64.dmg` for Apple Silicon Macs.
- Choose `x64.dmg` for Intel Macs.
- Open the DMG and drag CSV Anonymizer into Applications.

macOS builds are signed and notarized.

### Linux

Choose the format that fits your system:

- `.AppImage` for a portable application
- `.deb` for Debian and Ubuntu
- `.rpm` for Fedora, RHEL, and compatible distributions

Debian and Ubuntu users can also enable the signed package repository:

```bash
bash <(curl -fsSL https://ddv1982.github.io/csv-data-anonymizer/install-apt-repo.sh)
sudo apt update
sudo apt install csv-anonymizer
```

Direct Linux downloads include signed checksum files and the public archive keyring.

## Using the app

1. Select a CSV file or switch to **Paste Sample**.
2. Review each column's detected format, privacy meaning, action, and risk.
3. Select the columns that should be transformed.
4. Adjust an action when the suggested choice does not fit your use case.
5. Preview the output and review any warnings.
6. Choose a new output location and run the transformation.
7. Read the privacy report before sharing the result.

The original file is never overwritten. File output must use a different path, and incomplete
runs do not leave a partially written destination behind.

## Protection actions

| Action | What it does | Keeps repeated values linkable |
| --- | --- | --- |
| Redact | Replaces selected values with a constant descriptive marker. | No |
| Mask | Hides most of each value while retaining a limited visual shape. | No |
| Pseudonymize | Creates readable or shape-preserving replacement values. | Yes |
| Tokenize | Creates opaque `tok_...` replacement values. | Yes |
| Label with column name | Gives each distinct value a numbered label based on its column. | Yes |
| Smart replacement | Uses an optional Ollama model for more natural-looking replacements. | Yes |
| Pass through | Leaves values unchanged. | Not applicable |

Actions that keep repeated values linkable produce pseudonymized data, not anonymous data. A
reader may still learn that several rows refer to the same source value, and value frequencies
can sometimes help reconnect replacements to their originals.

## What the app detects

CSV Anonymizer recognizes common structured and personal-data formats, including:

- names and contact details
- email addresses and phone numbers
- addresses and postal codes
- dates and timestamps
- persistent record identifiers
- financial and government identifiers
- IP addresses, URLs, and device addresses
- credentials and secrets suggested by headers

Detection uses both column headers and sampled values. A detected format describes what values
look like; the privacy meaning describes what the available evidence supports. Ambiguous columns
are marked for review instead of being presented as certain.

Header detection includes maintained terminology for English, Dutch, German, French, Spanish,
Portuguese, and Italian, with limited Japanese coverage for a small set of unambiguous headers.
Value-based validators operate independently of header language for supported structured formats.

## Supported input

- Streaming CSV file workflow for large UTF-8 CSV files
- Paste workflow for CSV, JSON, XML, YAML, plain text, and logs up to 5 MiB
- Quick generation by data type when no source data is needed

Files that appear binary or use an unsupported encoding are refused rather than guessed. Re-save
those files as UTF-8 before processing them.

## Optional Smart replacement

Smart replacement is intended for selected values where rule-based output is too mechanical.
It requires [Ollama](https://ollama.com/) and is off by default.

1. Install and start Ollama.
2. Open Local AI setup in CSV Anonymizer.
3. Download or select a supported local model.
4. Enable Smart replacement only for the columns that need it.
5. Review generated examples before running the transformation.

Selected values are sent to the configured loopback Ollama endpoint. A loopback address alone
cannot prove where an independently configured Ollama runtime performs inference, so review your
Ollama configuration before using sensitive data. CSV Anonymizer refuses documented cloud-model
name forms and does not bundle model weights or runtime binaries.

## Large files

CSV rows are streamed, but actions that keep repeated values consistent must remember each
distinct source value and its replacement for the duration of a run. Memory use therefore grows
with the number of distinct transformed values, not simply with file size.

Redact and Mask keep no replacement map and use substantially less memory. Preflight warns when
a selected combination may require a large mapping. Extremely large mappings are refused instead
of silently becoming inconsistent or leaving incomplete output.

## Privacy expectations

CSV Anonymizer is designed to reduce exposure for activities such as testing, demonstrations,
support, and controlled data sharing. It does not provide:

- a formal anonymity guarantee
- differential privacy
- aggregate-only output
- a fully synthetic dataset

Unselected columns and values using Pass through remain unchanged. Structural details, retained
shapes, repeated-value patterns, and combinations of otherwise ordinary fields may still identify
people or records. Treat generated output according to its remaining risk.

## Help and troubleshooting

- If analysis asks you to review a column, inspect its explanation and samples before choosing an action.
- If a file is refused because of encoding, export it as UTF-8 and try again.
- If Smart replacement is unavailable, verify that Ollama is running and that the selected model is installed.
- If a large run reports high projected memory use, use Redact or Mask for high-cardinality columns.
- Report reproducible problems through [GitHub Issues](https://github.com/ddv1982/csv-data-anonymizer/issues).

## For contributors

The application uses a Rust core, a Tauri desktop shell, and a React frontend. Development and
release details live in the repository documentation:

- [Release and signing workflow](docs/releasing.md)
- [Current architecture and modernization status](docs/modernization-status-2026-07-13.md)
- [Detection calibration](docs/calibration.md)

Install frontend dependencies and run the canonical local gate with:

```bash
npm ci --prefix frontend
npm run validate
```

The project is licensed under the [MIT License](LICENSE).
