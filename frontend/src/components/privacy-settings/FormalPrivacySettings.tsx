import type { PrivacyConfig } from '../../types'
import { GlossaryPopover } from '../GlossaryPopover'
import { SwitchRow } from '../SwitchRow'
import { NullableNumberField, NumberField } from './PrivacyFieldControls'

export function FormalPrivacySettings({
  config,
  disabled,
  onChange,
}: {
  config: PrivacyConfig['formal']
  disabled: boolean
  onChange: (config: Partial<PrivacyConfig['formal']>) => void
}) {
  return (
    <div className="settings-grid">
      <NumberField
        id="privacy-k"
        label="k"
        glossaryTerm="kAnonymity"
        min={1}
        max={1000000}
        integer
        value={config.k}
        disabled={disabled}
        onChange={(value) => onChange({ k: value })}
      />
      <NullableNumberField
        id="privacy-l"
        label="l-diversity"
        glossaryTerm="lDiversity"
        min={1}
        max={1000000}
        integer
        value={config.lDiversity}
        disabled={disabled}
        onChange={(value) => onChange({ lDiversity: value })}
      />
      <NullableNumberField
        id="privacy-t"
        label="t-closeness"
        glossaryTerm="tCloseness"
        min={0}
        max={1}
        step={0.01}
        value={config.tCloseness}
        disabled={disabled}
        onChange={(value) => onChange({ tCloseness: value })}
      />
      <SwitchRow
        id="privacy-suppress"
        label="Suppress small classes"
        labelHelp={<GlossaryPopover term="suppressSmallClasses" />}
        checked={config.suppressSmallClasses}
        disabled={disabled}
        compact
        onChange={(checked) => onChange({ suppressSmallClasses: checked })}
      />
    </div>
  )
}
