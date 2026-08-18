import type { AnonymizationStrategy, DataType } from './types'
import { formatToken } from './utils/format'

export const dataTypes: DataType[] = [
  'email',
  'uuid',
  'timestamp',
  'numericId',
  'numericValue',
  'postalCode',
  'address',
  'ipAddress',
  'url',
  'macAddress',
  'taxId',
  'boolean',
  'currency',
  'percentage',
  'countryCode',
  'phone',
  'firstName',
  'lastName',
  'fullName',
  'enum',
  'string',
  'unknown',
]

const smartReplacementStrategies: AnonymizationStrategy[] = [
  'auto',
  'pseudonymize',
  'tokenize',
  'localAi',
]

/**
 * Every strategy a column can be given. CSV and paste share this list so a new
 * strategy cannot appear in one workflow and not the other.
 */
export const columnStrategies: AnonymizationStrategy[] = [
  ...smartReplacementStrategies,
  'mask',
  'label',
  'redact',
  'passThrough',
]

export const quickGenerateStrategies: AnonymizationStrategy[] = smartReplacementStrategies

export function strategyLabel(strategy: AnonymizationStrategy) {
  if (strategy === 'localAi') {
    return 'Smart replacement (Local AI)'
  }
  if (strategy === 'redact') {
    return 'Redact'
  }
  if (strategy === 'label') {
    return 'Label with column name'
  }
  return formatToken(strategy)
}
