/**
 * Data types that can be detected and anonymized
 */
export type DataType =
  | 'email'
  | 'uuid'
  | 'timestamp'
  | 'numeric_id'
  | 'country_code'
  | 'phone'
  | 'first_name'
  | 'last_name'
  | 'full_name'
  | 'enum'
  | 'string'
  | 'unknown';

/**
 * Confidence level for type detection
 */
export type Confidence = 'high' | 'medium' | 'low';

/**
 * PII risk level classification
 */
export type PiiRisk = 'high' | 'medium' | 'low';

/**
 * Format used for empty/null values in a column
 */
export type EmptyFormat = 'empty_string' | 'null' | 'mixed';

/**
 * Result from type detection analysis
 */
export interface DetectionResult {
  type: DataType;
  confidence: Confidence;
  sampleMatches: number;
  totalSamples: number;
}

/**
 * Complete metadata for a CSV column including detection results
 */
export interface ColumnMetadata {
  /** Column header name */
  name: string;
  /** 0-based column index in the CSV */
  index: number;
  /** Auto-detected or manually specified data type */
  detectedType: DataType;
  /** Detection confidence level */
  confidence: Confidence;
  /** PII risk classification based on data type */
  piiRisk: PiiRisk;
  /** Sample values from the column for preview/validation */
  sampleValues: string[];
  /** Format used for empty/null values */
  emptyFormat: EmptyFormat;
  /** Whether this column is selected for anonymization */
  isSelected: boolean;
}

/**
 * Preview of anonymization transformation for a single value
 */
export interface PreviewRow {
  original: string;
  anonymized: string;
}

/**
 * Preview result containing sample transformations for each column
 */
export interface PreviewResult {
  columns: Map<string, PreviewRow[]>;
  sampleCount: number;
}
