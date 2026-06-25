import { ClipboardList, FileSpreadsheet, ListChecks } from 'lucide-react'

export type InputMode = 'csv' | 'paste' | 'quick'

const inputModes: Array<{
  id: InputMode
  label: string
  icon: typeof FileSpreadsheet
}> = [
  { id: 'csv', label: 'CSV File', icon: FileSpreadsheet },
  { id: 'paste', label: 'Paste Data', icon: ClipboardList },
  { id: 'quick', label: 'Quick by Data Type', icon: ListChecks },
]

export function InputModeTabs({
  activeMode,
  onChange,
}: {
  activeMode: InputMode
  onChange: (mode: InputMode) => void
}) {
  return (
    <div className="mode-tabs" role="tablist" aria-label="Anonymization input method">
      {inputModes.map((mode, index) => {
        const Icon = mode.icon
        const selected = mode.id === activeMode
        return (
          <button
            key={mode.id}
            id={`input-mode-tab-${mode.id}`}
            type="button"
            role="tab"
            className={`mode-tab${selected ? ' active' : ''}`}
            aria-selected={selected}
            aria-controls={`input-mode-panel-${mode.id}`}
            tabIndex={selected ? 0 : -1}
            onClick={() => onChange(mode.id)}
            onKeyDown={(event) => {
              if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return
              event.preventDefault()
              const nextIndex =
                event.key === 'Home'
                  ? 0
                  : event.key === 'End'
                    ? inputModes.length - 1
                    : event.key === 'ArrowLeft'
                      ? (index - 1 + inputModes.length) % inputModes.length
                      : (index + 1) % inputModes.length
              onChange(inputModes[nextIndex].id)
              document.getElementById(`input-mode-tab-${inputModes[nextIndex].id}`)?.focus()
            }}
          >
            <Icon aria-hidden="true" />
            <span>{mode.label}</span>
          </button>
        )
      })}
    </div>
  )
}
