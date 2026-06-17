/**
 * Column Type Detection Engine
 * Analyzes sample values to infer column data types, confidence levels,
 * PII risk classification, and empty value format detection.
 */

import type { DataType, Confidence, PiiRisk, EmptyFormat, DetectionResult } from '../types/index.js';
import {
  EMAIL_PATTERN,
  UUID_PATTERN,
  TIMESTAMP_PATTERN,
  NUMERIC_ID_PATTERN,
  COUNTRY_CODE_PATTERN,
  PHONE_PATTERN,
} from '../utils/patterns.js';

/**
 * Checks if a value is considered empty
 */
function isEmptyValue(value: string): boolean {
  return value === '' || value.toLowerCase() === 'null';
}

/**
 * Tests a pattern against non-empty values and returns match count
 */
function countPatternMatches(values: string[], pattern: RegExp): number {
  return values.filter(v => !isEmptyValue(v) && pattern.test(v)).length;
}

/**
 * Calculates confidence level based on match percentage
 * High: ≥80% match
 * Medium: 50-80% match
 * Low: <50% match
 */
function calculateConfidence(matchCount: number, totalNonEmpty: number): Confidence {
  if (totalNonEmpty === 0) return 'low';
  const percentage = (matchCount / totalNonEmpty) * 100;
  if (percentage >= 80) return 'high';
  if (percentage >= 50) return 'medium';
  return 'low';
}

/**
 * Detects if values represent an enum type
 * Enum is detected when:
 * - Unique non-empty values ≤20
 * - Total non-empty samples >10
 */
function detectEnumType(nonEmptyValues: string[]): boolean {
  if (nonEmptyValues.length <= 10) return false;
  const uniqueValues = new Set(nonEmptyValues);
  return uniqueValues.size <= 20;
}

/**
 * Type detection priority order (highest priority first)
 * - email/UUID are checked first (most distinctive patterns)
 * - timestamp/phone next
 * - numeric_id/country_code last (more generic patterns)
 */
const DETECTION_PRIORITY: Array<{ type: DataType; pattern: RegExp }> = [
  { type: 'email', pattern: EMAIL_PATTERN },
  { type: 'uuid', pattern: UUID_PATTERN },
  { type: 'timestamp', pattern: TIMESTAMP_PATTERN },
  { type: 'phone', pattern: PHONE_PATTERN },
  { type: 'numeric_id', pattern: NUMERIC_ID_PATTERN },
  { type: 'country_code', pattern: COUNTRY_CODE_PATTERN },
];

/**
 * Detects the data type of a column based on sample values.
 *
 * @param values - Array of sample values from the column
 * @returns Detection result with type, confidence, and match statistics
 */
export function detectColumnType(values: string[]): DetectionResult {
  const nonEmptyValues = values.filter(v => !isEmptyValue(v));
  const totalNonEmpty = nonEmptyValues.length;

  // If all values are empty, return unknown with low confidence
  if (totalNonEmpty === 0) {
    return {
      type: 'unknown',
      confidence: 'low',
      sampleMatches: 0,
      totalSamples: values.length,
    };
  }

  // Try each pattern in priority order
  for (const { type, pattern } of DETECTION_PRIORITY) {
    const matchCount = countPatternMatches(values, pattern);
    const confidence = calculateConfidence(matchCount, totalNonEmpty);

    // If we have medium or high confidence, accept this type
    if (confidence !== 'low') {
      return {
        type,
        confidence,
        sampleMatches: matchCount,
        totalSamples: values.length,
      };
    }
  }

  // Check for enum type (after all pattern-based detection)
  if (detectEnumType(nonEmptyValues)) {
    return {
      type: 'enum',
      confidence: 'high',
      sampleMatches: nonEmptyValues.length,
      totalSamples: values.length,
    };
  }

  // Fallback to 'string' type with low confidence
  return {
    type: 'string',
    confidence: 'low',
    sampleMatches: nonEmptyValues.length,
    totalSamples: values.length,
  };
}

/**
 * Classifies PII risk level based on detected data type.
 *
 * Risk levels:
 * - High: email, phone, full_name (directly identifying)
 * - Medium: first_name, last_name, uuid, numeric_id (potentially identifying)
 * - Low: timestamp, country_code, enum, string, unknown (generally not identifying)
 *
 * @param type - The detected data type
 * @returns PII risk classification
 */
export function classifyPiiRisk(type: DataType): PiiRisk {
  switch (type) {
    case 'email':
    case 'phone':
    case 'full_name':
      return 'high';

    case 'first_name':
    case 'last_name':
    case 'uuid':
    case 'numeric_id':
      return 'medium';

    case 'timestamp':
    case 'country_code':
    case 'enum':
    case 'string':
    case 'unknown':
    default:
      return 'low';
  }
}

/**
 * Detects how empty values are represented in a column.
 *
 * Formats:
 * - 'empty_string': All empty values are empty strings ('')
 * - 'null': All empty values are the string 'null' (case insensitive)
 * - 'mixed': Column contains both empty strings and 'null' strings
 *
 * @param values - Array of sample values from the column
 * @returns The detected empty value format
 */
export function detectEmptyFormat(values: string[]): EmptyFormat {
  let hasEmptyString = false;
  let hasNullString = false;

  for (const value of values) {
    if (value === '') {
      hasEmptyString = true;
    } else if (value.toLowerCase() === 'null') {
      hasNullString = true;
    }

    // If we've found both types, it's mixed
    if (hasEmptyString && hasNullString) {
      return 'mixed';
    }
  }

  // Determine format based on what we found
  if (hasNullString && !hasEmptyString) {
    return 'null';
  }

  // Default to empty_string (includes case where no empty values exist)
  return 'empty_string';
}
