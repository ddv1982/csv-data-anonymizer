import { RefreshCcw } from 'lucide-react'
import type { DpBudgetAction, DpBudgetConfig } from '../../types'
import { GlossaryPopover } from '../GlossaryPopover'
import { SwitchRow } from '../SwitchRow'
import { FieldLabel, NullableNumberField, NumberField } from './PrivacyFieldControls'

export function DpBudgetSettings({
  budget,
  disabled,
  onResetBudget,
  onChange,
}: {
  budget: DpBudgetConfig
  disabled: boolean
  onResetBudget: () => void
  onChange: (config: Partial<DpBudgetConfig>) => void
}) {
  return (
    <>
      <SwitchRow
        id="privacy-dp-budget-enabled"
        label="Track DP budget"
        labelHelp={<GlossaryPopover term="dpBudget" />}
        checked={budget.enabled}
        disabled={disabled}
        compact
        onChange={(checked) => onChange({ enabled: checked })}
      />
      {budget.enabled ? (
        <>
          <NullableNumberField
            id="privacy-dp-budget-limit"
            label="Budget limit"
            glossaryTerm="dpBudgetLimit"
            min={0.01}
            max={1000000}
            step={0.01}
            value={budget.limitEpsilon}
            disabled={disabled}
            onChange={(value) => onChange({ limitEpsilon: value })}
          />
          <NumberField
            id="privacy-dp-budget-spent"
            label="Spent epsilon"
            glossaryTerm="dpBudgetSpent"
            min={0}
            max={1000000}
            step={0.01}
            value={budget.spentEpsilon}
            disabled
            onChange={() => undefined}
          />
          <button
            type="button"
            className="button button-outline button-sm"
            disabled={disabled || budget.spentEpsilon === 0}
            onClick={onResetBudget}
          >
            <RefreshCcw aria-hidden="true" />
            Reset budget
          </button>
          <div className="field">
            <FieldLabel id="privacy-dp-budget-action" label="Over budget" glossaryTerm="dpBudgetAction" />
            <select
              id="privacy-dp-budget-action"
              value={budget.action}
              disabled={disabled}
              onChange={(event) => onChange({ action: event.target.value as DpBudgetAction })}
            >
              <option value="block">Block release</option>
              <option value="warn">Warn only</option>
            </select>
          </div>
        </>
      ) : null}
    </>
  )
}
