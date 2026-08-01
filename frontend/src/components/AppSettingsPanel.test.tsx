import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { defaultSettings } from '../defaults'
import type { AppSettings } from '../types'
import { AppSettingsPanel } from './AppSettingsPanel'

describe('AppSettingsPanel', () => {
  it('generates an ephemeral 256-bit tokenization key outside persisted settings', async () => {
    const user = userEvent.setup()
    const onTokenizationKeyChange = vi.fn()
    const onUpdateSetting = vi.fn()
    render(
      <AppSettingsPanel
        settings={defaultSettings}
        open
        onToggleOpen={vi.fn()}
        onUpdateSetting={onUpdateSetting}
        tokenizationKey={null}
        onTokenizationKeyChange={onTokenizationKeyChange}
      />,
    )

    await user.click(screen.getByRole('button', { name: /generate key/i }))
    expect(onTokenizationKeyChange).toHaveBeenCalledWith(expect.stringMatching(/^[0-9a-f]{64}$/))
    expect(onUpdateSetting).not.toHaveBeenCalled()
  })

  it('validates a manually entered tokenization key inline', async () => {
    const user = userEvent.setup()
    const onTokenizationKeyChange = vi.fn()
    const { rerender } = render(
      <AppSettingsPanel
        settings={defaultSettings}
        open
        onToggleOpen={vi.fn()}
        onUpdateSetting={vi.fn()}
        tokenizationKey={null}
        onTokenizationKeyChange={onTokenizationKeyChange}
      />,
    )

    await user.type(screen.getByLabelText(/repeatable token key/i), 'abc')
    expect(onTokenizationKeyChange).toHaveBeenCalledWith('a')

    rerender(
      <AppSettingsPanel
        settings={defaultSettings}
        open
        onToggleOpen={vi.fn()}
        onUpdateSetting={vi.fn()}
        tokenizationKey="abc"
        onTokenizationKeyChange={onTokenizationKeyChange}
      />,
    )
    expect(screen.getByRole('alert')).toHaveTextContent('Enter exactly 64 hexadecimal characters.')
    expect(screen.getByLabelText(/repeatable token key/i)).toHaveAttribute('aria-invalid', 'true')
    expect(screen.getByRole('button', { name: /copy key/i })).toBeDisabled()
  })

  it('confirms when a valid key is copied', async () => {
    const user = userEvent.setup()
    const writeText = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } })
    const key = 'a'.repeat(64)

    render(
      <AppSettingsPanel
        settings={defaultSettings}
        open
        onToggleOpen={vi.fn()}
        onUpdateSetting={vi.fn()}
        tokenizationKey={key}
        onTokenizationKeyChange={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: /copy key/i }))
    expect(writeText).toHaveBeenCalledWith(key)
    expect(screen.getByRole('status')).toHaveTextContent('Key copied to clipboard.')
  })

  it('allows remember-paths changes before a CSV is loaded', async () => {
    const user = userEvent.setup()
    const updates: Array<[keyof AppSettings, AppSettings[keyof AppSettings]]> = []

    render(<SettingsPanelHarness updates={updates} />)

    expect(screen.getByRole('switch', { name: /Remember paths/ })).toBeEnabled()
    await user.click(screen.getByRole('switch', { name: /Remember paths/ }))
    expect(updates).toContainEqual(['rememberLastPaths', false])
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
    await user.click(screen.getByRole('switch', { name: /Local AI detection/ }))
    fireEvent.change(screen.getByLabelText(/Output suffix/), { target: { value: '_redacted' } })

    expect(updates).toContainEqual(['rememberLastPaths', false])
    expect(updates).toContainEqual(['localNerEnabled', true])
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
