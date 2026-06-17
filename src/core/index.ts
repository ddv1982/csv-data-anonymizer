// Core processing module exports

export {
  detectColumnType,
  classifyPiiRisk,
  detectEmptyFormat,
} from './detector.js';

// File reading utilities
export {
  validateFile,
  stripBom,
  readFileContent,
  getFileSize,
} from './fileReader.js';

export type { FileStats, FileValidationResult } from './fileReader.js';

// CSV sample reading
export { readSample, readAllRows } from './sampleReader.js';

export type { SampleReadOptions } from './sampleReader.js';

// Column metadata building
export {
  buildColumnMetadata,
  applyColumnSelection,
  getSelectedColumns,
  getHighRiskColumns,
  autoSelectPiiColumns,
} from './metadataBuilder.js';

// Value transformation
export {
  transformValue,
  createTransformContext,
  transformRow,
  createRowTransformer,
} from './transformer.js';

// File processing
export { processFile, processFileSimple } from './processor.js';
