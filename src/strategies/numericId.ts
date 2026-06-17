/**
 * Numeric ID anonymization strategy.
 * Generates random numbers preserving exact digit count.
 */

import type { Strategy } from './index.js';
import type { TransformContext } from '../types/index.js';
import { isEmptyValue } from './index.js';
import { deterministicString } from '../utils/hash.js';

/**
 * Generate a random numeric string of the specified length.
 * @param length - Number of digits
 * @param deterministic - Whether to use deterministic generation
 * @param value - Original value for deterministic hashing
 * @param seed - Seed for deterministic hashing
 * @returns A numeric string of the specified length
 */
function generateNumericId(
  length: number,
  deterministic: boolean,
  value: string,
  seed: string
): string {
  if (deterministic) {
    // Use deterministic hash-based generation
    // First digit can't be 0 to avoid leading zeros changing the effective length
    const firstDigit = deterministicString(value, seed + ':first', 1, '123456789');

    if (length === 1) {
      return firstDigit;
    }

    const restDigits = deterministicString(value, seed + ':rest', length - 1, '0123456789');
    return firstDigit + restDigits;
  }

  // Random generation
  // Ensure first digit is not 0 to preserve length when parsed
  let result = String(Math.floor(Math.random() * 9) + 1);

  for (let i = 1; i < length; i++) {
    result += String(Math.floor(Math.random() * 10));
  }

  return result;
}

/**
 * Numeric ID anonymization strategy implementation.
 * Preserves exact digit count while anonymizing the value.
 */
export const numericIdStrategy: Strategy = {
  transform(value: string, context: TransformContext): string {
    // Preserve empty values
    if (isEmptyValue(value)) {
      return value;
    }

    // Get the digit count
    const digitCount = value.length;

    // Handle edge case of single digit
    if (digitCount === 0) {
      return value;
    }

    // Check if original has leading zeros (special case)
    const hasLeadingZeros = value[0] === '0' && value.length > 1;

    if (hasLeadingZeros) {
      // Preserve leading zeros pattern
      // Count leading zeros
      let leadingZeros = 0;
      for (const char of value) {
        if (char === '0') {
          leadingZeros++;
        } else {
          break;
        }
      }

      // Generate the non-zero part
      const nonZeroLength = digitCount - leadingZeros;
      if (nonZeroLength <= 0) {
        // All zeros - return as is
        return value;
      }

      // Generate new non-zero part
      const newNonZero = generateNumericId(
        nonZeroLength,
        context.deterministic,
        value,
        context.seed
      );

      return '0'.repeat(leadingZeros) + newNonZero;
    }

    // Standard case: generate new numeric ID with same digit count
    return generateNumericId(digitCount, context.deterministic, value, context.seed);
  },
};
