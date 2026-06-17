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
  StrategyOptions,
  ColumnConfig,
  AnonymizationConfig,
  ProcessOptions,
  ProcessResult,
  TransformContext,
  AnonymizeCommandOptions,
  ParsedSample,
} from './config.js';

// Error types
export {
  ErrorCodes,
  AnonymizerError,
  FileNotFoundError,
  CsvParseError,
  ConfigValidationError,
  ColumnNotFoundError,
  OutputExistsError,
  InvalidSelectionError,
} from './errors.js';

export type { ErrorCode } from './errors.js';
