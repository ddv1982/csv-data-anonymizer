import { ChevronDown } from 'lucide-react'
import { useState } from 'react'
import { maxPreviewSampleCount, maxSampleRowCount } from '../defaults'
import type { AppSettings } from '../types'
import { clampNumber } from '../utils/numbers'
import { copyTextToClipboard } from '../utils/clipboard'
import { getTokenizationKeyError } from '../utils/tokenizationKey'
import { SwitchRow } from './SwitchRow'

export function AppSettingsPanel({
  settings,
  open,
  disabled,
  onToggleOpen,
  onUpdateSetting,
  tokenizationKey,
  onTokenizationKeyChange,
}: {
  settings: AppSettings
  open: boolean
  disabled?: boolean
  onToggleOpen: () => void
  onUpdateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void
  tokenizationKey?: string | null
  onTokenizationKeyChange?: (key: string | null) => void
}) {
  const [showTokenizationKey, setShowTokenizationKey] = useState(false)
  const [copyStatus, setCopyStatus] = useState<'copied' | 'failed' | null>(null)
  const tokenizationKeyError = getTokenizationKeyError(tokenizationKey)

  const updateTokenizationKey = (key: string | null) => {
    setCopyStatus(null)
    onTokenizationKeyChange?.(key)
  }

  const copyTokenizationKey = async () => {
    if (!tokenizationKey) return
    try {
      await copyTextToClipboard(tokenizationKey)
      setCopyStatus('copied')
    } catch {
      setCopyStatus('failed')
    }
  }
  return (
    <div className="collapsible">
      <div className="collapsible-header">
        <button
          type="button"
          className="button button-ghost settings-trigger"
          disabled={disabled}
          onClick={onToggleOpen}
          aria-expanded={open}
        >
          <span>App Settings</span>
          <ChevronDown className={open ? 'chevron open' : 'chevron'} aria-hidden="true" />
        </button>
      </div>
      {open ? (
        <div className="settings-panel">
          {onTokenizationKeyChange ? (
            <div className="field">
              <label htmlFor="tokenization-key">Repeatable token key</label>
              <input
                id="tokenization-key"
                type={showTokenizationKey ? 'text' : 'password'}
                autoComplete="off"
                spellCheck={false}
                maxLength={64}
                placeholder="Disabled — tokens vary between runs"
                value={tokenizationKey ?? ''}
                disabled={disabled}
                aria-invalid={tokenizationKeyError ? true : undefined}
                aria-describedby={tokenizationKeyError ? 'tokenization-key-help tokenization-key-error' : 'tokenization-key-help'}
                onChange={(event) => updateTokenizationKey(event.target.value || null)}
              />
              <p id="tokenization-key-help" className="muted-text text-sm">
                Memory-only 256-bit key. The same key links tokenized values across releases that keep the same column name and position; losing it prevents reproduction.
              </p>
              {tokenizationKeyError ? <p id="tokenization-key-error" className="danger-text text-sm" role="alert">{tokenizationKeyError}</p> : null}
              <div className="button-row">
                <button type="button" className="button button-outline button-sm" disabled={disabled} onClick={() => updateTokenizationKey(generateTokenizationKey())}>
                  Generate key
                </button>
                {tokenizationKey ? <button type="button" className="button button-outline button-sm" disabled={disabled || Boolean(tokenizationKeyError)} onClick={() => void copyTokenizationKey()}>Copy key</button> : null}
                {tokenizationKey ? <button type="button" className="button button-ghost button-sm" disabled={disabled} onClick={() => setShowTokenizationKey((visible) => !visible)}>{showTokenizationKey ? 'Hide key' : 'Show key'}</button> : null}
                {tokenizationKey ? <button type="button" className="button button-ghost button-sm" disabled={disabled} onClick={() => updateTokenizationKey(null)}>Clear key</button> : null}
              </div>
              <p className={copyStatus === 'failed' ? 'danger-text text-sm' : 'muted-text text-sm'} role="status" aria-live="polite">
                {copyStatus === 'copied' ? 'Key copied to clipboard.' : copyStatus === 'failed' ? 'Could not copy the key. Select and copy it manually.' : ''}
              </p>
            </div>
          ) : null}
          <SwitchRow
            id="overwrite-output"
            label="Overwrite Output"
            description="Replace the output file when it already exists."
            checked={settings.overwriteOutput}
            disabled={disabled}
            onChange={(checked) => onUpdateSetting('overwriteOutput', checked)}
          />
          <SwitchRow
            id="local-ner"
            label="Local AI detection"
            description="Optional. Uses the configured Ollama model to suggest additional sensitive fields for review. It may be unavailable, and suggestions are never selected automatically."
            checked={settings.localNerEnabled}
            disabled={disabled}
            onChange={(checked) => onUpdateSetting('localNerEnabled', checked)}
          />
          <div className="settings-grid">
            <div className="field">
              <label htmlFor="output-suffix">Output suffix</label>
              <input
                id="output-suffix"
                value={settings.defaultOutputSuffix}
                disabled={disabled}
                onChange={(event) => onUpdateSetting('defaultOutputSuffix', event.target.value)}
              />
            </div>
            <div className="field">
              <label htmlFor="sample-rows">Sample rows</label>
              <input
                id="sample-rows"
                type="number"
                min={1}
                max={maxSampleRowCount}
                value={settings.sampleRowCount}
                disabled={disabled}
                onChange={(event) =>
                  onUpdateSetting(
                    'sampleRowCount',
                    clampNumber(event.target.valueAsNumber, 1, maxSampleRowCount),
                  )
                }
              />
            </div>
            <div className="field">
              <label htmlFor="preview-rows">Preview rows</label>
              <input
                id="preview-rows"
                type="number"
                min={1}
                max={maxPreviewSampleCount}
                value={settings.previewSampleCount}
                disabled={disabled}
                onChange={(event) =>
                  onUpdateSetting(
                    'previewSampleCount',
                    clampNumber(event.target.valueAsNumber, 1, maxPreviewSampleCount),
                  )
                }
              />
            </div>
            <SwitchRow
              id="remember-paths"
              label="Remember paths"
              checked={settings.rememberLastPaths}
              disabled={disabled}
              compact
              onChange={(checked) => onUpdateSetting('rememberLastPaths', checked)}
            />
          </div>
        </div>
      ) : null}
    </div>
  )
}

function generateTokenizationKey() {
  const bytes = new Uint8Array(32)
  crypto.getRandomValues(bytes)
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('')
}
