import type { AnonymizeData } from '../types'

export function formatRowCount(headers: { rowCount: number; rowCountIsComplete: boolean }) {
  const rows = headers.rowCount.toLocaleString()
  return headers.rowCountIsComplete ? `${rows} rows` : `${rows}+ sampled rows`
}

export function formatToken(value: string) {
  return value
    .replace(/([A-Z])/g, ' $1')
    .replace(/^./, (first) => first.toUpperCase())
    .trim()
}

export function formatResultStats(result: AnonymizeData) {
  const rows = result.rowCount.toLocaleString()
  const colText = result.columnsAnonymized === 1 ? 'column' : 'columns'
  const duration = result.durationMs < 1000 ? `${result.durationMs}ms` : `${(result.durationMs / 1000).toFixed(2)}s`
  if (result.privacyReport.releaseMode === 'differentialPrivacyAggregate') {
    return `${rows} aggregate rows released, ${result.columnsAnonymized} input ${colText} used in ${duration}`
  }
  return `${rows} rows processed, ${result.columnsAnonymized} ${colText} transformed in ${duration}`
}
