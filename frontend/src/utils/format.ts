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

type TransformStats = {
  rowCount: number
  columnsAnonymized: number
  durationMs: number
}

/**
 * The one-line "what just happened" summary shown under any transform result.
 *
 * `unit` is a caller-supplied noun rather than a hard-coded "column" because the
 * paste workflow anonymizes fields of a JSON or log record, not CSV columns, and
 * calling those columns would describe something the user never gave us.
 */
export function formatTransformStats(result: TransformStats, options: { unit?: string } = {}) {
  const unit = options.unit ?? 'column'
  const rows = result.rowCount.toLocaleString()
  const unitText = result.columnsAnonymized === 1 ? unit : `${unit}s`
  const duration = result.durationMs < 1000 ? `${result.durationMs}ms` : `${(result.durationMs / 1000).toFixed(2)}s`
  return `${rows} rows processed, ${result.columnsAnonymized} ${unitText} transformed in ${duration}`
}
