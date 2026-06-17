/**
 * Column Metadata Builder
 * Combines type detection, PII classification, and sample values into complete column metadata.
 */

import type { ColumnMetadata, DetectionResult } from '../types/column.js';
import { detectColumnType, classifyPiiRisk, detectEmptyFormat } from './detector.js';

/**
 * Default number of sample values to store in metadata
 */
const DEFAULT_SAMPLE_COUNT = 5;

/**
 * Extracts column values from sample rows.
 *
 * @param rows - Sample data rows
 * @param columnIndex - Index of the column to extract
 * @returns Array of values for the column
 */
function extractColumnValues(rows: string[][], columnIndex: number): string[] {
  return rows.map(row => row[columnIndex] ?? '');
}

/**
 * Builds metadata for a single column.
 *
 * @param name - Column header name
 * @param index - Column index (0-based)
 * @param values - Sample values from the column
 * @param sampleCount - Number of sample values to include in metadata
 * @returns Complete column metadata
 */
function buildSingleColumnMetadata(
  name: string,
  index: number,
  values: string[],
  sampleCount: number = DEFAULT_SAMPLE_COUNT
): ColumnMetadata {
  // Detect column type
  const detection: DetectionResult = detectColumnType(values);

  // Classify PII risk based on detected type
  const piiRisk = classifyPiiRisk(detection.type);

  // Detect empty value format
  const emptyFormat = detectEmptyFormat(values);

  // Get sample values (non-empty, limited count)
  const nonEmptyValues = values.filter(v => v !== '' && v.toLowerCase() !== 'null');
  const sampleValues = nonEmptyValues.slice(0, sampleCount);

  return {
    name,
    index,
    detectedType: detection.type,
    confidence: detection.confidence,
    piiRisk,
    sampleValues,
    emptyFormat,
    isSelected: false, // Default to not selected, UI will set this
  };
}

/**
 * Builds complete metadata for all columns in a CSV.
 *
 * @param headers - Column header names
 * @param samples - Sample data rows (each row is array of values)
 * @param sampleCount - Number of sample values to include per column
 * @returns Array of ColumnMetadata for each column
 */
export function buildColumnMetadata(
  headers: string[],
  samples: string[][],
  sampleCount: number = DEFAULT_SAMPLE_COUNT
): ColumnMetadata[] {
  return headers.map((header, index) => {
    const values = extractColumnValues(samples, index);
    return buildSingleColumnMetadata(header, index, values, sampleCount);
  });
}

/**
 * Updates metadata with user selection.
 *
 * @param metadata - Array of column metadata
 * @param selectedIndices - Indices of columns selected for anonymization
 * @returns Updated metadata array with isSelected flags set
 */
export function applyColumnSelection(
  metadata: ColumnMetadata[],
  selectedIndices: number[]
): ColumnMetadata[] {
  const selectedSet = new Set(selectedIndices);

  return metadata.map(col => ({
    ...col,
    isSelected: selectedSet.has(col.index),
  }));
}

/**
 * Gets columns that are selected for anonymization.
 *
 * @param metadata - Array of column metadata
 * @returns Array of selected columns only
 */
export function getSelectedColumns(metadata: ColumnMetadata[]): ColumnMetadata[] {
  return metadata.filter(col => col.isSelected);
}

/**
 * Gets columns with high PII risk.
 *
 * @param metadata - Array of column metadata
 * @returns Array of high-risk columns
 */
export function getHighRiskColumns(metadata: ColumnMetadata[]): ColumnMetadata[] {
  return metadata.filter(col => col.piiRisk === 'high');
}

/**
 * Auto-selects columns based on PII risk level.
 * Selects columns with high or medium PII risk.
 *
 * @param metadata - Array of column metadata
 * @returns Updated metadata with auto-selection applied
 */
export function autoSelectPiiColumns(metadata: ColumnMetadata[]): ColumnMetadata[] {
  return metadata.map(col => ({
    ...col,
    isSelected: col.piiRisk === 'high' || col.piiRisk === 'medium',
  }));
}
