import { InvalidSelectionError } from '../types/errors.js';

/**
 * Parse user column selection input into an array of 0-based column indices.
 *
 * Supported input formats:
 * - Comma-separated numbers: "1,3,5" → [0, 2, 4]
 * - Ranges: "1-5" → [0, 1, 2, 3, 4]
 * - Mixed: "1,3-5,7" → [0, 2, 3, 4, 6]
 * - Special values: "all", "none"
 *
 * Input uses 1-based indexing (user-friendly), output is 0-based (code-friendly).
 *
 * @param input - User's column selection string
 * @param totalColumns - Total number of columns in the CSV (for validation)
 * @returns Array of 0-based column indices
 * @throws InvalidSelectionError if input is invalid or indices are out of range
 */
export function parseColumnSelection(input: string, totalColumns: number): number[] {
  // Normalize input: trim and lowercase for special values
  const trimmed = input.trim();

  // Handle empty input
  if (trimmed === '') {
    throw new InvalidSelectionError(input, 'Selection cannot be empty');
  }

  // Handle "all" keyword
  if (trimmed.toLowerCase() === 'all') {
    return Array.from({ length: totalColumns }, (_, i) => i);
  }

  // Handle "none" keyword
  if (trimmed.toLowerCase() === 'none') {
    return [];
  }

  // Parse comma-separated parts
  const parts = trimmed.split(',');
  const indices = new Set<number>();

  for (const part of parts) {
    const trimmedPart = part.trim();

    if (trimmedPart === '') {
      throw new InvalidSelectionError(input, 'Empty segment found (consecutive commas)');
    }

    // Check if this part is a range (contains a hyphen)
    if (trimmedPart.includes('-')) {
      const rangeParts = trimmedPart.split('-');

      // Handle negative numbers or malformed ranges
      if (rangeParts.length !== 2) {
        throw new InvalidSelectionError(input, `Invalid range format: "${trimmedPart}"`);
      }

      const [startStr, endStr] = rangeParts;

      if (startStr.trim() === '' || endStr.trim() === '') {
        throw new InvalidSelectionError(input, `Invalid range format: "${trimmedPart}"`);
      }

      const start = parseNumber(startStr.trim(), input);
      const end = parseNumber(endStr.trim(), input);

      if (start > end) {
        throw new InvalidSelectionError(
          input,
          `Range start (${start}) cannot be greater than end (${end})`
        );
      }

      // Validate bounds before adding
      validateBounds(start, totalColumns, input);
      validateBounds(end, totalColumns, input);

      // Add all indices in the range (convert 1-based to 0-based)
      for (let i = start; i <= end; i++) {
        indices.add(i - 1);
      }
    } else {
      // Single number
      const num = parseNumber(trimmedPart, input);
      validateBounds(num, totalColumns, input);
      indices.add(num - 1);
    }
  }

  // Convert Set to sorted array
  return Array.from(indices).sort((a, b) => a - b);
}

/**
 * Parse a string as a positive integer.
 *
 * @param str - String to parse
 * @param originalInput - Original input for error messages
 * @returns Parsed integer
 * @throws InvalidSelectionError if not a valid positive integer
 */
function parseNumber(str: string, originalInput: string): number {
  // Check for non-numeric characters
  if (!/^\d+$/.test(str)) {
    throw new InvalidSelectionError(originalInput, `"${str}" is not a valid positive integer`);
  }

  const num = parseInt(str, 10);

  if (isNaN(num)) {
    throw new InvalidSelectionError(originalInput, `"${str}" is not a valid number`);
  }

  if (num <= 0) {
    throw new InvalidSelectionError(originalInput, `Column numbers must be positive (got ${num})`);
  }

  return num;
}

/**
 * Validate that a column number is within bounds.
 *
 * @param num - 1-based column number
 * @param totalColumns - Total number of columns
 * @param originalInput - Original input for error messages
 * @throws InvalidSelectionError if out of bounds
 */
function validateBounds(num: number, totalColumns: number, originalInput: string): void {
  if (num > totalColumns) {
    throw new InvalidSelectionError(
      originalInput,
      `Column ${num} is out of range (total columns: ${totalColumns})`
    );
  }
}
