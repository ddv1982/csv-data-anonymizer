import type { ColumnMetadata } from '../types'

export const maxVisibleColumns = 50

export function isSelectableColumn(_column: ColumnMetadata) {
  return true
}

export function hasSampleData(column: ColumnMetadata) {
  return column.sampleValues.length > 0
}
