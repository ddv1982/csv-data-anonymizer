import type { ColumnMetadata, DpAggregate, PrivacyConfig } from '../../types'
import { formatToken } from '../../utils/format'
import { parsePublicGroupValues } from '../../utils/privacyDisplay'
import { GlossaryPopover } from '../GlossaryPopover'
import { SwitchRow } from '../SwitchRow'
import { DpBudgetSettings } from './DpBudgetSettings'
import { ColumnSelect, FieldLabel, NullableNumberField, NumberField } from './PrivacyFieldControls'

const aggregates: DpAggregate[] = ['count', 'sum', 'mean']

export function DifferentialPrivacySettings({
  config,
  columns,
  disabled,
  onResetBudget,
  onChange,
}: {
  config: PrivacyConfig['differentialPrivacy']
  columns: ColumnMetadata[]
  disabled: boolean
  onResetBudget: () => void
  onChange: (config: Partial<PrivacyConfig['differentialPrivacy']>) => void
}) {
  function updateBudget(next: Partial<PrivacyConfig['differentialPrivacy']['budget']>) {
    onChange({ budget: { ...config.budget, ...next } })
  }

  return (
    <div className="settings-grid">
      <NumberField
        id="privacy-epsilon"
        label="Epsilon"
        glossaryTerm="epsilon"
        min={0.01}
        max={1000}
        step={0.01}
        value={config.epsilon}
        disabled={disabled}
        onChange={(value) => onChange({ epsilon: value })}
      />
      <div className="field">
        <FieldLabel id="privacy-aggregate" label="Aggregate" glossaryTerm="aggregate" />
        <select
          id="privacy-aggregate"
          value={config.aggregate}
          disabled={disabled}
          onChange={(event) => {
            const aggregate = event.target.value as DpAggregate
            onChange({
              aggregate,
              valueColumn: aggregate === 'count' ? null : config.valueColumn,
            })
          }}
        >
          {aggregates.map((aggregate) => (
            <option key={aggregate} value={aggregate}>
              {formatToken(aggregate)}
            </option>
          ))}
        </select>
      </div>
      {config.aggregate === 'sum' || config.aggregate === 'mean' ? (
        <>
          <ColumnSelect
            id="privacy-value-column"
            label="Value column"
            glossaryTerm="valueColumn"
            columns={columns}
            value={config.valueColumn}
            disabled={disabled}
            allowNone
            onChange={(value) => onChange({ valueColumn: value })}
          />
          <NullableNumberField
            id="privacy-lower-bound"
            label="Lower bound"
            glossaryTerm="lowerBound"
            value={config.lowerBound}
            disabled={disabled}
            onChange={(value) => onChange({ lowerBound: value })}
          />
          <NullableNumberField
            id="privacy-upper-bound"
            label="Upper bound"
            glossaryTerm="upperBound"
            value={config.upperBound}
            disabled={disabled}
            onChange={(value) => onChange({ upperBound: value })}
          />
        </>
      ) : null}
      <details className="privacy-advanced-settings">
        <summary>Advanced</summary>
        <div className="settings-grid">
          <ColumnSelect
            id="privacy-group-column"
            label="Group column"
            glossaryTerm="groupColumn"
            columns={columns}
            value={config.groupByColumn}
            disabled={disabled}
            allowNone
            onChange={(value) =>
              onChange({
                groupByColumn: value,
                groupLabelsPublic: value === null ? false : config.groupLabelsPublic,
                publicGroupValues: value === null ? [] : config.publicGroupValues,
              })
            }
          />
          {config.groupByColumn !== null ? (
            <>
              <SwitchRow
                id="privacy-group-labels-public"
                label="Group labels are public"
                labelHelp={<GlossaryPopover term="publicGroupLabels" />}
                checked={config.groupLabelsPublic}
                disabled={disabled}
                compact
                onChange={(checked) => onChange({ groupLabelsPublic: checked })}
              />
              <div className="field">
                <FieldLabel
                  id="privacy-allowed-group-values"
                  label="Allowed group values"
                  glossaryTerm="publicGroupDomain"
                />
                <textarea
                  id="privacy-allowed-group-values"
                  rows={3}
                  value={config.publicGroupValues.join('\n')}
                  disabled={disabled}
                  onChange={(event) => onChange({ publicGroupValues: parsePublicGroupValues(event.target.value) })}
                />
              </div>
            </>
          ) : null}
          <ColumnSelect
            id="privacy-unit-column"
            label="Privacy unit"
            glossaryTerm="privacyUnitColumn"
            columns={columns}
            value={config.privacyUnitColumn}
            disabled={disabled}
            allowNone
            onChange={(value) =>
              onChange({
                privacyUnitColumn: value,
                maxContributionsPerUnit: value === null ? null : (config.maxContributionsPerUnit ?? 1),
              })
            }
          />
          {config.privacyUnitColumn !== null ? (
            <NullableNumberField
              id="privacy-max-contributions"
              label="Max contributions"
              glossaryTerm="maxContributionsPerUnit"
              min={1}
              max={1000000}
              integer
              value={config.maxContributionsPerUnit}
              disabled={disabled}
              onChange={(value) => onChange({ maxContributionsPerUnit: value })}
            />
          ) : null}
          <DpBudgetSettings
            budget={config.budget}
            disabled={disabled}
            onResetBudget={onResetBudget}
            onChange={updateBudget}
          />
        </div>
      </details>
    </div>
  )
}
