/**
 * Anonymize Command
 * Main CLI command that orchestrates the anonymization workflow.
 */

import { existsSync } from 'node:fs';
import { Command } from 'commander';
import type { AnonymizeCommandOptions, AnonymizationConfig } from '../../types/index.js';
import {
  AnonymizerError,
  OutputExistsError,
} from '../../types/errors.js';
import {
  readSample,
} from '../../core/sampleReader.js';
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
  formatError,
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
 * Default sample size for type detection.
 */
const DEFAULT_SAMPLE_SIZE = 100;

/**
 * Create the anonymize command for Commander.
 * @returns Configured Command instance
 */
export function createAnonymizeCommand(): Command {
  const command = new Command('anonymize');

  command
    .description('Anonymize CSV file columns with smart type detection')
    .argument('<file>', 'Input CSV file to anonymize')
    .option('-o, --output <file>', 'Output file path (default: {name}_anonymized.csv)')
    .option('-c, --config <file>', 'YAML config file path')
    .option('-C, --columns <list>', 'Comma-separated column names/indices')
    .option('--no-interactive', 'Skip interactive prompts')
    .option('-p, --preview', 'Show preview only, don\'t process', false)
    .option('-d, --deterministic', 'Use deterministic transforms', false)
    .option('-s, --seed <string>', 'Seed for deterministic mode')
    .option('-f, --force', 'Overwrite output file if exists', false)
    .option('-q, --quiet', 'Suppress progress output', false)
    .action(async (file: string, options: AnonymizeCommandOptions) => {
      await runAnonymize(file, options);
    });

  return command;
}

/**
 * Main anonymization workflow.
 * @param inputPath - Path to input CSV file
 * @param options - CLI options
 */
export async function runAnonymize(
  inputPath: string,
  options: AnonymizeCommandOptions
): Promise<void> {
  const write = (text: string) => process.stdout.write(text);
  const writeErr = (text: string) => process.stderr.write(text);

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
    const merged = mergeConfig(options, fileConfig);

    // Step 4: Determine output path
    const outputPath = options.output ?? fileConfig?.output ?? generateDefaultOutputPath(inputPath);

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
        const names = selectedIndices.map(i => columns[i].name).join(', ');
        write(formatInfo(`Selected columns from CLI: ${names}`));
        write('\n');
      }
    } else if (fileConfig?.columns && fileConfig.columns.length > 0) {
      // Use config file columns
      selectedIndices = [];
      for (const colConfig of fileConfig.columns) {
        const idx = columns.findIndex(c => c.name === colConfig.name);
        if (idx !== -1) {
          selectedIndices.push(idx);
          // Override type if specified in config
          if (colConfig.type) {
            columns[idx].detectedType = colConfig.type;
          }
        }
      }
      if (!options.quiet) {
        const names = selectedIndices.map(i => columns[i].name).join(', ');
        write(formatInfo(`Selected columns from config: ${names}`));
        write('\n');
      }
    } else if (options.noInteractive === false) {
      // Interactive mode: prompt user
      selectedIndices = await promptColumnSelection(columns);
    } else {
      // Non-interactive without config: auto-select high/medium PII risk
      selectedIndices = getSuggestedColumns(columns);
      if (!options.quiet) {
        if (selectedIndices.length > 0) {
          const names = selectedIndices.map(i => columns[i].name).join(', ');
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
    if (options.noInteractive === false && !options.quiet) {
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

    const result = await processFile(
      inputPath,
      outputPath,
      selectedColumns,
      {
        deterministic: merged.deterministic,
        seed: merged.seed,
        onProgress: createProgressCallback(progress),
      }
    );

    // Step 13: Report success
    progress.succeed();

    if (!options.quiet) {
      write('\n');
      write(formatSuccess(`Anonymization complete!`));
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
    // Handle errors with user-friendly messages
    if (error instanceof AnonymizerError) {
      writeErr('\n' + formatError(error.toUserMessage()) + '\n\n');
      process.exit(1);
    }

    // Unexpected error
    const message = error instanceof Error ? error.message : 'Unknown error occurred';
    writeErr('\n' + formatError(message) + '\n\n');
    process.exit(1);
  }
}

/**
 * Create the default command (anonymize is the default action).
 * @returns Configured Command for default action
 */
export function createDefaultCommand(): Command {
  const command = new Command();

  command
    .argument('<file>', 'Input CSV file to anonymize')
    .option('-o, --output <file>', 'Output file path (default: {name}_anonymized.csv)')
    .option('-c, --config <file>', 'YAML config file path')
    .option('-C, --columns <list>', 'Comma-separated column names/indices')
    .option('--no-interactive', 'Skip interactive prompts')
    .option('-p, --preview', 'Show preview only, don\'t process', false)
    .option('-d, --deterministic', 'Use deterministic transforms', false)
    .option('-s, --seed <string>', 'Seed for deterministic mode')
    .option('-f, --force', 'Overwrite output file if exists', false)
    .option('-q, --quiet', 'Suppress progress output', false)
    .action(async (file: string, options: AnonymizeCommandOptions) => {
      await runAnonymize(file, options);
    });

  return command;
}
