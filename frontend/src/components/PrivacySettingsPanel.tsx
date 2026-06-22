import { ShieldCheck } from 'lucide-react'
import type { GlossaryKey } from '../glossary'
import type { ColumnMetadata, DpAggregate, PrivacyConfig, ReleaseMode } from '../types'
import { formatToken } from '../utils/format'
import { GlossaryLabel, GlossaryPopover } from './GlossaryPopover'
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
          <GlossaryLabel term="releaseMode">Privacy Release</GlossaryLabel>
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
      <p className="privacy-mode-help muted-text text-sm">
        <GlossaryLabel term={releaseModeGlossaryTerm(config.releaseMode)}>
          {releaseModeLabel(config.releaseMode)}
        </GlossaryLabel>
        <span>{releaseModeHelp(config.releaseMode)}</span>
      </p>

      {config.releaseMode === 'formalTabular' ? (
        <div className="settings-grid">
          <NumberField
            id="privacy-k"
            label="k"
            glossaryTerm="kAnonymity"
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
            glossaryTerm="lDiversity"
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
            glossaryTerm="tCloseness"
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
            labelHelp={<GlossaryPopover term="suppressSmallClasses" />}
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
            glossaryTerm="epsilon"
            min={0.01}
            max={1000}
            step={0.01}
            value={config.differentialPrivacy.epsilon}
            disabled={disabled}
            onChange={(value) => updateDp({ epsilon: value })}
          />
          <div className="field">
            <FieldLabel id="privacy-aggregate" label="Aggregate" glossaryTerm="aggregate" />
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
            glossaryTerm="groupColumn"
            columns={columns}
            value={config.differentialPrivacy.groupByColumn}
            disabled={disabled}
            allowNone
            onChange={(value) => updateDp({ groupByColumn: value })}
          />
          <ColumnSelect
            id="privacy-value-column"
            label="Value column"
            glossaryTerm="valueColumn"
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
                glossaryTerm="lowerBound"
                value={config.differentialPrivacy.lowerBound}
                disabled={disabled}
                onChange={(value) => updateDp({ lowerBound: value })}
              />
              <NullableNumberField
                id="privacy-upper-bound"
                label="Upper bound"
                glossaryTerm="upperBound"
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
            glossaryTerm="syntheticEpsilon"
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

function releaseModeGlossaryTerm(mode: ReleaseMode): GlossaryKey {
  if (mode === 'formalTabular') return 'formalTabular'
  if (mode === 'differentialPrivacyAggregate') return 'dpAggregate'
  if (mode === 'syntheticData') return 'syntheticData'
  return 'standardMasking'
}

function releaseModeHelp(mode: ReleaseMode) {
  if (mode === 'formalTabular') {
    return 'Best when you need row-level output plus checks for re-identification risk.'
  }
  if (mode === 'differentialPrivacyAggregate') {
    return 'Best when you only need summary statistics with a formal privacy budget.'
  }
  if (mode === 'syntheticData') {
    return 'Best when consumers need example-like rows instead of original records.'
  }
  return 'Best for row-level files where selected columns should be transformed in place.'
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
  glossaryTerm,
}: {
  id: string
  label: string
  glossaryTerm?: GlossaryKey
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
      <FieldLabel id={id} label={label} glossaryTerm={glossaryTerm} />
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
  glossaryTerm,
}: {
  id: string
  label: string
  glossaryTerm?: GlossaryKey
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
      <FieldLabel id={id} label={label} glossaryTerm={glossaryTerm} />
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
  glossaryTerm,
}: {
  id: string
  label: string
  glossaryTerm?: GlossaryKey
  columns: ColumnMetadata[]
  value: number | null
  disabled: boolean
  allowNone: boolean
  onChange: (value: number | null) => void
}) {
  return (
    <div className="field">
      <FieldLabel id={id} label={label} glossaryTerm={glossaryTerm} />
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

function FieldLabel({ id, label, glossaryTerm }: { id: string; label: string; glossaryTerm?: GlossaryKey }) {
  return (
    <span className="field-label-row">
      <label htmlFor={id}>{label}</label>
      {glossaryTerm ? <GlossaryPopover term={glossaryTerm} /> : null}
    </span>
  )
}
