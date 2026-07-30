import { Check, Clipboard, Loader2 } from 'lucide-react'
import { Card } from './Card'

/**
 * Read-only output text with a copy button and a live-region status line.
 *
 * Shared by the paste and quick-generate workflows, which showed the same card with
 * only the title, the textarea label and the stats sentence differing.
 */
export function CopyableOutputCard({
  title,
  outputLabel,
  output,
  stats,
  copying,
  disabled,
  copyStatus,
  onCopy,
}: {
  title: string
  outputLabel: string
  output: string
  stats: string
  copying: boolean
  disabled: boolean
  copyStatus: string | null
  onCopy: () => void
}) {
  return (
    <Card
      title={title}
      action={
        <button type="button" className="button button-outline button-sm" disabled={disabled} onClick={onCopy}>
          {copying ? <Loader2 className="spin" aria-hidden="true" /> : <Clipboard aria-hidden="true" />}
          Copy
        </button>
      }
    >
      <div className="direct-output-stack">
        <textarea className="direct-output" value={output} readOnly aria-label={outputLabel} />
        <div className="direct-output-meta" aria-live="polite">
          <span className="muted-text text-sm">{stats}</span>
          {copyStatus ? (
            <span className="status-pill success">
              <Check aria-hidden="true" />
              {copyStatus}
            </span>
          ) : null}
        </div>
      </div>
    </Card>
  )
}
