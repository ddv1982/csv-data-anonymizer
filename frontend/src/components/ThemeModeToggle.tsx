import { Monitor, Moon, Sun, type LucideIcon } from 'lucide-react'
import type { ThemeMode } from '../hooks/useTheme'

interface ThemeModeToggleProps {
  themeMode: ThemeMode
  disabled?: boolean
  onChange: (themeMode: ThemeMode) => void
}

const options = [
  {
    value: 'system',
    label: 'Use system theme',
    icon: Monitor,
  },
  {
    value: 'light',
    label: 'Use light theme',
    icon: Sun,
  },
  {
    value: 'dark',
    label: 'Use dark theme',
    icon: Moon,
  },
] satisfies Array<{
  value: ThemeMode
  label: string
  icon: LucideIcon
}>

export function ThemeModeToggle({ themeMode, disabled = false, onChange }: ThemeModeToggleProps) {
  return (
    <div className="theme-mode-control" role="group" aria-label="Theme mode">
      {options.map(({ value, label, icon: Icon }) => {
        const selected = themeMode === value
        return (
          <button
            key={value}
            type="button"
            className={selected ? 'theme-mode-option active' : 'theme-mode-option'}
            aria-label={label}
            aria-pressed={selected}
            title={label}
            disabled={disabled}
            onClick={() => onChange(value)}
          >
            <Icon aria-hidden="true" />
          </button>
        )
      })}
    </div>
  )
}
