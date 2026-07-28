import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { columnMetadataFixture } from '../test-utils/builders'
import { ColumnTable } from './ColumnTable'

describe('ColumnTable', () => {
  it('allows every rendered column to be selected', async () => {
    const user = userEvent.setup()
    const onToggleColumn = vi.fn()

    render(
      <ColumnTable
        columns={[columnMetadataFixture({ name: 'email' })]}
        allColumnCount={1}
        selectedSet={new Set()}
        loading={false}
        showAllColumns={false}
        hiddenColumnCount={0}
        onToggleColumn={onToggleColumn}
        controls={{}}
        onStrategyChange={vi.fn()}
        onToggleShowAll={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('checkbox', { name: /select column email/i }))

    expect(onToggleColumn).toHaveBeenCalledWith(expect.objectContaining({ name: 'email' }))
  })

  it('blocks selection and strategy controls when disabled', async () => {
    const user = userEvent.setup()
    const onToggleColumn = vi.fn()
    const onStrategyChange = vi.fn()

    render(
      <ColumnTable
        columns={[columnMetadataFixture({ name: 'email' })]}
        allColumnCount={1}
        selectedSet={new Set()}
        loading={false}
        disabled
        showAllColumns={false}
        hiddenColumnCount={0}
        onToggleColumn={onToggleColumn}
        controls={{}}
        onStrategyChange={onStrategyChange}
        onToggleShowAll={vi.fn()}
      />,
    )

    const checkbox = screen.getByRole('checkbox', { name: /select column email/i })
    const strategy = screen.getByRole('combobox', { name: /strategy for email/i })
    expect(checkbox).toBeDisabled()
    expect(strategy).toBeDisabled()

    await user.click(checkbox)
    await user.click(strategy)

    expect(onToggleColumn).not.toHaveBeenCalled()
    expect(onStrategyChange).not.toHaveBeenCalled()
  })
})
