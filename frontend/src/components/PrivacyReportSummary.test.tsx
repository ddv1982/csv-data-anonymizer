import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import type { PrivacyReport } from '../types'
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
})

function privacyReportFixture(overrides: Partial<PrivacyReport> = {}): PrivacyReport {
  return {
    directIdentifiers: 0,
    quasiIdentifiers: 0,
    sensitiveColumns: 0,
    pseudonymizedColumns: 0,
    smartReplacementColumns: 0,
    opaqueTokenColumns: 0,
    maskedColumns: 0,
    redactedColumns: 0,
    passThroughColumns: 0,
    uniquePseudonymValues: 0,
    reusedPseudonymValues: 0,
    collisionsAvoided: 0,
    exhaustedPseudonymPools: 0,
    opaqueTokenValues: 0,
    smartReplacementValues: 0,
    smartReplacementRejections: 0,
    smartReplacementRejectionReasons: [],
    smartReplacementFallbacks: 0,
    readiness: {
      status: 'verified',
      blockers: [],
      reviewItems: [],
      verifiedItems: [],
    },
    evidence: [],
    columnReports: [],
    utilityMetrics: [],
    notes: [],
    ...overrides,
  }
}
