export function formatUnselectedRiskMessage(columnNames: string[], releaseMode: string) {
  const count = columnNames.length
  const shownNames = columnNames.slice(0, 3).join(', ')
  const extraCount = Math.max(count - 3, 0)
  const suffix = extraCount > 0 ? `, and ${extraCount} more` : ''
  const columnText = count === 1 ? 'column is' : 'columns are'
  const prefix = `${count} ${columnText} flagged high or medium risk by detection (${shownNames}${suffix}).`
  if (releaseMode === 'differentialPrivacyAggregate') {
    return `${prefix} DP aggregate output does not include row-level source rows, but configured group/value columns must still be selected.`
  }
  if (releaseMode === 'syntheticData') {
    return `${prefix} Synthetic data requires every CSV column to be selected so source columns are not copied through.`
  }
  return `${prefix} Unselected row-level columns are written unchanged.`
}
