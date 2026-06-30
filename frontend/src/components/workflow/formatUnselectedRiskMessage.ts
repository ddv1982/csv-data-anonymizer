export function formatUnselectedRiskMessage(columnNames: string[]) {
  const count = columnNames.length
  const shownNames = columnNames.slice(0, 3).join(', ')
  const extraCount = Math.max(count - 3, 0)
  const suffix = extraCount > 0 ? `, and ${extraCount} more` : ''
  const columnText = count === 1 ? 'column is' : 'columns are'
  const prefix = `${count} ${columnText} flagged high or medium risk by detection (${shownNames}${suffix}).`
  return `${prefix} Unselected row-level columns are written unchanged.`
}
