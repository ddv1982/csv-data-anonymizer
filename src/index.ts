#!/usr/bin/env node

/**
 * CSV Anonymizer CLI
 * A CLI tool for anonymizing CSV data while preserving format and structure.
 *
 * Commands:
 *   headers <file>  - List column headers with detected types and PII risk
 *   preview <file>  - Preview anonymization transformations
 *   run <file>      - Anonymize file and save output
 *   serve           - Start the web UI server
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { createHeadersCommand } from './cli/commands/headers.js';
import { createPreviewCommand } from './cli/commands/preview.js';
import { createRunCommand } from './cli/commands/run.js';
import { createServeCommand } from './cli/commands/serve.js';
import { AnonymizerError } from './types/errors.js';

const program = new Command();

program
  .name('csv-anonymizer')
  .description(
    'Interactive CSV Anonymizer - A CLI tool for anonymizing CSV data while preserving format and structure'
  )
  .version('1.0.0');

// Add subcommands
program.addCommand(createHeadersCommand());
program.addCommand(createPreviewCommand());
program.addCommand(createRunCommand());
program.addCommand(createServeCommand());

// Global error handling for uncaught errors
program.hook('preAction', () => {
  process.on('uncaughtException', (error) => {
    if (error instanceof AnonymizerError) {
      process.stderr.write(chalk.red(`\n✖ ${error.toUserMessage()}\n\n`));
    } else {
      const message = error instanceof Error ? error.message : 'Unknown error occurred';
      process.stderr.write(chalk.red(`\n✖ Error: ${message}\n\n`));
    }
    process.exit(1);
  });
});

// Handle SIGINT gracefully
process.on('SIGINT', () => {
  console.log(chalk.yellow('\n\nOperation cancelled by user'));
  process.exit(130);
});

// Handle no arguments - show help
if (process.argv.length <= 2) {
  program.help();
}

program.parse(process.argv);
