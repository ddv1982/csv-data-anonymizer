/**
 * Terminal Output Formatting Utilities
 * Provides consistent styling for CLI output using chalk.
 */

import chalk from 'chalk';
import type { ColumnMetadata, PiiRisk } from '../../types/index.js';

/**
 * Color mapping for PII risk levels.
 */
const riskColors = {
  high: chalk.red,
  medium: chalk.yellow,
  low: chalk.green,
} as const;

/**
 * Format PII risk level with appropriate color.
 * @param risk - The PII risk level
 * @returns Colored risk string
 */
export function formatPiiRisk(risk: PiiRisk): string {
  const colorFn = riskColors[risk];
  const label = risk.toUpperCase();
  return colorFn(label);
}

/**
 * Format a column type with consistent styling.
 * @param type - The data type name
 * @returns Styled type string
 */
export function formatDataType(type: string): string {
  // Format for display (e.g., "numeric_id" -> "Numeric ID")
  const formatted = type
    .split('_')
    .map(word => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
  return chalk.cyan(formatted);
}

/**
 * Format a column name with consistent styling.
 * @param name - The column name
 * @returns Styled column name
 */
export function formatColumnName(name: string): string {
  return chalk.bold(name);
}

/**
 * Format a value for display (truncate if too long).
 * @param value - The value to format
 * @param maxLength - Maximum display length
 * @returns Formatted value string
 */
export function formatValue(value: string, maxLength: number = 40): string {
  if (value === '' || value.toLowerCase() === 'null') {
    return chalk.dim('(empty)');
  }
  if (value.length > maxLength) {
    return value.substring(0, maxLength - 3) + chalk.dim('...');
  }
  return value;
}

/**
 * Format an error message for display.
 * @param message - The error message
 * @returns Colored error string
 */
export function formatError(message: string): string {
  return chalk.red(`✖ ${message}`);
}

/**
 * Format a success message for display.
 * @param message - The success message
 * @returns Colored success string
 */
export function formatSuccess(message: string): string {
  return chalk.green(`✔ ${message}`);
}

/**
 * Format a warning message for display.
 * @param message - The warning message
 * @returns Colored warning string
 */
export function formatWarning(message: string): string {
  return chalk.yellow(`⚠ ${message}`);
}

/**
 * Format an info message for display.
 * @param message - The info message
 * @returns Colored info string
 */
export function formatInfo(message: string): string {
  return chalk.blue(`ℹ ${message}`);
}

/**
 * Pad a string to a specific width with spaces.
 * @param str - The string to pad
 * @param width - Target width
 * @param align - Alignment ('left' or 'right')
 * @returns Padded string
 */
export function padString(str: string, width: number, align: 'left' | 'right' = 'left'): string {
  // Strip ANSI codes for accurate length calculation
  const stripped = stripAnsi(str);
  const padding = Math.max(0, width - stripped.length);

  if (align === 'right') {
    return ' '.repeat(padding) + str;
  }
  return str + ' '.repeat(padding);
}

/**
 * Strip ANSI color codes from a string for length calculation.
 * @param str - String potentially containing ANSI codes
 * @returns Plain string without ANSI codes
 */
function stripAnsi(str: string): string {
  // eslint-disable-next-line no-control-regex
  return str.replace(/\x1B\[[0-9;]*[mK]/g, '');
}

/**
 * Format column metadata for display in the selection list.
 * @param column - Column metadata
 * @param index - 1-based display index
 * @returns Formatted column line
 */
export function formatColumnLine(column: ColumnMetadata, index: number): string {
  const indexStr = chalk.dim(`[${index}]`);
  const name = padString(formatColumnName(column.name), 20);
  const type = padString(`(${formatDataType(column.detectedType)})`, 20);
  const risk = `- PII Risk: ${formatPiiRisk(column.piiRisk)}`;

  return `  ${indexStr} ${name} ${type} ${risk}`;
}

/**
 * Format a table of columns for display.
 * @param columns - Array of column metadata
 * @returns Formatted table string
 */
export function formatColumnTable(columns: ColumnMetadata[]): string {
  const lines = [
    chalk.bold('Detected columns:'),
    '',
  ];

  columns.forEach((column, idx) => {
    lines.push(formatColumnLine(column, idx + 1));
  });

  lines.push('');
  return lines.join('\n');
}

/**
 * Format a preview transformation for display.
 * @param original - Original value
 * @param anonymized - Anonymized value
 * @returns Formatted transformation line
 */
export function formatPreviewTransform(original: string, anonymized: string): string {
  const orig = formatValue(original);
  const anon = formatValue(anonymized);
  const arrow = chalk.dim('→');

  return `    Original: ${orig} ${arrow} Anonymized: ${anon}`;
}

/**
 * Format a column preview section.
 * @param columnName - Name of the column
 * @param transforms - Array of original/anonymized pairs
 * @returns Formatted preview section
 */
export function formatColumnPreview(
  columnName: string,
  transforms: Array<{ original: string; anonymized: string }>
): string {
  const lines = [
    '',
    `  ${formatColumnName(columnName)}:`,
  ];

  transforms.forEach(({ original, anonymized }) => {
    lines.push(formatPreviewTransform(original, anonymized));
  });

  return lines.join('\n');
}

/**
 * Draw a horizontal line/divider.
 * @param width - Width of the line
 * @param char - Character to use (default: ─)
 * @returns Divider string
 */
export function drawDivider(width: number = 60, char: string = '─'): string {
  return chalk.dim(char.repeat(width));
}

/**
 * Create a box around content.
 * @param title - Box title
 * @param content - Content lines
 * @returns Boxed content string
 */
export function drawBox(title: string, content: string[]): string {
  const maxWidth = Math.max(
    title.length + 4,
    ...content.map(line => stripAnsi(line).length + 4)
  );

  const top = '┌' + '─'.repeat(maxWidth - 2) + '┐';
  const bottom = '└' + '─'.repeat(maxWidth - 2) + '┘';
  const separator = '├' + '─'.repeat(maxWidth - 2) + '┤';

  const contentLines = content.map(line => {
    const stripped = stripAnsi(line);
    const padding = maxWidth - 4 - stripped.length;
    return '│ ' + line + ' '.repeat(Math.max(0, padding)) + ' │';
  });

  return [
    chalk.dim(top),
    chalk.dim('│') + ' ' + chalk.bold(title) + ' '.repeat(maxWidth - 4 - title.length) + ' ' + chalk.dim('│'),
    chalk.dim(separator),
    ...contentLines.map(line => line.replace(/^│/, chalk.dim('│')).replace(/│$/, chalk.dim('│'))),
    chalk.dim(bottom),
  ].join('\n');
}

/**
 * Format file size in human-readable format.
 * @param bytes - File size in bytes
 * @returns Formatted size string
 */
export function formatFileSize(bytes: number): string {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(1)} GB`;
}

/**
 * Format duration in human-readable format.
 * @param ms - Duration in milliseconds
 * @returns Formatted duration string
 */
export function formatDuration(ms: number): string {
  if (ms < 1000) {
    return `${ms}ms`;
  }
  if (ms < 60000) {
    return `${(ms / 1000).toFixed(1)}s`;
  }
  const minutes = Math.floor(ms / 60000);
  const seconds = ((ms % 60000) / 1000).toFixed(0);
  return `${minutes}m ${seconds}s`;
}

/**
 * Format row count with comma separators.
 * @param count - Row count
 * @returns Formatted count string
 */
export function formatRowCount(count: number): string {
  return count.toLocaleString();
}
