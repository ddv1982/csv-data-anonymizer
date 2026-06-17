/**
 * Value Transformer
 * Applies anonymization strategies to values based on column metadata.
 */

import type { ColumnMetadata } from '../types/column.js';
import type { TransformContext } from '../types/config.js';
import { getStrategy, isEmptyValue } from '../strategies/index.js';

/**
 * Transforms a single value using the appropriate strategy for its column type.
 *
 * @param value - The original value to transform
 * @param column - Column metadata including detected type
 * @param context - Transformation context with settings
 * @returns The transformed/anonymized value
 */
export function transformValue(
  value: string,
  column: ColumnMetadata,
  context: TransformContext
): string {
  // Preserve empty values unchanged
  if (isEmptyValue(value)) {
    return value;
  }

  // Get the appropriate strategy for this column type
  const strategy = getStrategy(column.detectedType);

  // Transform the value using the strategy
  return strategy.transform(value, context);
}

/**
 * Creates a transform context for a specific column and row.
 *
 * @param column - Column metadata
 * @param rowIndex - Current row index (0-based, excludes header)
 * @param seed - Seed for deterministic transformations
 * @param deterministic - Whether to use deterministic mode
 * @returns Complete transform context
 */
export function createTransformContext(
  column: ColumnMetadata,
  rowIndex: number,
  seed: string,
  deterministic: boolean
): TransformContext {
  return {
    columnName: column.name,
    columnIndex: column.index,
    rowIndex,
    seed,
    deterministic,
    emptyFormat: column.emptyFormat,
  };
}

/**
 * Transforms an entire row of values.
 * Only transforms columns that are selected for anonymization.
 *
 * @param row - Array of values for a single row
 * @param columns - Array of column metadata (all columns)
 * @param rowIndex - Current row index
 * @param seed - Seed for deterministic transformations
 * @param deterministic - Whether to use deterministic mode
 * @returns Transformed row with only selected columns anonymized
 */
export function transformRow(
  row: string[],
  columns: ColumnMetadata[],
  rowIndex: number,
  seed: string,
  deterministic: boolean
): string[] {
  return row.map((value, colIndex) => {
    const column = columns[colIndex];

    // If column doesn't exist in metadata or is not selected, return unchanged
    if (!column || !column.isSelected) {
      return value;
    }

    const context = createTransformContext(column, rowIndex, seed, deterministic);
    return transformValue(value, column, context);
  });
}

/**
 * Creates a transformer function for batch processing.
 * Returns a function that can transform rows efficiently.
 *
 * @param columns - Array of column metadata
 * @param seed - Seed for deterministic transformations
 * @param deterministic - Whether to use deterministic mode
 * @returns Row transformer function
 */
export function createRowTransformer(
  columns: ColumnMetadata[],
  seed: string,
  deterministic: boolean
): (row: string[], rowIndex: number) => string[] {
  return (row: string[], rowIndex: number) => {
    return transformRow(row, columns, rowIndex, seed, deterministic);
  };
}
