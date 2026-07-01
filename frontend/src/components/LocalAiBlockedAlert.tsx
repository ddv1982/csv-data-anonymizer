import { AlertCircle } from 'lucide-react'
import { Alert } from './Alert'

export function LocalAiBlockedAlert({
  message,
  onOpenSettings,
}: {
  message: string
  onOpenSettings: () => void
}) {
  return (
    <Alert icon={<AlertCircle aria-hidden="true" />}>
      <div className="alert-line">
        <span>{message}</span>
        <button type="button" className="button button-outline button-sm" onClick={onOpenSettings}>
          Open Local AI settings
        </button>
      </div>
    </Alert>
  )
}
