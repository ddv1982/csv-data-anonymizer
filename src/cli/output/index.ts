// CLI output module exports

// Formatting utilities
export {
  formatPiiRisk,
  formatDataType,
  formatColumnName,
  formatValue,
  formatError,
  formatSuccess,
  formatWarning,
  formatInfo,
  padString,
  formatColumnLine,
  formatColumnTable,
  formatPreviewTransform,
  formatColumnPreview,
  drawDivider,
  drawBox,
  formatFileSize,
  formatDuration,
  formatRowCount,
} from './format.js';

// Progress tracking
export {
  ProgressTracker,
  createSpinner,
  createProgressCallback,
} from './progress.js';
