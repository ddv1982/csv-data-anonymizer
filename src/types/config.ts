/**
 * Options passed to the processing pipeline.
 */
export interface ProcessOptions {
  /** Use deterministic transformations */
  deterministic: boolean;
  /** Seed for deterministic mode */
  seed: string;
  /** Progress callback called with processed data row count */
  onProgress?: (rowCount: number) => void;
}

/**
 * Result from the file processing operation.
 */
export interface ProcessResult {
  /** Total number of data rows processed, excluding header */
  rowCount: number;
  /** Whether processing completed successfully */
  success: boolean;
  /** Path to the output file */
  outputPath: string;
  /** Processing duration in milliseconds */
  duration: number;
}

/**
 * Context passed to transformation strategies.
 */
export interface TransformContext {
  /** Name of the column being transformed */
  columnName: string;
  /** Index of the column in the CSV */
  columnIndex: number;
  /** Current row index, excluding header */
  rowIndex: number;
  /** Seed for deterministic transformations */
  seed: string;
  /** Whether to use deterministic mode */
  deterministic: boolean;
  /** Empty value format for this column */
  emptyFormat: 'empty_string' | 'null' | 'mixed';
}

/**
 * Parsed sample from a CSV file.
 */
export interface ParsedSample {
  /** Column header names */
  headers: string[];
  /** Sample data rows, each row as an array of values */
  rows: string[][];
}
