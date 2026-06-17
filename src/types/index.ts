// Type definitions module exports

// Column-related types
export type {
  DataType,
  Confidence,
  PiiRisk,
  EmptyFormat,
  DetectionResult,
  ColumnMetadata,
  PreviewRow,
  PreviewResult,
} from './column.js';

// Configuration types
export type {
  ProcessOptions,
  ProcessResult,
  TransformContext,
  ParsedSample,
} from './config.js';

// Error types
export {
  ErrorCodes,
  AnonymizerError,
  FileNotFoundError,
  CsvParseError,
  ColumnNotFoundError,
  OutputExistsError,
  InvalidSelectionError,
} from './errors.js';

export type { ErrorCode } from './errors.js';
