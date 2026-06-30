import type { AppSettings } from './types'

export const defaultSettings: AppSettings = {
  schemaVersion: 10,
  themeMode: 'system',
  overwriteOutput: false,
  sampleRowCount: 100,
  previewSampleCount: 5,
  defaultOutputSuffix: '_private_output',
  rememberLastPaths: true,
  lastInputDirectory: null,
  lastOutputDirectory: null,
  localAiEnabled: false,
  localAiModel: 'gemma3:4b',
}
