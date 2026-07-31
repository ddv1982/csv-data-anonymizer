import type { AppSettings } from './types'

export const defaultLocalAiModel = 'gemma3:4b'

/**
 * Mirrors `MAX_SAMPLE_ROW_COUNT` in the core crate, which is the single ceiling
 * every workflow honours. `scripts/check-contracts.mjs` compares the two numbers,
 * because a settings panel offering a value the engine rejects is the drift this
 * pair used to have.
 */
export const maxSampleRowCount = 10000

/** Mirrors `MAX_PREVIEW_SAMPLE_COUNT` in the core crate. See `maxSampleRowCount`. */
export const maxPreviewSampleCount = 100

export const defaultSettings: AppSettings = {
  schemaVersion: 11,
  themeMode: 'system',
  overwriteOutput: false,
  sampleRowCount: 100,
  previewSampleCount: 5,
  defaultOutputSuffix: '_private_output',
  rememberLastPaths: true,
  lastInputDirectory: null,
  lastOutputDirectory: null,
  localAiEnabled: false,
  localAiModel: defaultLocalAiModel,
  localNerEnabled: false,
}
