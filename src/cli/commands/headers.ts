/**
 * Headers Command
 * Analyzes a CSV file and displays column headers with detected types and PII risk levels.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { readSample } from '../../core/sampleReader.js';
import { buildColumnMetadata } from '../../core/metadataBuilder.js';
import { validateFile } from '../../core/fileReader.js';
import { handleCommandError } from '../output/errorHandler.js';
import type { ColumnMetadata, PiiRisk } from '../../types/index.js';

/**
 * Options for the headers command.
 */
interface HeadersCommandOptions {
  quiet?: boolean;
}

/**
 * Default sample size for type detection.
 */
const DEFAULT_SAMPLE_SIZE = 100;

/**
 * Get risk badge display with appropriate color.
 */
function getRiskBadge(risk: PiiRisk): string {
  switch (risk) {
    case 'high':
      return chalk.red.bold('high');
    case 'medium':
      return chalk.yellow('medium');
    case 'low':
      return chalk.green('low');
    default:
      return chalk.dim('unknown');
  }
}

/**
 * Format column type for display.
 */
function formatType(type: string): string {
  return type.replace(/_/g, ' ');
}

/**
 * Calculate column widths for table display.
 */
function calculateColumnWidths(columns: ColumnMetadata[]): { name: number; type: number } {
  const nameWidth = Math.max(
    'Column Name'.length,
    ...columns.map((c) => c.name.length)
  );
  const typeWidth = Math.max(
    'Type'.length,
    ...columns.map((c) => formatType(c.detectedType).length)
  );
  return { name: nameWidth, type: typeWidth };
}

/**
 * Display formatted table of columns.
 */
function displayTable(columns: ColumnMetadata[], filePath: string): void {
  const write = (text: string) => process.stdout.write(text);

  const widths = calculateColumnWidths(columns);
  const numWidth = Math.max(2, columns.length.toString().length);

  // Header
  write('\n');
  write(chalk.bold(`CSV Headers: ${filePath}`));
  write('\n\n');

  // Table header
  const headerRow = [
    '#'.padEnd(numWidth),
    'Column Name'.padEnd(widths.name),
    'Type'.padEnd(widths.type),
    'PII Risk',
  ].join('  ');
  write(`  ${chalk.bold(headerRow)}\n`);

  // Separator
  const separator = '─'.repeat(numWidth + widths.name + widths.type + 24);
  write(`  ${chalk.dim(separator)}\n`);

  // Data rows
  columns.forEach((column, idx) => {
    const num = (idx + 1).toString().padEnd(numWidth);
    const name = column.name.padEnd(widths.name);
    const type = formatType(column.detectedType).padEnd(widths.type);
    const risk = getRiskBadge(column.piiRisk);

    write(`  ${chalk.dim(num)}  ${name}  ${type}  ${risk}\n`);
  });

  write('\n');

  // Footer with usage hint
  write(
    chalk.dim(`Use column numbers with 'run' command: csv-anonymizer run ${filePath} -C 1,2,3`)
  );
  write('\n\n');
}

/**
 * Display JSON output for quiet mode.
 */
function displayJson(columns: ColumnMetadata[]): void {
  const output = {
    columns: columns.map((col, idx) => ({
      index: idx + 1,
      name: col.name,
      type: col.detectedType,
      piiRisk: col.piiRisk,
    })),
  };
  console.log(JSON.stringify(output, null, 2));
}

/**
 * Run the headers command.
 */
async function runHeaders(file: string, options: HeadersCommandOptions): Promise<void> {
  try {
    // Validate input file
    await validateFile(file);

    // Read sample for type detection
    const sample = await readSample(file, DEFAULT_SAMPLE_SIZE);

    // Build column metadata
    const columns = buildColumnMetadata(sample.headers, sample.rows);

    // Output
    if (options.quiet) {
      displayJson(columns);
    } else {
      displayTable(columns, file);
    }
  } catch (error) {
    handleCommandError(error);
  }
}

/**
 * Create the headers command.
 */
export function createHeadersCommand(): Command {
  const command = new Command('headers');

  command
    .description('List column headers with detected types and PII risk levels')
    .argument('<file>', 'CSV file to analyze')
    .option('-q, --quiet', 'Output JSON format (for scripting)')
    .action(async (file: string, options: HeadersCommandOptions) => {
      await runHeaders(file, options);
    });

  return command;
}

export { runHeaders };
