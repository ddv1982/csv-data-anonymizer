import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { ProcessingStatus } from './ProcessingStatus'
import type { AnonymizeJobStatus } from '../types'

function runningStatus(overrides: Partial<AnonymizeJobStatus> = {}): AnonymizeJobStatus {
  return {
    jobId: 'job-1',
    state: 'running',
    rowsProcessed: 10,
    totalRows: 100,
    cancelRequested: false,
    result: null,
    error: null,
    ...overrides,
  }
}

describe('ProcessingStatus', () => {
  it('keeps cancel actionable while a cancel request is pending', async () => {
    const onCancel = vi.fn()
    const user = userEvent.setup()
    const { rerender } = render(
      <ProcessingStatus status={runningStatus()} fallbackRowCount={0} onCancel={onCancel} />,
    )

    await user.click(screen.getByRole('button', { name: 'Cancel' }))
    rerender(
      <ProcessingStatus
        status={runningStatus({ cancelRequested: true })}
        fallbackRowCount={0}
        onCancel={onCancel}
      />,
    )

    // The label announces the pending cancel, and the button stays pressable so a
    // request the worker never noticed can be retried instead of stranding the user.
    const cancelButton = screen.getByRole('button', { name: 'Canceling…' })
    expect(cancelButton).toBeEnabled()
    await user.click(cancelButton)
    expect(onCancel).toHaveBeenCalledTimes(2)
  })

  it('reports progress and cancel state as live status text', () => {
    render(
      <ProcessingStatus
        status={runningStatus({ rowsProcessed: 1500, totalRows: 3000, cancelRequested: true })}
        fallbackRowCount={0}
        onCancel={vi.fn()}
      />,
    )

    const live = screen.getByRole('status')
    expect(live).toHaveAttribute('aria-live', 'polite')
    // Built through toLocaleString so the assertion does not depend on the test
    // machine's digit grouping.
    expect(live).toHaveTextContent(
      `${(1500).toLocaleString()} of ${(3000).toLocaleString()} rows processed`,
    )
    expect(live).toHaveTextContent('Canceling')
  })

  it('falls back to the analyzed row count before any row is processed', () => {
    render(<ProcessingStatus status={null} fallbackRowCount={2500} onCancel={vi.fn()} />)

    expect(screen.getByRole('status')).toHaveTextContent(
      `Preparing ${(2500).toLocaleString()} rows`,
    )
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeEnabled()
  })
})
