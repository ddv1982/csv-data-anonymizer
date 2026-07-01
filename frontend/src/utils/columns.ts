export const maxVisibleColumns = 50

export function hasSampleData(column: { sampleValues: string[] }) {
  return column.sampleValues.length > 0
}
