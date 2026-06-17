/**
 * Column Selection Prompt
 * Interactive prompt for selecting columns to anonymize.
 */

import * as readline from 'node:readline';
import type { ColumnMetadata } from '../../types/index.js';
import { parseColumnSelection } from '../../config/selection.js';
import { formatColumnTable, formatInfo } from '../output/format.js';

/**
 * Options for the column selection prompt.
 */
export interface ColumnSelectOptions {
  /** Input stream (default: process.stdin) */
  input?: NodeJS.ReadableStream;
  /** Output stream (default: process.stdout) */
  output?: NodeJS.WritableStream;
  /** Auto-select columns with high/medium PII risk */
  autoSelectPii?: boolean;
}

/**
 * Create a readline interface for user input.
 * @param options - Input/output streams
 * @returns Readline interface
 */
function createReadlineInterface(options: ColumnSelectOptions): readline.Interface {
  return readline.createInterface({
    input: options.input ?? process.stdin,
    output: options.output ?? process.stdout,
  });
}

/**
 * Prompt user for a line of input.
 * @param rl - Readline interface
 * @param prompt - Prompt text
 * @returns User input
 */
function question(rl: readline.Interface, prompt: string): Promise<string> {
  return new Promise((resolve) => {
    rl.question(prompt, (answer) => {
      resolve(answer);
    });
  });
}

/**
 * Display the column selection prompt and get user's selection.
 *
 * @param columns - Array of column metadata to display
 * @param options - Prompt options
 * @returns Array of 0-based selected column indices
 */
export async function promptColumnSelection(
  columns: ColumnMetadata[],
  options: ColumnSelectOptions = {}
): Promise<number[]> {
  const output = options.output ?? process.stdout;
  const write = (text: string) => output.write(text);

  // Display column table
  write('\n');
  write(formatColumnTable(columns));

  // Show suggestion for high-risk columns
  const highRiskColumns = columns.filter(c => c.piiRisk === 'high' || c.piiRisk === 'medium');
  if (highRiskColumns.length > 0) {
    const highRiskIndices = highRiskColumns.map(c => c.index + 1).join(',');
    write(formatInfo(`Suggested columns with PII risk: ${highRiskIndices}`));
    write('\n\n');
  }

  // Create readline for input
  const rl = createReadlineInterface(options);

  try {
    const prompt = 'Enter columns to anonymize (e.g., 1,3,5 or 1-3 or \'all\'): ';
    const input = await question(rl, prompt);

    // Parse and validate selection
    const selectedIndices = parseColumnSelection(input, columns.length);

    // Show confirmation of selection
    if (selectedIndices.length === 0) {
      write('\n' + formatInfo('No columns selected.') + '\n');
    } else {
      const selectedNames = selectedIndices.map(i => columns[i].name).join(', ');
      write('\n' + formatInfo(`Selected ${selectedIndices.length} column(s): ${selectedNames}`) + '\n');
    }

    return selectedIndices;
  } finally {
    rl.close();
  }
}

/**
 * Get suggested column indices based on PII risk.
 * Returns indices of columns with high or medium PII risk.
 *
 * @param columns - Array of column metadata
 * @returns Array of 0-based indices for high/medium risk columns
 */
export function getSuggestedColumns(columns: ColumnMetadata[]): number[] {
  return columns
    .filter(c => c.piiRisk === 'high' || c.piiRisk === 'medium')
    .map(c => c.index);
}

/**
 * Validate a column selection against available columns.
 *
 * @param selection - Array of 0-based column indices
 * @param columns - Array of column metadata
 * @returns Validated selection (invalid indices removed)
 */
export function validateSelection(
  selection: number[],
  columns: ColumnMetadata[]
): number[] {
  return selection.filter(idx => idx >= 0 && idx < columns.length);
}

/**
 * Format selected columns summary.
 *
 * @param columns - Array of column metadata
 * @param selectedIndices - Array of selected indices
 * @returns Summary string
 */
export function formatSelectionSummary(
  columns: ColumnMetadata[],
  selectedIndices: number[]
): string {
  if (selectedIndices.length === 0) {
    return 'No columns selected for anonymization.';
  }

  const selectedNames = selectedIndices.map(i => columns[i].name);
  return `Selected ${selectedIndices.length} column(s) for anonymization: ${selectedNames.join(', ')}`;
}
