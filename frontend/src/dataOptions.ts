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

export const csvStrategies: AnonymizationStrategy[] = [
  ...smartReplacementStrategies,
  'mask',
  'redact',
  'passThrough',
]

export const directInputStrategies: AnonymizationStrategy[] = [
  ...smartReplacementStrategies,
  'mask',
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
  return formatToken(strategy)
}
