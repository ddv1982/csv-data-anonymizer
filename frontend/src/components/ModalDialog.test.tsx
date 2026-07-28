import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { ModalDialog } from './ModalDialog'

describe('ModalDialog', () => {
  it('makes background content inert only while open', () => {
    const background = document.createElement('main')
    document.body.append(background)
    const { rerender, unmount } = render(
      <ModalDialog open title="Settings" onClose={vi.fn()}>
        Dialog content
      </ModalDialog>,
    )

    expect(screen.getByRole('dialog', { name: 'Settings' })).toBeInTheDocument()
    expect(background).toHaveAttribute('inert')

    rerender(
      <ModalDialog open={false} title="Settings" onClose={vi.fn()}>
        Dialog content
      </ModalDialog>,
    )
    expect(background).not.toHaveAttribute('inert')

    unmount()
    background.remove()
  })
})
