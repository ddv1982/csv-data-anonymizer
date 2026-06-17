# CSV Data Anonymizer

A CLI tool and web UI for anonymizing CSV data to protect PII (Personally Identifiable Information) while preserving data structure and format.

## Features

- **Auto-detection** of column types (email, UUID, timestamp, numeric ID, phone, country codes)
- **Format preservation** (email domains, UUID format, timestamp precision, digit counts)
- **Deterministic mode** for reproducible anonymization across runs
- **Streaming support** for large files (1GB+ with constant memory usage)
- **Interactive mode** with column selection and preview
- **Web UI** for graphical interface (Vue.js-based)
- **YAML config** support for CI/CD automation

## Installation

```bash
npm install
npm run build
```

## Quick Start

### Web UI (Graphical Interface)

```bash
# Start the web UI server (opens browser automatically)
npx csv-anonymizer serve

# Or use Makefile shortcut
make ui
```

The web UI allows you to:
1. Browse and select CSV files
2. View column types and PII risk levels
3. Select columns to anonymize with checkboxes
4. Preview transformations before processing
5. Configure output path and deterministic mode

### CLI (Command Line)

```bash
# Interactive mode - auto-detects columns and prompts for selection
npx csv-anonymizer run data.csv

# Preview what will be anonymized without processing
npx csv-anonymizer preview data.csv

# List column headers with detected types
npx csv-anonymizer headers data.csv

# Non-interactive with specific columns (by number)
npx csv-anonymizer run data.csv --columns 2,3,5 -y

# Using a config file
npx csv-anonymizer run data.csv --config config.yml -y
```

## Usage

### CLI Commands

```
csv-anonymizer <command> [options]

Commands:
  headers <file>    List column headers with detected types
  preview <file>    Preview anonymization transformations
  run <file>        Anonymize file and save output
  serve             Start the web UI server
  help [command]    Display help for command
```

### run Command Options

```
csv-anonymizer run <file> [options]

Options:
  -o, --output <file>     Output file path (default: {name}_anonymized.csv)
  -c, --config <file>     YAML config file for column settings
  -C, --columns <list>    Columns to anonymize (e.g., "1,3,5" or "all")
  -d, --deterministic     Use deterministic transforms (same input = same output)
  -s, --seed <string>     Seed for deterministic mode
  -f, --force             Overwrite output file if exists
  -q, --quiet             Suppress progress output
  -y, --yes               Skip confirmation prompt
  -h, --help              Show help
```

### serve Command Options

```
csv-anonymizer serve [options]

Options:
  -p, --port <number>     Server port (default: 3456)
  -H, --host <address>    Host address (default: localhost)
  --no-open               Don't open browser automatically
  -h, --help              Show help
```

## Examples

### Interactive Mode

```bash
npx csv-anonymizer run users.csv
```

The tool will:
1. Read and analyze the CSV file
2. Display detected columns with types and PII risk levels
3. Prompt you to select columns to anonymize
4. Show a preview of the anonymization
5. Process the file and save to `users_anonymized.csv`

### Non-Interactive Mode

```bash
# Anonymize columns 2 and 3 with deterministic output
npx csv-anonymizer run data.csv -C 2,3 -d -s "my-seed" -y

# Select all columns
npx csv-anonymizer run data.csv -C all -y
```

### Using Config Files

Create a `config.yml`:

```yaml
columns:
  - name: email_address
    type: email
  - name: customer_id
    type: uuid
  - name: id
    type: numeric_id

output: anonymized_data.csv
deterministic: true
seed: "reproducible-seed-123"
```

Run with config:

```bash
npx csv-anonymizer run data.csv --config config.yml -y
```

## Anonymization Strategies

| Data Type | Strategy | Format Preservation |
|-----------|----------|---------------------|
| Email | Fake local part | Domain preserved (`@gmail.com`) |
| UUID | Deterministic hash | Valid UUID v4 format |
| Timestamp | Date offset (±365 days) | Precision preserved (microseconds) |
| Numeric ID | Random/hash | Exact digit count preserved |
| Phone | Generic replacement | Approximate format |
| Country Code | Pass-through | Unchanged |
| Enum | Pass-through | Unchanged |

