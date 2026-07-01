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
})
