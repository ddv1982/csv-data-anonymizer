/**
 * Run Command
 * Main command for anonymizing CSV files.
 * Refactored from the original anonymize.ts to work as a subcommand.
 */

import { existsSync } from 'node:fs';
import { Command } from 'commander';
import type { AnonymizationConfig } from '../../types/index.js';
import { OutputExistsError } from '../../types/errors.js';
import { handleCommandError } from '../output/errorHandler.js';
import { readSample } from '../../core/sampleReader.js';
import {
  buildColumnMetadata,
  applyColumnSelection,
} from '../../core/metadataBuilder.js';
import { processFile } from '../../core/processor.js';
import { loadConfig } from '../../config/loader.js';
import {
  generateDefaultOutputPath,
  mergeConfig,
} from '../../config/defaults.js';
import { parseColumnSelection } from '../../config/selection.js';
import { validateFile, getFileSize } from '../../core/fileReader.js';
import {
  formatSuccess,
  formatInfo,
  formatFileSize,
  formatDuration,
  formatRowCount,
} from '../output/format.js';
import { ProgressTracker, createProgressCallback } from '../output/progress.js';
import { promptColumnSelection, getSuggestedColumns } from '../prompts/columnSelect.js';
import { displayPreviewAndConfirm, displayPreviewOnly } from '../prompts/preview.js';

/**
 * Command options for the run command.
 */
interface RunCommandOptions {
  output?: string;
  config?: string;
  columns?: string;
  yes?: boolean;
  preview?: boolean;
  deterministic?: boolean;
  seed?: string;
  force?: boolean;
  quiet?: boolean;
}

/**
 * Default sample size for type detection.
 */
const DEFAULT_SAMPLE_SIZE = 100;

/**
 * Run the anonymization workflow.
 * @param inputPath - Path to input CSV file
 * @param options - CLI options
 */
