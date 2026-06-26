import { Settings } from 'lucide-react'
import type { LocalAiState } from '../hooks/useLocalAi'
import type { AppSettings } from '../types'
import { LocalAiSettingsBlock } from './LocalAiSettingsBlock'
import { ModalDialog } from './ModalDialog'

export function LocalAiTopbarControl({
  settings,
  localAi,
  disabled,
  settingsOpen,
  onToggleSettings,
  onUpdateSetting,
}: {
  settings: AppSettings
  localAi: LocalAiState
  disabled: boolean
  settingsOpen: boolean
  onToggleSettings: (open: boolean) => void
  onUpdateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void
}) {
  const status = localAiTopbarStatus(settings, localAi)

  return (
    <>
      <div className="local-ai-topbar-control" role="group" aria-label="Local AI">
        <button
          type="button"
          role="switch"
          aria-checked={settings.localAiEnabled}
          aria-label="Use Local AI"
          title="Use Local AI"
          className={settings.localAiEnabled ? 'switch checked' : 'switch'}
          disabled={disabled}
          onClick={() => onUpdateSetting('localAiEnabled', !settings.localAiEnabled)}
        >
          <span />
        </button>
        <span className="local-ai-topbar-name">Local AI</span>
        <span className={status.ready ? 'status-pill success' : 'status-pill'}>{status.label}</span>
        <button
          type="button"
          className="button button-ghost button-icon local-ai-settings-button"
          aria-label="Open Local AI settings"
          aria-haspopup="dialog"
          aria-expanded={settingsOpen}
          title="Open Local AI settings"
          disabled={disabled}
          onClick={() => onToggleSettings(true)}
        >
          <Settings aria-hidden="true" />
        </button>
      </div>
      <ModalDialog
        open={settingsOpen}
        title="Local AI Settings"
        eyebrow="Local LLM"
        closeLabel="Close Local AI settings"
        className="local-ai-settings-modal"
        bodyClassName="local-ai-settings-modal-body"
        onClose={() => onToggleSettings(false)}
      >
        <LocalAiSettingsBlock
          settings={settings}
          localAi={localAi}
          disabled={disabled}
          onUpdateSetting={onUpdateSetting}
        />
      </ModalDialog>
    </>
  )
}

function localAiTopbarStatus(settings: AppSettings, localAi: LocalAiState) {
  if (!settings.localAiEnabled) {
    return { label: 'Off', ready: false }
  }
  if (localAi.downloadRunning) {
    return { label: 'Downloading', ready: false }
  }
  if (localAi.ready) {
    return { label: 'Ready', ready: true }
  }
  if (localAi.status) {
    return { label: 'Setup needed', ready: false }
  }
  return { label: 'Checking', ready: false }
}
