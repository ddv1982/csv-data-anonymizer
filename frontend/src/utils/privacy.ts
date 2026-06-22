import type { PrivacyConfig } from '../types'

export function validatePrivacyConfig(config: PrivacyConfig) {
  if (config.releaseMode === 'standard') return true
  if (config.releaseMode === 'formalTabular') {
    return (
      config.formal.k >= 1 &&
      (config.formal.lDiversity === null || config.formal.lDiversity >= 1) &&
      (config.formal.tCloseness === null ||
        (config.formal.tCloseness >= 0 && config.formal.tCloseness <= 1))
    )
  }
  if (config.releaseMode === 'differentialPrivacyAggregate') {
    if (config.differentialPrivacy.epsilon <= 0) return false
    if (config.differentialPrivacy.aggregate === 'count') return true
    const hasValueColumn = config.differentialPrivacy.valueColumn !== null
    const lower = config.differentialPrivacy.lowerBound
    const upper = config.differentialPrivacy.upperBound
    return hasValueColumn && lower !== null && upper !== null && lower <= upper
  }
  if (config.releaseMode === 'syntheticData') {
    return (
      (config.synthetic.rowCount === null || config.synthetic.rowCount >= 0) &&
      (config.synthetic.epsilon === null || config.synthetic.epsilon > 0)
    )
  }
  return false
}