export async function runAnonymization(
  inputPath: string,
  options: RunCommandOptions
): Promise<void> {
  const write = (text: string) => process.stdout.write(text);

  // Map yes flag to interactive mode (yes=true means noInteractive=true)
  const interactive = !options.yes;

  try {
    // Step 1: Validate input file
    await validateFile(inputPath);
    const fileSize = await getFileSize(inputPath);

    if (!options.quiet) {
      write('\n');
      write(formatInfo(`Input file: ${inputPath} (${formatFileSize(fileSize)})`));
      write('\n');
    }

    // Step 2: Load config if provided
    let fileConfig: AnonymizationConfig | undefined;
    if (options.config) {
      if (!options.quiet) {
        write(formatInfo(`Loading config from: ${options.config}`));
        write('\n');
      }
      fileConfig = loadConfig(options.config);
    }

    // Step 3: Merge configuration with precedence
    const merged = mergeConfig(
      {
        output: options.output,
        config: options.config,
        columns: options.columns,
        noInteractive: !interactive,
        preview: options.preview ?? false,
        deterministic: options.deterministic ?? false,
        seed: options.seed,
        force: options.force ?? false,
        quiet: options.quiet ?? false,
      },
      fileConfig
    );

    // Step 4: Determine output path
    const outputPath =
      options.output ?? fileConfig?.output ?? generateDefaultOutputPath(inputPath);

    // Step 5: Check output file doesn't exist (unless --force)
    if (!options.force && existsSync(outputPath)) {
      throw new OutputExistsError(outputPath);
    }

    if (!options.quiet) {
      write(formatInfo(`Output file: ${outputPath}`));
      write('\n');
    }

    // Step 6: Read sample for type detection
    if (!options.quiet) {
      write(formatInfo('Analyzing columns...'));
      write('\n');
    }

    const sample = await readSample(inputPath, DEFAULT_SAMPLE_SIZE);

    // Step 7: Build column metadata
    const columns = buildColumnMetadata(sample.headers, sample.rows);

    // Step 8: Determine column selection
    let selectedIndices: number[];

    if (options.columns) {
      // Use CLI-specified columns
      selectedIndices = parseColumnSelection(options.columns, columns.length);
      if (!options.quiet) {
        const names = selectedIndices.map((i) => columns[i].name).join(', ');
        write(formatInfo(`Selected columns from CLI: ${names}`));
        write('\n');
      }
    } else if (fileConfig?.columns && fileConfig.columns.length > 0) {
      // Use config file columns
      selectedIndices = [];
      for (const colConfig of fileConfig.columns) {
        const idx = columns.findIndex((c) => c.name === colConfig.name);
        if (idx !== -1) {
          selectedIndices.push(idx);
          // Override type if specified in config
          if (colConfig.type) {
            columns[idx].detectedType = colConfig.type;
          }
        }
      }
      if (!options.quiet) {
        const names = selectedIndices.map((i) => columns[i].name).join(', ');
        write(formatInfo(`Selected columns from config: ${names}`));
        write('\n');
      }
    } else if (interactive) {
      // Interactive mode: prompt user
      selectedIndices = await promptColumnSelection(columns);
    } else {
      // Non-interactive without config: auto-select high/medium PII risk
      selectedIndices = getSuggestedColumns(columns);
      if (!options.quiet) {
        if (selectedIndices.length > 0) {
          const names = selectedIndices.map((i) => columns[i].name).join(', ');
          write(formatInfo(`Auto-selected PII columns: ${names}`));
          write('\n');
        } else {
          write(formatInfo('No PII columns detected. Use --columns to specify columns.'));
          write('\n');
        }
      }
    }

    // Exit if no columns selected
    if (selectedIndices.length === 0) {
      write(formatInfo('No columns selected for anonymization. Exiting.'));
      write('\n');
      return;
    }

    // Step 9: Apply selection to metadata
    const selectedColumns = applyColumnSelection(columns, selectedIndices);

    // Step 10: Preview mode
    if (options.preview) {
      displayPreviewOnly(selectedColumns, selectedIndices, sample.rows, {
        seed: merged.seed,
        deterministic: merged.deterministic,
      });
      return;
    }

    // Step 11: Show preview and confirm (interactive mode only)
    if (interactive && !options.quiet) {
      const proceed = await displayPreviewAndConfirm(
        selectedColumns,
        selectedIndices,
        sample.rows,
        {
          seed: merged.seed,
          deterministic: merged.deterministic,
        }
      );

      if (!proceed) {
        return;
      }
    }

    // Step 12: Process the file
    const progress = new ProgressTracker(options.quiet);
    progress.start('Processing file...');

    const result = await processFile(inputPath, outputPath, selectedColumns, {
      deterministic: merged.deterministic,
      seed: merged.seed,
      onProgress: createProgressCallback(progress),
    });

    // Step 13: Report success
    progress.succeed();

    if (!options.quiet) {
      write('\n');
      write(formatSuccess('Anonymization complete!'));
      write('\n');
      write(formatInfo(`Output: ${result.outputPath}`));
      write('\n');
      write(formatInfo(`Rows processed: ${formatRowCount(result.rowCount)}`));
      write('\n');
      write(formatInfo(`Duration: ${formatDuration(result.duration)}`));
      write('\n');
      write('\n');
    }
  } catch (error) {
    handleCommandError(error);
  }
}

/**
 * Create the run command.
 * @returns Configured Command instance
 */
export function createRunCommand(): Command {
  const command = new Command('run');

  command
    .description('Anonymize CSV file columns with smart type detection')
    .argument('<file>', 'Input CSV file to anonymize')
    .option('-o, --output <file>', 'Output file path (default: {name}_anonymized.csv)')
    .option('-c, --config <file>', 'YAML config file path')
    .option('-C, --columns <list>', 'Comma-separated column numbers/indices (e.g., "1,3,5" or "all")')
    .option('-y, --yes', 'Skip confirmation prompts (non-interactive mode)')
    .option('-p, --preview', "Show preview only, don't process", false)
    .option('-d, --deterministic', 'Use deterministic transforms (same input → same output)', false)
    .option('-s, --seed <string>', 'Seed for deterministic mode')
    .option('-f, --force', 'Overwrite output file if exists', false)
    .option('-q, --quiet', 'Suppress progress output', false)
    .addHelpText(
      'after',
      `
Examples:
  $ csv-anonymizer run data.csv                      # Interactive mode
  $ csv-anonymizer run data.csv -c config.yml        # Use config file
  $ csv-anonymizer run data.csv -C 2,3,5             # Anonymize specific columns
  $ csv-anonymizer run data.csv -p                   # Show preview only
  $ csv-anonymizer run data.csv -d -s myseed         # Deterministic mode
  $ csv-anonymizer run data.csv -y                   # Skip prompts
  $ csv-anonymizer run data.csv -y -C all            # Anonymize all columns
`
    )
    .action(async (file: string, options: RunCommandOptions) => {
      await runAnonymization(file, options);
    });

  return command;
}
