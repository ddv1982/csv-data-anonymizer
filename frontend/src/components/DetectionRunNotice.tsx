import { AlertCircle } from 'lucide-react'
import type { ColumnMetadata, DetectionRunSummary } from '../types'
import { Alert } from './Alert'

export function DetectionRunNotice({
  summary,
  columns,
}: {
  summary?: DetectionRunSummary
  columns: ColumnMetadata[]
}) {
  if (!summary || summary.localNer === 'disabled') return null

  const reviewCount = columns.filter((column) => column.reviewReasons?.length).length
  if (summary.localNer === 'completed' && reviewCount === 0) return null

  const message = summary.message?.trim()
  const text = summary.localNer === 'completed'
    ? `Local AI detection marked ${reviewCount} ${reviewCount === 1 ? 'column' : 'columns'} for review. These suggestions are never selected automatically; select each column you want to transform.`
    : 'Local detection was not completed. Rule-based detection still ran; review unselected columns.'

  return (
    <Alert icon={<AlertCircle aria-hidden="true" />}>
      <strong>{text}</strong>
      {message ? ` ${message}` : null}
    </Alert>
  )
}
