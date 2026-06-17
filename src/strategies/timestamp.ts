/**
 * Timestamp anonymization strategy.
 * Offsets dates while preserving precision format.
 */

import type { Strategy } from './index.js';
import type { TransformContext } from '../types/index.js';
import { isEmptyValue } from './index.js';
import { deterministicNumber } from '../utils/hash.js';

/**
 * Timestamp format types that we can detect and preserve.
 */
type TimestampFormat = 'date_only' | 'datetime' | 'datetime_micro';

/**
 * Detect the format of a timestamp string.
 * @param value - The timestamp string
 * @returns The detected format
 */
function detectFormat(value: string): TimestampFormat {
  // Check for microseconds (has decimal point in seconds)
  if (/\.\d+$/.test(value)) {
    return 'datetime_micro';
  }

  // Check for time component (has T or space followed by time)
  if (/[T ]\d{2}:\d{2}:\d{2}$/.test(value)) {
    return 'datetime';
  }

  // Date only format
  return 'date_only';
}

/**
 * Parse a timestamp string into components.
 * @param value - The timestamp string
 * @returns Object with date and optional time components
 */
function parseTimestamp(value: string): {
  year: number;
  month: number;
  day: number;
  separator?: string;
  hours?: number;
  minutes?: number;
  seconds?: number;
  microseconds?: string;
} {
  // Parse date part (YYYY-MM-DD)
  const dateMatch = value.match(/^(\d{4})-(\d{2})-(\d{2})/);
  if (!dateMatch) {
    throw new Error(`Invalid timestamp format: ${value}`);
  }

  const result: ReturnType<typeof parseTimestamp> = {
    year: parseInt(dateMatch[1], 10),
    month: parseInt(dateMatch[2], 10),
    day: parseInt(dateMatch[3], 10),
  };

  // Parse time part if present
  const timeMatch = value.match(/([T ])(\d{2}):(\d{2}):(\d{2})(\.\d+)?$/);
  if (timeMatch) {
    result.separator = timeMatch[1];
    result.hours = parseInt(timeMatch[2], 10);
    result.minutes = parseInt(timeMatch[3], 10);
    result.seconds = parseInt(timeMatch[4], 10);
    if (timeMatch[5]) {
      result.microseconds = timeMatch[5]; // Keep the entire decimal part including the dot
    }
  }

  return result;
}

/**
 * Format timestamp components back to string.
 * @param components - The parsed timestamp components
 * @param format - The original format to preserve
 * @returns Formatted timestamp string
 */
function formatTimestamp(
  components: ReturnType<typeof parseTimestamp>,
  format: TimestampFormat
): string {
  const pad2 = (n: number) => n.toString().padStart(2, '0');
  const datePart = `${components.year}-${pad2(components.month)}-${pad2(components.day)}`;

  if (format === 'date_only') {
    return datePart;
  }

  const separator = components.separator || ' ';
  const timePart = `${pad2(components.hours || 0)}:${pad2(components.minutes || 0)}:${pad2(components.seconds || 0)}`;

  if (format === 'datetime_micro' && components.microseconds) {
    return `${datePart}${separator}${timePart}${components.microseconds}`;
  }

  return `${datePart}${separator}${timePart}`;
}

/**
 * Apply an offset to a date.
 * @param year - Year
 * @param month - Month (1-12)
 * @param day - Day (1-31)
 * @param offsetDays - Number of days to offset (positive or negative)
 * @returns New date components
 */
function applyDateOffset(
  year: number,
  month: number,
  day: number,
  offsetDays: number
): { year: number; month: number; day: number } {
  // Create a Date object and apply the offset
  const date = new Date(year, month - 1, day); // month is 0-indexed in Date
  date.setDate(date.getDate() + offsetDays);

  return {
    year: date.getFullYear(),
    month: date.getMonth() + 1, // Convert back to 1-indexed
    day: date.getDate(),
  };
}

/**
 * Timestamp anonymization strategy implementation.
 * Offsets dates by a random amount (±365 days) while preserving time and format.
 */
export const timestampStrategy: Strategy = {
  transform(value: string, context: TransformContext): string {
    // Preserve empty values
    if (isEmptyValue(value)) {
      return value;
    }

    try {
      // Detect and parse the timestamp
      const format = detectFormat(value);
      const components = parseTimestamp(value);

      // Calculate offset (±365 days)
      let offsetDays: number;
      if (context.deterministic) {
        // Deterministic offset based on hash
        offsetDays = deterministicNumber(value, context.seed, -365, 365);
      } else {
        // Random offset
        offsetDays = Math.floor(Math.random() * 731) - 365; // -365 to +365
      }

      // Apply offset to date
      const newDate = applyDateOffset(
        components.year,
        components.month,
        components.day,
        offsetDays
      );

      // Update components with new date
      components.year = newDate.year;
      components.month = newDate.month;
      components.day = newDate.day;

      // Format and return
      return formatTimestamp(components, format);
    } catch {
      // If parsing fails, return original value
      return value;
    }
  },
};
