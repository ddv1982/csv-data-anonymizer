export function SwitchRow({
  id,
  label,
  description,
  checked,
  disabled,
  compact = false,
  onChange,
}: {
  id: string
  label: string
  description?: string
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
        <label htmlFor={id}>{label}</label>
        {description ? <p className="muted-text text-sm">{description}</p> : null}
      </div>
    </div>
  )
}
