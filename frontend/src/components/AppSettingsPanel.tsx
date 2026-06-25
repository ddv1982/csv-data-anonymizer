import { ChevronDown } from 'lucide-react'
import type { AppSettings } from '../types'
import { clampNumber } from '../utils/numbers'
import { SwitchRow } from './SwitchRow'

export function AppSettingsPanel({
  settings,
  open,
  disabled,
  onToggleOpen,
  onUpdateSetting,
}: {
  settings: AppSettings
  open: boolean
  disabled?: boolean
  onToggleOpen: () => void
  onUpdateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void
}) {
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
          <SwitchRow
            id="deterministic-mode"
            label="Repeatable replacements"
            description="Use the same private seed to get the same replacements again."
            checked={settings.deterministicDefault}
            disabled={disabled}
            onChange={(checked) => onUpdateSetting('deterministicDefault', checked)}
          />
          <div className={settings.deterministicDefault ? 'field' : 'field disabled-soft'}>
            <label htmlFor="seed-input">Seed</label>
            <input
              id="seed-input"
              type="text"
              value={settings.seed}
              disabled={disabled || !settings.deterministicDefault}
              placeholder="Enter a private seed"
              aria-describedby="seed-description"
              onChange={(event) => onUpdateSetting('seed', event.target.value)}
            />
            <p id="seed-description" className="muted-text text-sm">
              Useful when multiple files need matching replacements. Keep the seed private.
            </p>
          </div>
          <SwitchRow
            id="overwrite-output"
            label="Overwrite Output"
            description="Replace the output file when it already exists."
            checked={settings.overwriteOutput}
            disabled={disabled}
            onChange={(checked) => onUpdateSetting('overwriteOutput', checked)}
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
                max={10000}
                value={settings.sampleRowCount}
                disabled={disabled}
                onChange={(event) =>
                  onUpdateSetting('sampleRowCount', clampNumber(event.target.valueAsNumber, 1, 10000))
                }
              />
            </div>
            <div className="field">
              <label htmlFor="preview-rows">Preview rows</label>
              <input
                id="preview-rows"
                type="number"
                min={1}
                max={100}
                value={settings.previewSampleCount}
                disabled={disabled}
                onChange={(event) =>
                  onUpdateSetting('previewSampleCount', clampNumber(event.target.valueAsNumber, 1, 100))
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
