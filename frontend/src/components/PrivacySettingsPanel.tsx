import { ShieldCheck } from 'lucide-react'
import type { ColumnMetadata, DpAggregate, PrivacyConfig, ReleaseMode } from '../types'
import { formatToken } from '../utils/format'
import { SwitchRow } from './SwitchRow'

export function PrivacySettingsPanel({
  config,
  columns,
  disabled,
  onChange,
}: {
  config: PrivacyConfig
  columns: ColumnMetadata[]
  disabled: boolean
  onChange: (config: PrivacyConfig) => void
}) {
  function update(next: Partial<PrivacyConfig>) {
    onChange({ ...config, ...next })
  }

  function updateFormal(next: Partial<PrivacyConfig['formal']>) {
    update({ formal: { ...config.formal, ...next } })
  }

  function updateDp(next: Partial<PrivacyConfig['differentialPrivacy']>) {
    update({ differentialPrivacy: { ...config.differentialPrivacy, ...next } })
  }

  function updateSynthetic(next: Partial<PrivacyConfig['synthetic']>) {
    update({ synthetic: { ...config.synthetic, ...next } })
  }

  return (
    <div className="privacy-config-panel">
      <div className="privacy-config-header">
        <span className="privacy-config-title">
          <ShieldCheck aria-hidden="true" />
          Privacy Release
        </span>
        <select
          value={config.releaseMode}
          disabled={disabled}
          aria-label="Privacy release mode"
          onChange={(event) => update({ releaseMode: event.target.value as ReleaseMode })}
        >
          {releaseModes.map((mode) => (
            <option key={mode} value={mode}>
              {releaseModeLabel(mode)}
            </option>
          ))}
        </select>
      </div>

      {config.releaseMode === 'formalTabular' ? (
        <div className="settings-grid">
          <NumberField
            id="privacy-k"
            label="k"
            min={1}
            max={1000000}
            integer
            value={config.formal.k}
            disabled={disabled}
            onChange={(value) => updateFormal({ k: value })}
          />
          <NullableNumberField
            id="privacy-l"
            label="l-diversity"
            min={1}
            max={1000000}
            integer
            value={config.formal.lDiversity}
            disabled={disabled}
            onChange={(value) => updateFormal({ lDiversity: value })}
          />
          <NullableNumberField
            id="privacy-t"
            label="t-closeness"
            min={0}
            max={1}
            step={0.01}
            value={config.formal.tCloseness}
            disabled={disabled}
            onChange={(value) => updateFormal({ tCloseness: value })}
          />
          <SwitchRow
            id="privacy-suppress"
            label="Suppress small classes"
            checked={config.formal.suppressSmallClasses}
            disabled={disabled}
            compact
            onChange={(checked) => updateFormal({ suppressSmallClasses: checked })}
          />
        </div>
      ) : null}

      {config.releaseMode === 'differentialPrivacyAggregate' ? (
        <div className="settings-grid">
          <NumberField
            id="privacy-epsilon"
            label="Epsilon"
            min={0.01}
            max={1000}
            step={0.01}
            value={config.differentialPrivacy.epsilon}
            disabled={disabled}
            onChange={(value) => updateDp({ epsilon: value })}
          />
          <div className="field">
            <label htmlFor="privacy-aggregate">Aggregate</label>
            <select
              id="privacy-aggregate"
              value={config.differentialPrivacy.aggregate}
              disabled={disabled}
              onChange={(event) => updateDp({ aggregate: event.target.value as DpAggregate })}
            >
              {aggregates.map((aggregate) => (
                <option key={aggregate} value={aggregate}>
                  {formatToken(aggregate)}
                </option>
              ))}
            </select>
          </div>
          <ColumnSelect
            id="privacy-group-column"
            label="Group column"
            columns={columns}
            value={config.differentialPrivacy.groupByColumn}
            disabled={disabled}
            allowNone
            onChange={(value) => updateDp({ groupByColumn: value })}
          />
          <ColumnSelect
            id="privacy-value-column"
            label="Value column"
            columns={columns}
            value={config.differentialPrivacy.valueColumn}
            disabled={disabled || config.differentialPrivacy.aggregate === 'count'}
            allowNone
            onChange={(value) => updateDp({ valueColumn: value })}
          />
          {config.differentialPrivacy.aggregate === 'sum' || config.differentialPrivacy.aggregate === 'mean' ? (
            <>
              <NullableNumberField
                id="privacy-lower-bound"
                label="Lower bound"
                value={config.differentialPrivacy.lowerBound}
                disabled={disabled}
                onChange={(value) => updateDp({ lowerBound: value })}
              />
              <NullableNumberField
                id="privacy-upper-bound"
                label="Upper bound"
                value={config.differentialPrivacy.upperBound}
                disabled={disabled}
                onChange={(value) => updateDp({ upperBound: value })}
              />
            </>
          ) : null}
        </div>
      ) : null}

      {config.releaseMode === 'syntheticData' ? (
        <div className="settings-grid">
          <NullableNumberField
            id="privacy-synthetic-rows"
            label="Rows"
            min={0}
            max={1000000}
            integer
            value={config.synthetic.rowCount}
            disabled={disabled}
            onChange={(value) => updateSynthetic({ rowCount: value })}
          />
          <NullableNumberField
            id="privacy-synthetic-epsilon"
            label="DP epsilon"
            min={0.01}
            max={1000}
            step={0.01}
            value={config.synthetic.epsilon}
            disabled={disabled}
            onChange={(value) => updateSynthetic({ epsilon: value })}
          />
        </div>
      ) : null}
    </div>
  )
}

