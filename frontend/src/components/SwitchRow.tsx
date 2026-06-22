import type { ReactNode } from 'react'

export function SwitchRow({
  id,
  label,
  labelHelp,
  description,
  checked,
  disabled,
  compact = false,
  onChange,
}: {
  id: string
  label: ReactNode
  labelHelp?: ReactNode
  description?: ReactNode
  checked: boolean
  disabled?: boolean
  compact?: boolean
  onChange: (checked: boolean) => void
}) {
  return (
    <div className={compact ? 'switch-row compact-switch-row' : 'switch-row'}>
      <button
        id={id}
        type="button"
        role="switch"
        aria-checked={checked}
        className={checked ? 'switch checked' : 'switch'}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        <span />
      </button>
      <div className="switch-copy">
        <span className="field-label-row">
          <label htmlFor={id}>{label}</label>
          {labelHelp}
        </span>
        {description ? <p className="muted-text text-sm">{description}</p> : null}
      </div>
    </div>
  )
}
