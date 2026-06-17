/**
 * Preview Display
 * Shows anonymization preview with original → anonymized comparisons.
 */

import * as readline from 'node:readline';
import chalk from 'chalk';
import type { ColumnMetadata, PreviewRow } from '../../types/index.js';
import { transformValue, createTransformContext } from '../../core/transformer.js';
import {
  formatColumnPreview,
  drawDivider,
  formatSuccess,
  formatWarning,
} from '../output/format.js';

/**
 * Default number of preview rows to show.
 */
const DEFAULT_PREVIEW_ROWS = 5;

/**
 * Options for the preview display.
 */
export interface PreviewOptions {
  /** Input stream (default: process.stdin) */
  input?: NodeJS.ReadableStream;
  /** Output stream (default: process.stdout) */
  output?: NodeJS.WritableStream;
  /** Number of rows to preview (default: 5) */
  rowCount?: number;
  /** Seed for deterministic transforms */
  seed?: string;
  /** Whether to use deterministic mode */
  deterministic?: boolean;
}

/**
 * Create a readline interface for user input.
 * @param options - Input/output streams
 * @returns Readline interface
 */
function createReadlineInterface(options: PreviewOptions): readline.Interface {
  return readline.createInterface({
    input: options.input ?? process.stdin,
    output: options.output ?? process.stdout,
  });
}

/**
 * Prompt user for a yes/no confirmation.
 * @param rl - Readline interface
 * @param prompt - Prompt text
 * @returns Boolean indicating user's choice
 */
async function confirm(rl: readline.Interface, prompt: string): Promise<boolean> {
  return new Promise((resolve) => {
    rl.question(prompt, (answer) => {
      const normalized = answer.trim().toLowerCase();
      // Default to yes if empty, accept y/yes/1/true
      resolve(normalized === '' || normalized === 'y' || normalized === 'yes' || normalized === '1' || normalized === 'true');
    });
  });
}

/**
 * Generate preview transformations for a single column.
 *
 * @param column - Column metadata
 * @param sampleValues - Sample values to transform
 * @param seed - Seed for deterministic transforms
 * @param deterministic - Whether to use deterministic mode
 * @returns Array of preview rows
 */
export function generateColumnPreview(
  column: ColumnMetadata,
  sampleValues: string[],
  seed: string = '',
  deterministic: boolean = false
): PreviewRow[] {
  return sampleValues.map((original, rowIndex) => {
    const context = createTransformContext(column, rowIndex, seed, deterministic);
    const anonymized = transformValue(original, column, context);
    return { original, anonymized };
  });
}

/**
 * Generate preview for all selected columns.
 *
 * @param columns - All column metadata
 * @param selectedIndices - Indices of selected columns
 * @param sampleRows - Sample data rows
 * @param options - Preview options
 * @returns Map of column name to preview rows
 */
export function generatePreview(
  columns: ColumnMetadata[],
  selectedIndices: number[],
  sampleRows: string[][],
  options: PreviewOptions = {}
): Map<string, PreviewRow[]> {
  const rowCount = Math.min(options.rowCount ?? DEFAULT_PREVIEW_ROWS, sampleRows.length);
  const seed = options.seed ?? '';
  const deterministic = options.deterministic ?? false;

  const result = new Map<string, PreviewRow[]>();

  for (const idx of selectedIndices) {
    const column = columns[idx];
    if (!column) continue;

    // Extract sample values for this column
    const sampleValues = sampleRows.slice(0, rowCount).map(row => row[idx] ?? '');

    // Generate preview transformations
    const preview = generateColumnPreview(column, sampleValues, seed, deterministic);
    result.set(column.name, preview);
  }

  return result;
}

/**
 * Display the preview and prompt for confirmation.
 *
 * @param columns - All column metadata
 * @param selectedIndices - Indices of selected columns
 * @param sampleRows - Sample data rows
 * @param options - Preview options
 * @returns Boolean indicating whether to proceed
 */
export async function displayPreviewAndConfirm(
  columns: ColumnMetadata[],
  selectedIndices: number[],
  sampleRows: string[][],
  options: PreviewOptions = {}
): Promise<boolean> {
  const output = options.output ?? process.stdout;
  const write = (text: string) => output.write(text);

  // Generate preview data
  const previewData = generatePreview(columns, selectedIndices, sampleRows, options);

  // Display header
  write('\n');
  write(chalk.bold('Preview (first 5 rows):'));
  write('\n');
  write(drawDivider(60));

  // Display each column's preview
  for (const [columnName, rows] of previewData) {
    const transforms = rows.map(r => ({
      original: r.original,
      anonymized: r.anonymized,
    }));
    write(formatColumnPreview(columnName, transforms));
  }

  write('\n');
  write(drawDivider(60));
  write('\n\n');

  // Prompt for confirmation
  const rl = createReadlineInterface(options);

  try {
    const proceed = await confirm(rl, 'Proceed with anonymization? (Y/n): ');

    if (proceed) {
      write(formatSuccess('Proceeding with anonymization...') + '\n');
    } else {
      write(formatWarning('Anonymization cancelled.') + '\n');
    }

    return proceed;
  } finally {
    rl.close();
  }
}

/**
 * Display preview without prompting for confirmation.
 * Used for --preview flag (show only mode).
 *
 * @param columns - All column metadata
 * @param selectedIndices - Indices of selected columns
 * @param sampleRows - Sample data rows
 * @param options - Preview options
 */
export function displayPreviewOnly(
  columns: ColumnMetadata[],
  selectedIndices: number[],
  sampleRows: string[][],
  options: PreviewOptions = {}
): void {
  const output = options.output ?? process.stdout;
  const write = (text: string) => output.write(text);

  // Generate preview data
  const previewData = generatePreview(columns, selectedIndices, sampleRows, options);

  // Display header
  write('\n');
  write(chalk.bold('Anonymization Preview:'));
  write('\n');
  write(drawDivider(60));

  // Display each column's preview
  for (const [columnName, rows] of previewData) {
    const transforms = rows.map(r => ({
      original: r.original,
      anonymized: r.anonymized,
    }));
    write(formatColumnPreview(columnName, transforms));
  }

  write('\n');
  write(drawDivider(60));
  write('\n\n');
  write(formatSuccess('Preview complete. Use without --preview flag to process the file.') + '\n');
}

/**
 * Format a single preview row for display.
 *
 * @param row - Preview row with original and anonymized values
 * @returns Formatted preview string
 */
export function formatPreviewRow(row: PreviewRow): string {
  const arrow = chalk.dim('→');
  return `Original: ${row.original} ${arrow} Anonymized: ${row.anonymized}`;
}
