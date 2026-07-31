import type { ColumnMetadata } from '../types'

export function columnRedactionPlaceholder(column: ColumnMetadata) {
  const placeholder = column.evidenceProfile?.redactionDecision?.placeholder
  return typeof placeholder === 'string' && placeholder.length > 0 ? placeholder : null
}
