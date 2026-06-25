import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it } from 'vitest'
import { HelpPopover } from './GlossaryPopover'

describe('HelpPopover', () => {
  it('opens, closes on Escape, and restores focus', async () => {
    const user = userEvent.setup()
    render(
      <HelpPopover title="Privacy" triggerLabel="Explain privacy">
        <p>Privacy detail</p>
      </HelpPopover>,
    )

    const trigger = screen.getByRole('button', { name: 'Explain privacy' })
    await user.click(trigger)

    expect(await screen.findByRole('tooltip')).toHaveTextContent('Privacy detail')

    await user.keyboard('{Escape}')
    await waitFor(() => expect(screen.queryByRole('tooltip')).not.toBeInTheDocument())
    expect(trigger).toHaveFocus()
  })

  it('closes when focus moves outside the popover', async () => {
    const user = userEvent.setup()
    render(
      <>
        <HelpPopover title="Privacy" triggerLabel="Explain privacy">
          <p>Privacy detail</p>
        </HelpPopover>
        <button type="button">Outside</button>
      </>,
    )

    await user.click(screen.getByRole('button', { name: 'Explain privacy' }))
    expect(await screen.findByRole('tooltip')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Outside' }))
    await waitFor(() => expect(screen.queryByRole('tooltip')).not.toBeInTheDocument())
  })
})
