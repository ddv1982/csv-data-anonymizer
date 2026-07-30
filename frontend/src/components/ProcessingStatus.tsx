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
  const cancelRequested = Boolean(status?.cancelRequested)
  const stateLabel = cancelRequested ? 'Canceling' : 'Working'
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
        {/*
          Stays enabled once cancellation is requested, and only changes its label.

          `cancelRequested` is not a terminal state: the backend sets the flag and leaves
          the job Running until the worker reaches its next cancellation check, which can
          be a long operation away. Disabling here left the only enabled control in this
          view dead — the surrounding step gates everything else on the run being busy —
          so a cancel that never took (a rejected request, a wedged worker) stranded the
          user with no way to ask again and no way out short of killing the app.

          Asking again is safe: `cancel_anonymize_job` returns early for terminal jobs, and
          `request_cancel` only stores `true` into an already-`true` AtomicBool and re-sets
          `cancel_requested` on a Running status, so repeats are idempotent, not a queue of
          work. The label is the button's accessible name too, so the state change is
          announced rather than conveyed by the disabled styling alone.
        */}
        <button
          type="button"
          className="button button-outline button-sm"
          onClick={onCancel}
        >
          <X aria-hidden="true" />
          {cancelRequested ? 'Canceling…' : 'Cancel'}
        </button>
      </div>
    </div>
  )
}
