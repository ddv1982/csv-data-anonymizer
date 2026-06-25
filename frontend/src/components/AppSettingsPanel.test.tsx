import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { defaultSettings } from '../defaults'
import type { AppSettings } from '../types'
import { AppSettingsPanel } from './AppSettingsPanel'

describe('AppSettingsPanel', () => {
  it('allows remember-paths changes before a CSV is loaded', async () => {
    const updates: Array<[keyof AppSettings, AppSettings[keyof AppSettings]]> = []

    render(<SettingsPanelHarness updates={updates} />)

    expect(screen.getByRole('switch', { name: /Remember paths/ })).toBeEnabled()
  })

  it('emits typed setting updates when controls change', async () => {
    const user = userEvent.setup()
    const updates: Array<[keyof AppSettings, AppSettings[keyof AppSettings]]> = []

    render(
      <AppSettingsPanel
        settings={defaultSettings}
        open
        disabled={false}
        onToggleOpen={vi.fn()}
        onUpdateSetting={(key, value) => updates.push([key, value])}
      />,
    )

    await user.click(screen.getByRole('switch', { name: /Remember paths/ }))
    fireEvent.change(screen.getByLabelText(/Output suffix/), { target: { value: '_redacted' } })

    expect(updates).toContainEqual(['rememberLastPaths', false])
    expect(updates.at(-1)).toEqual(['defaultOutputSuffix', '_redacted'])
  })
})

function SettingsPanelHarness({
  updates,
}: {
  updates: Array<[keyof AppSettings, AppSettings[keyof AppSettings]]>
}) {
  const [settings, setSettings] = useState(defaultSettings)

  return (
    <AppSettingsPanel
      settings={settings}
      open
      disabled={false}
      onToggleOpen={vi.fn()}
      onUpdateSetting={(key, value) => {
        updates.push([key, value])
        setSettings((current) => ({ ...current, [key]: value }))
      }}
    />
  )
}
