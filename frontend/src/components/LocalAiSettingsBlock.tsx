import type { AppSettings } from '../types'
import type { LocalAiState } from '../hooks/useLocalAi'
import { LocalAiPanel } from './LocalAiPanel'

export function LocalAiSettingsBlock({
  settings,
  localAi,
  disabled,
  onUpdateSetting,
}: {
  settings: AppSettings
  localAi: LocalAiState
  disabled: boolean
  onUpdateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void
}) {
  return (
    <LocalAiPanel
      enabled={settings.localAiEnabled}
      model={settings.localAiModel}
      status={localAi.status}
      downloadStatus={localAi.downloadStatus}
      disabled={disabled}
      onToggle={(checked) => onUpdateSetting('localAiEnabled', checked)}
      onModelChange={(model) => onUpdateSetting('localAiModel', model)}
      onRefresh={() => void localAi.refresh()}
      onDownload={() => void localAi.startDownload()}
      onCancelDownload={() => void localAi.cancelDownload()}
      onOpenSetup={() => void localAi.openSetup()}
    />
  )
}
