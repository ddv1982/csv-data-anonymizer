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
 * Every strategy a column can be given, for both the CSV and the paste workflow.
 *
 * The two used to be separate byte-identical lists, which meant a strategy added to
 * one silently went missing from the other.
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
