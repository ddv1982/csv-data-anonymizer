import type { AppSettings, PrivacyConfig } from './types'

export const defaultSettings: AppSettings = {
  schemaVersion: 2,
  deterministicDefault: false,
  seed: '',
  overwriteOutput: false,
  sampleRowCount: 100,
  previewSampleCount: 5,
  defaultOutputSuffix: '_anonymized',
  rememberLastPaths: true,
  lastInputDirectory: null,
  lastOutputDirectory: null,
  localAiEnabled: false,
  localAiModel: 'gemma3:4b',
}

export const defaultPrivacyConfig: PrivacyConfig = {
  releaseMode: 'standard',
  columnRoles: [],
  formal: {
    k: 5,
    lDiversity: null,
    tCloseness: null,
    suppressSmallClasses: true,
  },
  differentialPrivacy: {
    epsilon: 1,
    aggregate: 'count',
    groupByColumn: null,
    valueColumn: null,
    lowerBound: null,
    upperBound: null,
  },
  synthetic: {
    rowCount: null,
    epsilon: null,
  },
}