### Example Transformations

```
email:      john.doe@company.com     → rand.user42@company.com
uuid:       096d440c-d21f-48ec-...   → 7f3a2b1c-9e8d-4567-...
timestamp:  2025-07-05 02:31:59.335  → 2025-03-17 02:31:59.335
numeric_id: 1234567                  → 9876543
```

## Deterministic Mode

Use `-d` or `--deterministic` with a seed to ensure the same input always produces the same output. This is useful for:

- Maintaining referential integrity across multiple files
- Reproducible test data generation
- CI/CD pipelines

```bash
# Same seed = same output
npx csv-anonymizer run data.csv -d -s "my-seed" -C 2,3 -y
npx csv-anonymizer run data.csv -d -s "my-seed" -C 2,3 -y  # Identical output
```

## PII Risk Classification

Columns are classified by PII risk level:

| Risk Level | Data Types |
|------------|------------|
| **High** | Email, Phone, Full Name |
| **Medium** | UUID, Numeric ID, First Name, Last Name |
| **Low** | Timestamp, Country Code, Enum, Generic String |

When using `-y` (non-interactive) mode without `-C/--columns`, high-risk columns are automatically selected.

## Performance

- **Streaming architecture** - constant memory usage regardless of file size
- **1GB file** processed in under 60 seconds
- **Progress indicator** shows rows processed and elapsed time

## Development

```bash
# Install dependencies
npm install

# Build CLI and UI
npm run build

# Run tests
npm test

# Run tests with coverage
npm run test:coverage

# Type checking
npm run typecheck

# Lint
npm run lint
```

### Makefile Commands

The project includes a Makefile with shortcuts:

```bash
# Build commands
make build          # Build CLI and UI for production
make build-cli      # Build CLI only
make build-ui       # Build UI only
make clean          # Clean build artifacts

# Development
make dev            # Start UI dev server with hot reload
make test           # Run unit and integration tests
make test-coverage  # Run tests with coverage
make e2e            # Run E2E tests with Playwright
make typecheck      # Run TypeScript type check

# CLI shortcuts
make headers FILE=data.csv                    # List columns with types
make preview FILE=data.csv                    # Preview high-risk columns
make preview FILE=data.csv COLS=1,3           # Preview specific columns
make anonymize FILE=data.csv                  # Anonymize (auto-select)
make anonymize FILE=data.csv COLS=2,3         # Anonymize specific columns
make anonymize FILE=data.csv OPTS="-d -s x"   # With deterministic mode

# Web UI
make ui                                       # Start web UI server
make ui-no-open                               # Start UI without browser

# Show all commands
make help
```

## Project Structure

```
csv-anonymizer/
├── src/                      # CLI and server source
│   ├── index.ts              # CLI entry point
│   ├── cli/
│   │   ├── commands/         # CLI commands (headers, preview, run, serve)
│   │   ├── prompts/          # Interactive prompts
│   │   └── output/           # Formatting, progress
│   ├── core/                 # Shared core logic
│   │   ├── detector.ts       # Type detection engine
│   │   ├── processor.ts      # Streaming processor
│   │   └── transformer.ts    # Value transformation
│   ├── server/               # Express API server
│   │   ├── routes/           # API endpoints
│   │   └── middleware/       # Error handling, validation
│   ├── strategies/           # Anonymization strategies
│   ├── config/               # Config loading, schemas
│   ├── types/                # TypeScript definitions
│   └── utils/                # Utilities (patterns, hash)
├── ui/                       # Vue.js frontend
│   ├── src/
│   │   ├── components/       # Vue components
│   │   ├── composables/      # Vue composables
│   │   └── lib/              # API client
│   └── dist/                 # Built UI assets
├── tests/                    # Test suites
├── e2e/                      # Playwright E2E tests
└── Makefile                  # Build shortcuts
```

## License

MIT
