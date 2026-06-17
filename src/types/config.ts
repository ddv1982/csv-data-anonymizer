import type { DataType } from './column.js';

/**
 * Options for custom anonymization strategy per column
 */
export interface StrategyOptions {
  /** Use deterministic transformation (same input → same output) */
  deterministic?: boolean;
  /** For emails: preserve the domain part */
  preserveDomain?: boolean;
  /** For numeric IDs: preserve exact digit count */
  preserveDigitCount?: boolean;
  /** For timestamps: preserve microsecond precision */
  preservePrecision?: boolean;
  /** For timestamps: maximum offset in days */
  offsetDays?: number;
}

/**
 * Configuration for a single column's anonymization
 */
export interface ColumnConfig {
  /** Column name (must match CSV header) */
  name: string;
  /** Override auto-detected type */
  type?: DataType;
  /** Custom strategy name */
  strategy?: string;
  /** Strategy-specific options */
  options?: StrategyOptions;
}

/**
 * Complete anonymization configuration (from YAML config file)
 */
export interface AnonymizationConfig {
  /** List of columns to anonymize with their settings */
  columns: ColumnConfig[];
  /** Output file path (default: {input}_anonymized.csv) */
  output?: string;
  /** Use deterministic transformations globally */
  deterministic?: boolean;
  /** Seed for deterministic mode */
  seed?: string;
}

/**
 * Options passed to the processing pipeline
 */
export interface ProcessOptions {
  /** Use deterministic transformations */
  deterministic: boolean;
  /** Seed for deterministic mode */
  seed: string;
  /** Progress callback (called every N rows) */
  onProgress?: (rowCount: number) => void;
}

/**
 * Result from the file processing operation
 */
export interface ProcessResult {
  /** Total number of data rows processed (excluding header) */
  rowCount: number;
  /** Whether processing completed successfully */
  success: boolean;
  /** Path to the output file */
  outputPath: string;
  /** Processing duration in milliseconds */
  duration: number;
}

/**
 * Context passed to transformation strategies
 */
export interface TransformContext {
  /** Name of the column being transformed */
  columnName: string;
  /** Index of the column in the CSV */
  columnIndex: number;
  /** Current row index (0-based, excludes header) */
  rowIndex: number;
  /** Seed for deterministic transformations */
  seed: string;
  /** Whether to use deterministic mode */
  deterministic: boolean;
  /** Empty value format for this column */
  emptyFormat: 'empty_string' | 'null' | 'mixed';
}

/**
 * CLI command options for the anonymize command
 */
export interface AnonymizeCommandOptions {
  /** Output file path */
  output?: string;
  /** YAML config file path */
  config?: string;
  /** Comma-separated column names/indices */
  columns?: string;
  /** Skip interactive prompts */
  noInteractive: boolean;
  /** Show preview only, don't process */
  preview: boolean;
  /** Use deterministic transforms */
  deterministic: boolean;
  /** Seed for deterministic mode */
  seed?: string;
  /** Overwrite output file if exists */
  force: boolean;
  /** Suppress progress output */
  quiet: boolean;
}

/**
 * Parsed sample from CSV file
 */
export interface ParsedSample {
  /** Column header names */
  headers: string[];
  /** Sample data rows (each row is array of values) */
  rows: string[][];
}
