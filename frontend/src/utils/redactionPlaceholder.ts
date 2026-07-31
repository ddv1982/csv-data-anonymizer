import type { ColumnMetadata } from '../types'

export function columnRedactionPlaceholder(column: ColumnMetadata) {
  return column.evidenceProfile.redactionDecision.placeholder
}
