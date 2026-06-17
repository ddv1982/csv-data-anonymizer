/**
 * Anonymization strategies module exports.
 * Provides the Strategy interface, TransformContext, and strategy registry.
 */

import type { DataType, TransformContext } from '../types/index.js';

// Re-export TransformContext from types for convenience
export type { TransformContext } from '../types/index.js';

/**
 * Strategy interface that all anonymization strategies must implement.
 * Each strategy handles the transformation of values for a specific data type.
 */
export interface Strategy {
  /**
   * Transform a value according to the strategy's anonymization rules.
   * @param value - The original value to transform
   * @param context - Context information including column metadata and settings
   * @returns The transformed/anonymized value
   */
  transform(value: string, context: TransformContext): string;
}

/**
 * Check if a value should be treated as empty.
 * Empty values are preserved unchanged by all strategies.
 */
export function isEmptyValue(value: string): boolean {
  return value === '' || value.toLowerCase() === 'null';
}

/**
 * Strategy registry mapping data types to their anonymization strategies.
 * Strategies are lazily loaded to avoid circular dependencies.
 */
export const STRATEGIES: Record<DataType, Strategy> = {} as Record<DataType, Strategy>;

/**
 * Register a strategy for a data type.
 * @param type - The data type this strategy handles
 * @param strategy - The strategy implementation
 */
export function registerStrategy(type: DataType, strategy: Strategy): void {
  STRATEGIES[type] = strategy;
}

/**
 * Get the strategy for a specific data type.
 * Falls back to the generic string strategy if no specific strategy is registered.
 * @param type - The data type to get a strategy for
 * @returns The strategy for the given type
 */
export function getStrategy(type: DataType): Strategy {
  const strategy = STRATEGIES[type];
  if (!strategy) {
    // Fall back to string strategy for unknown types
    return STRATEGIES.string || createPassThroughStrategy();
  }
  return strategy;
}

/**
 * Create a simple pass-through strategy that returns values unchanged.
 * Used for types that should not be modified (country_code, enum, unknown).
 */
export function createPassThroughStrategy(): Strategy {
  return {
    transform(value: string, _context: TransformContext): string {
      // Preserve empty values
      if (isEmptyValue(value)) {
        return value;
      }
      return value;
    },
  };
}

// Import and register all strategies
// Note: This will be populated as we implement each strategy
import { emailStrategy } from './email.js';
import { uuidStrategy } from './uuid.js';
import { timestampStrategy } from './timestamp.js';
import { numericIdStrategy } from './numericId.js';
import { genericStringStrategy } from './generic.js';

// Register strategies
registerStrategy('email', emailStrategy);
registerStrategy('uuid', uuidStrategy);
registerStrategy('timestamp', timestampStrategy);
registerStrategy('numeric_id', numericIdStrategy);
registerStrategy('string', genericStringStrategy);

// Pass-through strategies for types that should not be modified
const passThroughStrategy = createPassThroughStrategy();
registerStrategy('country_code', passThroughStrategy);
registerStrategy('enum', passThroughStrategy);
registerStrategy('unknown', passThroughStrategy);

// Phone and name strategies use generic string for now
registerStrategy('phone', genericStringStrategy);
registerStrategy('first_name', genericStringStrategy);
registerStrategy('last_name', genericStringStrategy);
registerStrategy('full_name', genericStringStrategy);
