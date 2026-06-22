import { AlertTriangle } from 'lucide-react'
import type { PrivacyConfig } from '../../types'
import { Alert } from '../Alert'
import { NullableNumberField } from './PrivacyFieldControls'

export function SyntheticDataSettings({
  config,
  disabled,
  onChange,
}: {
  config: PrivacyConfig['synthetic']
  disabled: boolean
  onChange: (config: Partial<PrivacyConfig['synthetic']>) => void
}) {
  return (
    <>
      {config.epsilon !== null ? (
        <Alert variant="destructive" icon={<AlertTriangle aria-hidden="true" />}>
          A stale synthetic epsilon setting is still present. Clear it from the saved config to create sampled test data;
          this mode has no DP guarantee.
        </Alert>
      ) : null}
      <div className="settings-grid">
        <NullableNumberField
          id="privacy-synthetic-rows"
          label="Rows"
          glossaryTerm="syntheticRowCount"
          min={0}
          max={1000000}
          integer
          value={config.rowCount}
          disabled={disabled}
          onChange={(value) => onChange({ rowCount: value })}
        />
      </div>
    </>
  )
}
