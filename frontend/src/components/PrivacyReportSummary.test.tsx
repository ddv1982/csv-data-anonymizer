import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { privacyReportFixture } from '../test-utils/builders'
import { PrivacyReportSummary } from './PrivacyReportSummary'

describe('PrivacyReportSummary', () => {
  it('shows a compact overview and omits zero-only advanced metrics', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          directIdentifiers: 1,
          pseudonymizedColumns: 1,
          passThroughColumns: 2,
          uniquePseudonymValues: 3,
        })}
      />,
    )

    expect(screen.getByText('Privacy Report')).toBeInTheDocument()
    expect(screen.getByText('Readiness')).toBeInTheDocument()
    expect(screen.getByText('Columns transformed')).toBeInTheDocument()
    expect(screen.getByText('1 pseudonymized')).toBeInTheDocument()
    expect(screen.getByText('Pass-through/no-op')).toBeInTheDocument()
    expect(screen.getByText('Advanced Counts')).toBeInTheDocument()
    expect(screen.queryByText('Pool exhaustions')).not.toBeInTheDocument()
    expect(screen.queryByText('Smart Replacement')).not.toBeInTheDocument()
  })

  it('shows Smart replacement details only when Smart replacement was used', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          smartReplacementColumns: 1,
          smartReplacementValues: 4,
          smartReplacementRejections: 1,
          smartReplacementRejectionReasons: [{ reason: 'containsOriginal', count: 1 }],
        })}
      />,
    )

    expect(screen.getByText('Smart Replacement')).toBeInTheDocument()
    expect(screen.getByText('Smart replacement columns')).toBeInTheDocument()
    expect(screen.getByText('Source text included')).toBeInTheDocument()
  })

  it('keeps column decisions compact in a labelled table', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [
            {
              columnIndex: 2,
              columnName: 'email',
              selected: true,
              detectedType: 'email',
              piiRisk: 'high',
              strategy: 'redact',
              action: 'Redacted values',
              status: 'verified',
              detail: 'All selected email values were redacted.',
            },
          ],
        })}
      />,
    )

    const table = screen.getByRole('table', { name: /privacy report column decisions/i })
    expect(within(table).getByText('email')).toBeInTheDocument()
    expect(within(table).getByText('#2 / Email')).toBeInTheDocument()
    expect(within(table).getByText('Redacted values')).toBeInTheDocument()
    expect(screen.getByText('Showing 1 of 1')).toBeInTheDocument()
  })

  it('renders every column decision instead of hiding later columns', () => {
    const columnReports = Array.from({ length: 13 }, (_, index) => ({
      columnIndex: index,
      columnName: `column-${index}`,
      selected: true,
      detectedType: 'email' as const,
      piiRisk: 'high' as const,
      strategy: 'redact' as const,
      action: 'Redacted values',
      status: 'verified' as const,
      detail: 'Selected values were redacted.',
    }))

    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({ columnReports })}
      />,
    )

    expect(screen.getByText('Showing 13 of 13')).toBeInTheDocument()
    expect(screen.getByText('column-12')).toBeInTheDocument()
  })

  it('surfaces readiness review items above the details sections', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          readiness: {
            status: 'review',
            blockers: [],
            reviewItems: ['Check low confidence columns'],
            verifiedItems: [],
          },
        })}
      />,
    )

    expect(screen.getByText('Needs review')).toBeInTheDocument()
    expect(screen.getByText('Check low confidence columns')).toBeInTheDocument()
  })

  it('renders every readiness item that needs review', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          readiness: {
            status: 'review',
            blockers: [],
            reviewItems: ['Review 1', 'Review 2', 'Review 3', 'Review 4'],
            verifiedItems: [],
          },
        })}
      />,
    )

    expect(screen.getByText('Review 4')).toBeInTheDocument()
  })
})
