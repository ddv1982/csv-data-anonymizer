// Configuration module exports

export {
  DataTypeSchema,
  StrategyOptionsSchema,
  ColumnConfigSchema,
  ConfigSchema,
  validateConfig,
  safeValidateConfig,
} from './schema.js';

export type {
  DataTypeSchemaType,
  StrategyOptionsSchemaType,
  ColumnConfigSchemaType,
  ConfigSchemaType,
} from './schema.js';

// Config loader
export { loadConfig } from './loader.js';

// Default configuration
export {
  DEFAULT_CONFIG,
  generateRandomSeed,
  generateDefaultOutputPath,
  mergeConfig,
  applyDefaults,
} from './defaults.js';

// Column selection parser
export { parseColumnSelection } from './selection.js';
