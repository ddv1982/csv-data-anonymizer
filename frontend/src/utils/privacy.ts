import type { HeadersData, PrivacyConfig } from '../types'

export interface PrivacyConfigValidation {
  valid: boolean
  reason: string | null
}

const FULL_DATASET_WARNING_ROWS = 100_000
const FULL_DATASET_CAP_ROWS = 1_000_000

export function getPrivacyConfigValidation(
  config: PrivacyConfig,
  selectedColumns?: Set<number>,
  columnCount?: number,
): PrivacyConfigValidation {
  if (config.releaseMode === 'standard') return { valid: true, reason: null }
  if (config.releaseMode === 'formalTabular') {
    if (!Number.isFinite(config.formal.k) || config.formal.k < 1) {
      return { valid: false, reason: 'Set k to 1 or higher for k/l/t tabular output.' }
    }
    if (config.formal.lDiversity !== null && (!Number.isFinite(config.formal.lDiversity) || config.formal.lDiversity < 1)) {
      return { valid: false, reason: 'Set l-diversity to 1 or higher, or leave it empty.' }
    }
    if (
      config.formal.tCloseness !== null &&
      (!Number.isFinite(config.formal.tCloseness) ||
        config.formal.tCloseness < 0 ||
        config.formal.tCloseness > 1)
    ) {
      return { valid: false, reason: 'Set t-closeness between 0 and 1, or leave it empty.' }
    }
    if ((config.formal.lDiversity !== null || config.formal.tCloseness !== null) && !hasSensitiveColumn(config, selectedColumns)) {
      return { valid: false, reason: 'Mark a selected column as Sensitive for l-diversity/t-closeness.' }
    }
    return { valid: true, reason: null }
  }
  if (config.releaseMode === 'differentialPrivacyAggregate') {
    if (!Number.isFinite(config.differentialPrivacy.epsilon) || config.differentialPrivacy.epsilon <= 0) {
      return { valid: false, reason: 'Set DP epsilon above 0 for aggregate output.' }
    }
    if (config.differentialPrivacy.groupByColumn === null) {
      if (config.differentialPrivacy.publicGroupValues.length > 0) {
        return { valid: false, reason: 'Clear allowed group values or choose a DP group column.' }
      }
    } else {
      if (selectedColumns && !selectedColumns.has(config.differentialPrivacy.groupByColumn)) {
        return { valid: false, reason: 'Select the DP group column or clear it before creating output.' }
      }
      if (!isAttributeRole(config, config.differentialPrivacy.groupByColumn)) {
        return { valid: false, reason: 'Mark the DP group column as Attribute before creating output.' }
      }
      if (!config.differentialPrivacy.groupLabelsPublic) {
        return {
          valid: false,
          reason: 'Mark DP group labels as public before creating grouped aggregate output.',
        }
      }
      if (config.differentialPrivacy.publicGroupValues.filter((value) => value.trim()).length === 0) {
        return {
          valid: false,
          reason: 'Add allowed group values before creating grouped DP output.',
        }
      }
      if (config.differentialPrivacy.publicGroupValues.some((value) => !value.trim())) {
        return {
          valid: false,
          reason: 'Remove blank entries from allowed group values before creating grouped DP output.',
        }
      }
    }
    if (
      config.differentialPrivacy.aggregate === 'count' &&
      config.differentialPrivacy.valueColumn !== null
    ) {
      return { valid: false, reason: 'Clear the value column before creating DP count output.' }
    }
    if (
      config.differentialPrivacy.maxContributionsPerUnit !== null &&
      config.differentialPrivacy.privacyUnitColumn === null
    ) {
      return {
        valid: false,
        reason: 'Choose a privacy unit column before setting a contribution limit.',
      }
    }
    if (
      config.differentialPrivacy.maxContributionsPerUnit !== null &&
      (!Number.isFinite(config.differentialPrivacy.maxContributionsPerUnit) ||
        config.differentialPrivacy.maxContributionsPerUnit < 1)
    ) {
      return { valid: false, reason: 'Set max contributions per unit to 1 or higher, or leave it empty.' }
    }
    if (
      selectedColumns &&
      config.differentialPrivacy.privacyUnitColumn !== null &&
      !selectedColumns.has(config.differentialPrivacy.privacyUnitColumn)
    ) {
      return { valid: false, reason: 'Select the DP privacy unit column or clear it before creating output.' }
    }
    if (config.differentialPrivacy.budget.enabled) {
      const budget = config.differentialPrivacy.budget
      if (budget.limitEpsilon === null || !Number.isFinite(budget.limitEpsilon) || budget.limitEpsilon <= 0) {
        return { valid: false, reason: 'Set a DP budget limit above 0, or turn off budget tracking.' }
      }
    }
    if (config.differentialPrivacy.aggregate === 'count') return { valid: true, reason: null }
    const hasValueColumn = config.differentialPrivacy.valueColumn !== null
    const lower = config.differentialPrivacy.lowerBound
    const upper = config.differentialPrivacy.upperBound
    if (!hasValueColumn) {
      return { valid: false, reason: 'Choose a numeric value column for DP sum or mean output.' }
    }
    if (
      selectedColumns &&
      config.differentialPrivacy.valueColumn !== null &&
      !selectedColumns.has(config.differentialPrivacy.valueColumn)
    ) {
      return { valid: false, reason: 'Select the DP value column before creating sum or mean output.' }
    }
    if (lower === null || upper === null || !Number.isFinite(lower) || !Number.isFinite(upper)) {
      return { valid: false, reason: 'Set public lower and upper bounds for DP sum or mean output.' }
    }
    if (lower > upper) {
      return { valid: false, reason: 'Set the DP lower bound less than or equal to the upper bound.' }
    }
    return { valid: true, reason: null }
  }
  if (config.releaseMode === 'syntheticData') {
    if (config.synthetic.rowCount !== null && (!Number.isFinite(config.synthetic.rowCount) || config.synthetic.rowCount < 0)) {
      return { valid: false, reason: 'Set synthetic row count to 0 or higher, or leave it empty.' }
    }
    if (config.synthetic.epsilon !== null) {
      return {
        valid: false,
        reason: 'Clear synthetic DP epsilon before creating output; this generator does not provide a DP synthetic guarantee.',
      }
    }
    if (selectedColumns && columnCount !== undefined && selectedColumns.size < columnCount) {
      return {
        valid: false,
        reason: 'Select every CSV column for synthetic data; this mode does not write unselected source columns into the output.',
      }
    }
    return { valid: true, reason: null }
  }
  return { valid: false, reason: 'Choose a supported privacy release mode.' }
}

