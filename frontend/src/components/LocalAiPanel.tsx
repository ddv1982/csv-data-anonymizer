import { Cpu, Download, ExternalLink, RefreshCw, X } from 'lucide-react'
import type { LocalAiDownloadStatus, LocalAiStatus } from '../types'
import { SwitchRow } from './SwitchRow'

export function LocalAiPanel({
  enabled,
  model,
  status,
  downloadStatus,
  disabled,
  onToggle,
  onModelChange,
  onRefresh,
  onDownload,
  onCancelDownload,
  onOpenSetup,
}: {
  enabled: boolean
  model: string
  status: LocalAiStatus | null
  downloadStatus: LocalAiDownloadStatus | null
  disabled: boolean
  onToggle: (checked: boolean) => void
  onModelChange: (model: string) => void
  onRefresh: () => void
  onDownload: () => void
  onCancelDownload: () => void
  onOpenSetup: () => void
}) {
  const ready = Boolean(status?.ready)
  const downloading = downloadStatus?.state === 'running'
  const progress = downloadProgress(downloadStatus)

  return (
    <div className="local-ai-panel">
      <div className="local-ai-header">
        <div>
          <div className="local-ai-title">
            <Cpu aria-hidden="true" />
            <span>Local AI</span>
            <span className={ready ? 'status-pill success' : 'status-pill'}>{ready ? 'Ready' : 'Setup needed'}</span>
          </div>
          <p className="muted-text text-sm">
            Optional smart replacements using Gemma 3 4B through Ollama on this device.
          </p>
        </div>
        <button type="button" className="button button-ghost button-icon" disabled={disabled} onClick={onRefresh} aria-label="Refresh Local AI status">
          <RefreshCw aria-hidden="true" />
        </button>
      </div>

      <SwitchRow
        id="local-ai-enabled"
        label="Use Local AI"
        description="Enable Smart replacement for selected columns. CSV rows are sent only to localhost."
        checked={enabled}
        disabled={disabled}
        compact
        onChange={onToggle}
      />

      <div className={enabled ? 'settings-grid local-ai-grid' : 'settings-grid local-ai-grid disabled-soft'}>
        <div className="field">
          <label htmlFor="local-ai-model">Model</label>
          <input
            id="local-ai-model"
            value={model}
            disabled={disabled || !enabled}
            placeholder="gemma3:4b"
            onChange={(event) => onModelChange(event.target.value)}
          />
        </div>
        <div className="local-ai-actions">
          {!status?.runtimeAvailable ? (
            <button type="button" className="button button-outline button-sm" disabled={disabled} onClick={onOpenSetup}>
              <ExternalLink aria-hidden="true" />
              Install Ollama
            </button>
          ) : null}
          <button
            type="button"
            className="button button-outline button-sm"
            disabled={disabled || !enabled || downloading || !status?.runtimeAvailable}
            onClick={onDownload}
          >
            <Download aria-hidden="true" />
            Download Gemma
          </button>
          {downloading ? (
            <button type="button" className="button button-ghost button-sm" disabled={disabled} onClick={onCancelDownload}>
              <X aria-hidden="true" />
              Cancel
            </button>
          ) : null}
        </div>
      </div>

      {downloadStatus ? (
        <p className="muted-text text-sm">
          {downloadStatus.statusMessage}
          {progress ? ` (${progress})` : ''}
        </p>
      ) : (
        <p className="muted-text text-sm">{status?.message ?? 'Checking Local AI status...'}</p>
      )}
    </div>
  )
}

function downloadProgress(status: LocalAiDownloadStatus | null) {
  if (!status?.completedBytes || !status.totalBytes) {
    return ''
  }
  return `${Math.round((status.completedBytes / status.totalBytes) * 100)}%`
}
