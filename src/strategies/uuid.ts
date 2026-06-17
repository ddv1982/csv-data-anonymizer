/**
 * UUID anonymization strategy.
 * Always deterministic - generates a valid UUID v4 from the input.
 */

import type { Strategy } from './index.js';
import type { TransformContext } from '../types/index.js';
import { isEmptyValue } from './index.js';
import { deterministicUuid } from '../utils/hash.js';

/**
 * UUID anonymization strategy implementation.
 * Always deterministic - same input UUID + seed produces same output UUID.
 * Output maintains valid UUID v4 format.
 */
export const uuidStrategy: Strategy = {
  transform(value: string, context: TransformContext): string {
    // Preserve empty values
    if (isEmptyValue(value)) {
      return value;
    }

    // Preserve case of original UUID
    const isUpperCase = value === value.toUpperCase();

    // Generate deterministic UUID using hash
    const newUuid = deterministicUuid(value, context.seed);

    // Match the case of the original
    return isUpperCase ? newUuid.toUpperCase() : newUuid;
  },
};