const releaseModes: ReleaseMode[] = ['standard', 'formalTabular', 'differentialPrivacyAggregate', 'syntheticData']
const aggregates: DpAggregate[] = ['count', 'sum', 'mean']

function releaseModeLabel(mode: ReleaseMode) {
  if (mode === 'formalTabular') return 'k/l/t tabular'
  if (mode === 'differentialPrivacyAggregate') return 'DP aggregate'
  if (mode === 'syntheticData') return 'Synthetic data'
  return 'Standard masking'
}

function NumberField({
  id,
  label,
  min,
  max,
  step,
  integer,
  value,
  disabled,
  onChange,
}: {
  id: string
  label: string
  min: number
  max: number
  step?: number
  integer?: boolean
  value: number
  disabled: boolean
  onChange: (value: number) => void
}) {
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="number"
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(coerceNumber(event.target.valueAsNumber, min, max, integer))}
      />
    </div>
  )
}

function NullableNumberField({
  id,
  label,
  min = Number.NEGATIVE_INFINITY,
  max = Number.POSITIVE_INFINITY,
  step,
  integer,
  value,
  disabled,
  onChange,
}: {
  id: string
  label: string
  min?: number
  max?: number
  step?: number
  integer?: boolean
  value: number | null
  disabled: boolean
  onChange: (value: number | null) => void
}) {
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        type="number"
        min={Number.isFinite(min) ? min : undefined}
        max={Number.isFinite(max) ? max : undefined}
        step={step}
        value={value ?? ''}
        disabled={disabled}
        onChange={(event) => {
          if (event.target.value.trim() === '') {
            onChange(null)
            return
          }
          const nextValue = event.target.valueAsNumber
          if (!Number.isFinite(nextValue)) {
            onChange(null)
            return
          }
          onChange(coerceNumber(nextValue, min, max, integer))
        }}
      />
    </div>
  )
}

function coerceNumber(value: number, min: number, max: number, integer = false) {
  if (!Number.isFinite(value)) return Number.isFinite(min) ? min : 0
  const rounded = integer ? Math.trunc(value) : value
  return Math.min(max, Math.max(min, rounded))
}

function ColumnSelect({
  id,
  label,
  columns,
  value,
  disabled,
  allowNone,
  onChange,
}: {
  id: string
  label: string
  columns: ColumnMetadata[]
  value: number | null
  disabled: boolean
  allowNone: boolean
  onChange: (value: number | null) => void
}) {
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={value ?? ''}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value === '' ? null : Number(event.target.value))}
      >
        {allowNone ? <option value="">None</option> : null}
        {columns.map((column) => (
          <option key={column.index} value={column.index}>
            {column.name}
          </option>
        ))}
      </select>
    </div>
  )
}
