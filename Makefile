# Makefile for CSV Anonymizer
# A CLI tool for anonymizing CSV data with smart type detection and web UI

.PHONY: help build build-cli build-ui dev dev-api test test-watch test-coverage e2e lint typecheck clean install headers preview anonymize ui ui-no-open

# Default target - show help
.DEFAULT_GOAL := help

# ============================================================================
# Help (Default)
# ============================================================================

## Show this help message
help:
	@echo ""
	@echo "\033[1mCSV Anonymizer - Makefile Commands\033[0m"
	@echo "==================================="
	@echo ""
	@echo "\033[1mGetting Started:\033[0m"
	@echo "  make install        Install all dependencies"
	@echo "  make build          Build CLI and UI for production"
	@echo "  make ui             Start web UI server (opens browser)"
	@echo ""
	@echo "\033[1mBuild:\033[0m"
	@echo "  make build          Build CLI and UI for production"
	@echo "  make build-cli      Build CLI only"
	@echo "  make build-ui       Build UI only"
	@echo "  make clean          Clean build artifacts (dist, coverage)"
	@echo ""
	@echo "\033[1mDevelopment:\033[0m"
	@echo "  make dev            Start UI dev server with hot reload"
	@echo "  make dev-api        Start API server for UI development"
	@echo "  make typecheck      Run TypeScript type checking"
	@echo "  make lint           Run linter (alias for typecheck)"
	@echo ""
	@echo "\033[1mTesting:\033[0m"
	@echo "  make test           Run all tests"
	@echo "  make test-watch     Run tests in watch mode"
	@echo "  make test-coverage  Run tests with coverage report"
	@echo "  make e2e            Run end-to-end tests (Playwright)"
	@echo ""
	@echo "\033[1mCLI Commands:\033[0m"
	@echo "  make headers FILE=data.csv                    List columns with types and PII risk"
	@echo "  make preview FILE=data.csv                    Preview anonymization (high-risk cols)"
	@echo "  make preview FILE=data.csv COLS=1,3           Preview specific columns"
	@echo "  make anonymize FILE=data.csv                  Anonymize file (auto-select columns)"
	@echo "  make anonymize FILE=data.csv COLS=2,3         Anonymize specific columns"
	@echo "  make anonymize FILE=data.csv OPTS=\"-d -s x\"   Deterministic mode with seed"
	@echo ""
	@echo "\033[1mWeb UI:\033[0m"
	@echo "  make ui             Start web UI server (opens browser)"
	@echo "  make ui-no-open     Start web UI server (no browser)"
	@echo ""
	@echo "\033[1mExamples:\033[0m"
	@echo "  make headers FILE=users.csv"
	@echo "  make preview FILE=users.csv COLS=2,4"
	@echo "  make anonymize FILE=users.csv COLS=2,4 OPTS=\"-d -s myseed\""
	@echo ""

# ============================================================================
# Setup
# ============================================================================

## Install all dependencies
install:
	npm install

# ============================================================================
# Build Commands
# ============================================================================

## Build CLI and UI for production
build:
	npm run build

## Build CLI only
build-cli:
	npm run build:cli

## Build UI only
build-ui:
	npm run build:ui

## Clean build artifacts
clean:
	rm -rf dist ui/dist coverage
	@echo "Cleaned: dist/, ui/dist/, coverage/"

# ============================================================================
# Development Commands
# ============================================================================

## Start UI dev server with hot reload (API proxied to localhost:3456)
dev:
	npm run dev

## Start API server for UI development (run in separate terminal)
dev-api:
	@echo "Starting API server on http://localhost:3456..."
	@echo "Run 'make dev' in another terminal for the UI dev server."
	@node dist/index.js serve --no-open

## Run type checking
typecheck:
	npm run typecheck

## Run linter (alias for typecheck)
lint:
	npm run lint

# ============================================================================
# Testing Commands
# ============================================================================

## Run all tests
test:
	npm test

## Run tests in watch mode
test-watch:
	npm run test:watch

## Run tests with coverage report
test-coverage:
	npm run test:coverage

## Run E2E tests with Playwright
e2e:
	npx playwright test

# ============================================================================
# CLI Shortcuts
# ============================================================================

## List CSV headers: make headers FILE=data.csv
headers:
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make headers FILE=<path>"; \
		echo "Example: make headers FILE=data.csv"; \
		exit 1; \
	fi
	@node dist/index.js headers $(FILE)

## Preview anonymization: make preview FILE=data.csv [COLS=1,3]
preview:
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make preview FILE=<path> [COLS=1,3]"; \
		echo "Example: make preview FILE=data.csv COLS=2,3"; \
		exit 1; \
	fi
	@node dist/index.js preview $(FILE) $(if $(COLS),-C $(COLS),)

## Anonymize file: make anonymize FILE=data.csv [COLS=1,3] [OPTS="-d -s myseed"]
anonymize:
	@if [ -z "$(FILE)" ]; then \
		echo "Usage: make anonymize FILE=<path> [COLS=1,3] [OPTS=\"-d -s seed\"]"; \
		echo "Example: make anonymize FILE=data.csv COLS=2,3 OPTS=\"-d -s myseed\""; \
		exit 1; \
	fi
	@node dist/index.js run $(FILE) $(if $(COLS),-C $(COLS),) $(OPTS) -y

# ============================================================================
# Web UI Commands
# ============================================================================

## Start web UI server (opens browser automatically)
ui:
	@node dist/index.js serve

## Start web UI server without opening browser
ui-no-open:
	@node dist/index.js serve --no-open
