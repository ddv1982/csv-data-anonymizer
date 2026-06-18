import { X } from 'lucide-react'
import type { AnonymizeJobStatus } from '../types'

export function ProcessingStatus({
  status,
  fallbackRowCount,
  onCancel,
}: {
  status: AnonymizeJobStatus | null
  fallbackRowCount: number
  onCancel: () => void
}) {
  const rowsProcessed = status?.rowsProcessed ?? 0
  const totalRows = status?.totalRows ?? fallbackRowCount
  const hasTotal = totalRows > 0
  const percent = hasTotal ? Math.min(100, Math.round((rowsProcessed / totalRows) * 100)) : null
  const stateLabel = status?.cancelRequested ? 'Canceling' : 'Working'
  const progressCopy =
    rowsProcessed > 0
      ? `${rowsProcessed.toLocaleString()}${hasTotal ? ` of ${totalRows.toLocaleString()}` : ''} rows processed`
      : hasTotal
        ? `Preparing ${totalRows.toLocaleString()} rows`
        : 'Preparing file'

  return (
    <div className="progress-stack" role="status" aria-live="polite">
      <div className="progress-copy">
        <span className="muted-text text-sm">{progressCopy}</span>
        <span className="text-sm progress-state">{stateLabel}</span>
      </div>
      <div className="progress-track" aria-hidden="true">
        {percent === null ? (
          <span className="progress-bar-indeterminate" />
        ) : (
          <span className="progress-bar-determinate" style={{ width: `${percent}%` }} />
        )}
      </div>
      <div className="progress-actions">
        <button
          type="button"
          className="button button-outline button-sm"
          disabled={status?.cancelRequested}
          onClick={onCancel}
        >
          <X aria-hidden="true" />
          Cancel
        </button>
      </div>
    </div>
  )
}
