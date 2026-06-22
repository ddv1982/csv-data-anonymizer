import type { AppSettings, PrivacyConfig } from './types'

export const defaultSettings: AppSettings = {
  schemaVersion: 5,
  deterministicDefault: false,
  seed: '',
  overwriteOutput: false,
  sampleRowCount: 100,
  previewSampleCount: 5,
  defaultOutputSuffix: '_private_output',
  dpBudgetEnabled: true,
  dpBudgetLimitEpsilon: 10,
  dpBudgetSpentEpsilon: 0,
  dpBudgetAction: 'block',
  dpReleaseHistory: [],
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
    groupLabelsPublic: false,
    publicGroupValues: [],
    valueColumn: null,
    lowerBound: null,
    upperBound: null,
    privacyUnitColumn: null,
    maxContributionsPerUnit: null,
    budget: {
      enabled: false,
      limitEpsilon: null,
      spentEpsilon: 0,
      action: 'block',
    },
  },
  synthetic: {
    rowCount: null,
    epsilon: null,
  },
}

export function privacyConfigFromSettings(settings: AppSettings): PrivacyConfig {
  return applyBudgetSettingsToPrivacyConfig(defaultPrivacyConfig, settings)
}

export function applyBudgetSettingsToPrivacyConfig(config: PrivacyConfig, settings: AppSettings): PrivacyConfig {
  return {
    ...config,
    differentialPrivacy: {
      ...config.differentialPrivacy,
      budget: {
        enabled: settings.dpBudgetEnabled,
        limitEpsilon: settings.dpBudgetLimitEpsilon,
        spentEpsilon: settings.dpBudgetSpentEpsilon,
        action: settings.dpBudgetAction,
      },
    },
  }
}

export function settingsWithPrivacyBudget(settings: AppSettings, config: PrivacyConfig): AppSettings {
  return {
    ...settings,
    dpBudgetEnabled: config.differentialPrivacy.budget.enabled,
    dpBudgetLimitEpsilon: config.differentialPrivacy.budget.limitEpsilon,
    dpBudgetSpentEpsilon: settings.dpBudgetSpentEpsilon,
    dpBudgetAction: config.differentialPrivacy.budget.action,
  }
}