export function getPrivacyScaleWarning(
  config: PrivacyConfig,
  headers: HeadersData | null,
): string | null {
  if (config.releaseMode === 'standard' || !headers) return null
  const mode = privacyReleaseModeLabel(config.releaseMode)

  if (!headers.rowCountIsComplete) {
    return `${mode} materializes the privacy release dataset in memory. Exact row count is still being calculated; inputs over ${FULL_DATASET_CAP_ROWS.toLocaleString()} data rows are blocked.`
  }

  if (headers.rowCount > FULL_DATASET_WARNING_ROWS) {
    return `${mode} will materialize ${headers.rowCount.toLocaleString()} data rows in memory. Inputs over ${FULL_DATASET_CAP_ROWS.toLocaleString()} data rows are blocked.`
  }

  return null
}

function hasSensitiveColumn(config: PrivacyConfig, selectedColumns?: Set<number>) {
  return config.columnRoles.some(
    (role) => role.role === 'sensitive' && (!selectedColumns || selectedColumns.has(role.columnIndex)),
  )
}

function isAttributeRole(config: PrivacyConfig, columnIndex: number) {
  return config.columnRoles.some((role) => role.columnIndex === columnIndex && role.role === 'attribute')
}

function privacyReleaseModeLabel(mode: PrivacyConfig['releaseMode']) {
  if (mode === 'formalTabular') return 'k/l/t tabular output'
  if (mode === 'differentialPrivacyAggregate') return 'DP aggregate output'
  if (mode === 'syntheticData') return 'Synthetic data output'
  return 'This release mode'
}
