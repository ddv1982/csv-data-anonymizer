/**
 * Preview Command
 * Previews anonymization transformations for selected columns without modifying files.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import { readSample } from '../../core/sampleReader.js';
import { buildColumnMetadata } from '../../core/metadataBuilder.js';
import { validateFile } from '../../core/fileReader.js';
import { parseColumnSelection } from '../../config/selection.js';
import { generatePreview } from '../prompts/preview.js';
import { handleCommandError } from '../output/errorHandler.js';
import type { ColumnMetadata, PreviewRow } from '../../types/index.js';
import { formatValue, drawDivider } from '../output/format.js';

/**
 * Options for the preview command.
 */
interface PreviewCommandOptions {
  columns?: string;
  samples?: string;
  deterministic?: boolean;
  seed?: string;
}

/**
 * Default sample size for type detection.
 */
const DEFAULT_SAMPLE_SIZE = 100;

/**
 * Default number of sample rows to show.
 */
const DEFAULT_SAMPLE_COUNT = 5;

/**
 * Get indices of high-risk columns.
 */
function getHighRiskIndices(columns: ColumnMetadata[]): number[] {
  return columns
    .filter((col) => col.piiRisk === 'high')
    .map((col) => col.index);
}

/**
 * Display preview output.
 */
function displayPreview(
  previewData: Map<string, PreviewRow[]>,
  filePath: string
): void {
  const write = (text: string) => process.stdout.write(text);

  write('\n');
  write(chalk.bold(`Preview: ${filePath}`));
  write('\n');
  write(drawDivider(60));
  write('\n');

  for (const [columnName, rows] of previewData) {
    write('\n');
    write(`  ${chalk.bold(columnName)}:\n`);

    for (const row of rows) {
      const original = formatValue(row.original, 30);
      const anonymized = formatValue(row.anonymized, 30);
      const arrow = chalk.dim('→');
      write(`    ${original.padEnd(32)} ${arrow}  ${anonymized}\n`);
    }
  }

  write('\n');
  write(drawDivider(60));
  write('\n\n');

  // Footer with usage hint
  const columnIndices = Array.from(previewData.keys())
    .map((_, idx) => idx + 1)
    .join(',');
  write(
    chalk.dim(`Use 'csv-anonymizer run ${filePath} -C ${columnIndices}' to anonymize.`)
  );
  write('\n\n');
}

/**
 * Run the preview command.
 */
async function runPreview(file: string, options: PreviewCommandOptions): Promise<void> {
  try {
    // Validate input file
    await validateFile(file);

    // Read sample for type detection
    const sample = await readSample(file, DEFAULT_SAMPLE_SIZE);

    // Build column metadata
    const columns = buildColumnMetadata(sample.headers, sample.rows);

    // Determine column selection
    let selectedIndices: number[];

    if (options.columns) {
      // Parse column selection from option
      selectedIndices = parseColumnSelection(options.columns, columns.length);
    } else {
      // Default to high-risk columns
      selectedIndices = getHighRiskIndices(columns);

      if (selectedIndices.length === 0) {
        process.stdout.write(chalk.yellow('\n⚠ No high-risk columns detected.\n'));
        process.stdout.write(
          chalk.dim('Use -C option to specify columns, e.g., csv-anonymizer preview data.csv -C 1,2,3\n\n')
        );
        return;
      }
    }

    // Parse sample count
    const sampleCount = options.samples
      ? Math.max(1, Math.min(100, parseInt(options.samples, 10))) || DEFAULT_SAMPLE_COUNT
      : DEFAULT_SAMPLE_COUNT;

    // Generate preview
    const previewData = generatePreview(columns, selectedIndices, sample.rows, {
      rowCount: sampleCount,
      seed: options.seed ?? '',
      deterministic: options.deterministic ?? false,
    });

    // Display preview
    displayPreview(previewData, file);
  } catch (error) {
    handleCommandError(error);
  }
}

/**
 * Create the preview command.
 */
export function createPreviewCommand(): Command {
  const command = new Command('preview');

  command
    .description('Preview anonymization transformations for selected columns')
    .argument('<file>', 'CSV file to preview')
    .option(
      '-C, --columns <list>',
      'Column numbers to preview (e.g., "1,3,5"). Defaults to high-risk columns.'
    )
    .option(
      '-n, --samples <num>',
      'Number of sample rows to preview',
      String(DEFAULT_SAMPLE_COUNT)
    )
    .option('-d, --deterministic', 'Use deterministic transforms (same input → same output)', false)
    .option('-s, --seed <string>', 'Seed for deterministic mode')
    .action(async (file: string, options: PreviewCommandOptions) => {
      await runPreview(file, options);
    });

  return command;
}

export { runPreview };
