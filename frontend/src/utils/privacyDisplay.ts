import type { GlossaryKey } from '../glossary'
import type { PrivacyConfig, ReleaseMode } from '../types'

export const releaseModes: ReleaseMode[] = [
  'standard',
  'formalTabular',
  'differentialPrivacyAggregate',
  'syntheticData',
]

export function releaseModeLabel(mode: ReleaseMode) {
  if (mode === 'formalTabular') return 'k/l/t tabular'
  if (mode === 'differentialPrivacyAggregate') return 'DP aggregate'
  if (mode === 'syntheticData') return 'Synthetic data'
  return 'Standard CSV transform'
}

export function releaseModeGlossaryTerm(mode: ReleaseMode): GlossaryKey {
  if (mode === 'formalTabular') return 'formalTabular'
  if (mode === 'differentialPrivacyAggregate') return 'dpAggregate'
  if (mode === 'syntheticData') return 'syntheticData'
  return 'standardMasking'
}

export function releaseModeHelp(mode: ReleaseMode) {
  if (mode === 'formalTabular') {
    return 'Row-level output with Direct IDs redacted, Quasi-IDs generalized, and k/l/t checks reported.'
  }
  if (mode === 'differentialPrivacyAggregate') {
    return 'No row-level output: writes noisy aggregate result rows. Repeatable deterministic output is not available; release history can block or warn when cumulative epsilon exceeds a local limit.'
  }
  if (mode === 'syntheticData') {
    return 'Sampled test data from a simple generator, without a DP synthetic guarantee.'
  }
  return 'Row-level output where selected columns are transformed in place using Strategy settings.'
}

export function dpBudgetProjection(config: PrivacyConfig) {
  if (config.releaseMode !== 'differentialPrivacyAggregate') return null
  const budget = config.differentialPrivacy.budget
  const limit = budget.limitEpsilon
  if (!budget.enabled || limit === null || !Number.isFinite(limit) || limit <= 0) return null
  const spentAfter = budget.spentEpsilon + config.differentialPrivacy.epsilon
  if (!Number.isFinite(spentAfter)) return null
  return {
    limit,
    spentAfter,
    overLimit: spentAfter > limit,
  }
}

export function formatBudgetNumber(value: number) {
  return value.toLocaleString(undefined, { maximumFractionDigits: 3 })
}

export function parsePublicGroupValues(value: string) {
  return value
    .split(/[\n,]/)
    .map((entry) => entry.trim())
}
