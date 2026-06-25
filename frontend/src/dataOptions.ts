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

export const csvStrategies: AnonymizationStrategy[] = [
  'auto',
  'pseudonymize',
  'tokenize',
  'localAi',
  'mask',
  'passThrough',
]

export const standardStrategies: AnonymizationStrategy[] = [
  'auto',
  'pseudonymize',
  'tokenize',
  'mask',
  'passThrough',
]

export const quickGenerateStrategies: AnonymizationStrategy[] = ['auto', 'pseudonymize', 'tokenize']

export function strategyLabel(strategy: AnonymizationStrategy) {
  if (strategy === 'localAi') {
    return 'Smart replacement (Local AI)'
  }
  return formatToken(strategy)
}
