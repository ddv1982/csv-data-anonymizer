import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { columnMetadataFixture } from '../test-utils/builders'
import { DetectionRunNotice } from './DetectionRunNotice'

describe('DetectionRunNotice', () => {
  it('stays absent for old responses and disabled supplemental detection', () => {
    const { rerender } = render(<DetectionRunNotice columns={[]} />)
    expect(screen.queryByText(/Local detection/)).not.toBeInTheDocument()

    rerender(
      <DetectionRunNotice
        summary={{
          deterministic: 'completed',
          localNer: 'disabled',
          examinedCells: 0,
          totalEligibleCells: 0,
          skippedOversizedCells: 0,
          acceptedCandidates: 0,
          rejectedCandidates: 0,
        }}
        columns={[]}
      />,
    )
    expect(screen.queryByText(/Local detection/)).not.toBeInTheDocument()
  })

  it('summarizes review items after a completed supplemental check', () => {
    render(
      <DetectionRunNotice
        summary={{
          deterministic: 'completed',
          localNer: 'completed',
          examinedCells: 1,
          totalEligibleCells: 1,
          skippedOversizedCells: 0,
          acceptedCandidates: 1,
          rejectedCandidates: 0,
        }}
        columns={[columnMetadataFixture({ reviewReasons: ['detectorsDisagree'] })]}
      />,
    )

    expect(screen.getByText(/marked 1 column for review/i)).toBeInTheDocument()
  })

  it('keeps deterministic results explicit when the supplemental check fails', () => {
    render(
      <DetectionRunNotice
        summary={{
          deterministic: 'completed',
          localNer: 'failed',
          examinedCells: 0,
          totalEligibleCells: 0,
          skippedOversizedCells: 0,
          acceptedCandidates: 0,
          rejectedCandidates: 0,
        }}
        columns={[]}
      />,
    )

    expect(screen.getByText(/Rule-based detection still ran/i)).toBeInTheDocument()
  })
})
