import type { GlossaryKey } from '../../glossary'
import type { ColumnMetadata } from '../../types'
import { GlossaryPopover } from '../GlossaryPopover'

export function NumberField({
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

export function NullableNumberField({
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

export function ColumnSelect({
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

export function FieldLabel({ id, label, glossaryTerm }: { id: string; label: string; glossaryTerm?: GlossaryKey }) {
  return (
    <span className="field-label-row">
      <label htmlFor={id}>{label}</label>
      {glossaryTerm ? <GlossaryPopover term={glossaryTerm} /> : null}
    </span>
  )
}

function coerceNumber(value: number, min: number, max: number, integer = false) {
  if (!Number.isFinite(value)) return Number.isFinite(min) ? min : 0
  const rounded = integer ? Math.trunc(value) : value
  return Math.min(max, Math.max(min, rounded))
}
