import { Cpu, Download, ExternalLink, RefreshCw, X } from 'lucide-react'
import { defaultLocalAiModel } from '../defaults'
import type { LocalAiDownloadStatus, LocalAiStatus } from '../types'
import { GlossaryLabel, GlossaryPopover } from './GlossaryPopover'
import { SectionHelp } from './SectionHelp'
import { SwitchRow } from './SwitchRow'

export function LocalAiPanel({
  enabled,
  model,
  selectedModel,
  statusMatchesModel,
  ready,
  downloading,
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
  selectedModel: string
  statusMatchesModel: boolean
  ready: boolean
  downloading: boolean
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
  const availableModels = status?.availableModels ?? []
  const selectedModelAvailable = availableModels.includes(selectedModel)
  const modelInstalled = selectedModelAvailable || Boolean(statusMatchesModel && status?.modelInstalled)
  const modelOptions = uniqueModels([defaultLocalAiModel, ...availableModels])
  const progress = downloadProgress(downloadStatus)
  const statusMessage = status && !statusMatchesModel ? `Checking ${selectedModel} in Ollama...` : status?.message
  const downloadLabel = selectedModel === defaultLocalAiModel ? 'Download Gemma' : `Download ${selectedModel}`

  return (
    <div className="local-ai-panel">
      <div className="local-ai-header">
        <div>
          <div className="panel-title-row">
            <div className="local-ai-title">
              <Cpu aria-hidden="true" />
              <span>Local AI</span>
              <span className={ready ? 'status-pill success' : 'status-pill'}>{ready ? 'Ready' : 'Setup needed'}</span>
            </div>
            <SectionHelp topic="localAi" label="How Local AI works" />
          </div>
          <p className="muted-text text-sm">
            Optional <GlossaryLabel term="smartReplacement">smart replacements</GlossaryLabel> using{' '}
            <GlossaryLabel term="gemma">Gemma 3 4B</GlossaryLabel> through{' '}
            <GlossaryLabel term="ollama">Ollama</GlossaryLabel> on this device.
          </p>
        </div>
        <button type="button" className="button button-ghost button-icon" disabled={disabled} onClick={onRefresh} aria-label="Refresh Local AI status">
          <RefreshCw aria-hidden="true" />
        </button>
      </div>

      <SwitchRow
        id="local-ai-enabled"
        label="Use Local AI"
        description={
          <>
            Enable <GlossaryLabel term="smartReplacement">Smart replacement</GlossaryLabel> for selected columns. CSV
            rows are sent only to <GlossaryLabel term="localhost">localhost</GlossaryLabel>.
          </>
        }
        checked={enabled}
        disabled={disabled}
        compact
        onChange={onToggle}
      />

      <div className={enabled ? 'settings-grid local-ai-grid' : 'settings-grid local-ai-grid disabled-soft'}>
        <div className="field">
          <span className="field-label-row">
            <label htmlFor="local-ai-model">Model</label>
            <GlossaryPopover term="model" />
          </span>
          <div className="local-ai-model-control">
            <input
              id="local-ai-model"
              list="local-ai-model-options"
              value={model}
              disabled={disabled || !enabled}
              placeholder={defaultLocalAiModel}
              onChange={(event) => onModelChange(event.target.value)}
            />
            {modelInstalled ? <span className="local-ai-installed">Installed locally</span> : null}
          </div>
          <datalist id="local-ai-model-options">
            {modelOptions.map((option) => (
              <option key={option} value={option} />
            ))}
          </datalist>
          <p className="muted-text text-sm">Recommended lightweight default: {defaultLocalAiModel}.</p>
        </div>
        <div className="local-ai-actions">
          {status && !status.runtimeAvailable ? (
            <button type="button" className="button button-outline button-sm" disabled={disabled} onClick={onOpenSetup}>
              <ExternalLink aria-hidden="true" />
              Install Ollama
            </button>
          ) : null}
          {status?.runtimeAvailable && !modelInstalled ? (
            <button
              type="button"
              className="button button-outline button-sm"
              disabled={disabled || !enabled || downloading}
              onClick={onDownload}
            >
              <Download aria-hidden="true" />
              {downloadLabel}
            </button>
          ) : null}
          {downloading ? (
            <button type="button" className="button button-ghost button-sm" disabled={disabled} onClick={onCancelDownload}>
              <X aria-hidden="true" />
              Cancel
            </button>
          ) : null}
        </div>
      </div>

      {status?.runtimeAvailable ? (
        <div className="local-ai-model-list" aria-live="polite">
          <span className="muted-text text-sm">Available local models</span>
          {availableModels.length > 0 ? (
            <div className="local-ai-model-options">
              {availableModels.map((availableModel) => (
                <button
                  key={availableModel}
                  type="button"
                  className={availableModel === selectedModel ? 'local-ai-model-option selected' : 'local-ai-model-option'}
                  disabled={disabled || !enabled}
                  aria-pressed={availableModel === selectedModel}
                  onClick={() => onModelChange(availableModel)}
                >
                  {availableModel}
                </button>
              ))}
            </div>
          ) : (
            <p className="muted-text text-sm">No local Ollama models found yet.</p>
          )}
        </div>
      ) : null}

      {downloadStatus ? (
        <p className="muted-text text-sm">
          {downloadStatus.statusMessage}
          {progress ? ` (${progress})` : ''}
        </p>
      ) : (
        <p className="muted-text text-sm">{statusMessage ?? 'Checking Local AI status...'}</p>
      )}
    </div>
  )
}

function uniqueModels(models: string[]) {
  return Array.from(new Set(models.filter(Boolean)))
}

function downloadProgress(status: LocalAiDownloadStatus | null) {
  if (!status?.completedBytes || !status.totalBytes) {
    return ''
  }
  return `${Math.round((status.completedBytes / status.totalBytes) * 100)}%`
}
