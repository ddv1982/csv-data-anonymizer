import type { PrivacyReport } from '../../types'
import { SectionHelp } from '../SectionHelp'
import {
  nonZeroMetrics,
  pluralize,
  readinessSummary,
  sensitiveSummary,
  smartRejectionReasonLabel,
  statusLabel,
  statusPillClass,
  transformationSummary,
} from './helpers'
import { PrivacyMetricGrid } from './PrivacyMetricGrid'
import { PrivacyReportColumnDecisions } from './PrivacyReportColumnDecisions'
import { ReportDisclosure } from './ReportDisclosure'
import type { PrivacyMetric } from './types'

export function PrivacyReportSummary({ privacyReport }: { privacyReport: PrivacyReport }) {
  const transformedColumns =
    privacyReport.pseudonymizedColumns +
    privacyReport.smartReplacementColumns +
    privacyReport.opaqueTokenColumns +
    privacyReport.maskedColumns +
    privacyReport.redactedColumns
  const sensitiveColumnTotal =
    privacyReport.directIdentifiers +
    privacyReport.quasiIdentifiers +
    privacyReport.sensitiveColumns
  const advancedMetrics = nonZeroMetrics([
    { label: 'Direct identifiers', value: privacyReport.directIdentifiers, glossaryTerm: 'directIdentifier' },
    { label: 'Quasi-identifiers', value: privacyReport.quasiIdentifiers, glossaryTerm: 'quasiIdentifier' },
    { label: 'Sensitive columns', value: privacyReport.sensitiveColumns, glossaryTerm: 'sensitive' },
    { label: 'Pseudonymized columns', value: privacyReport.pseudonymizedColumns, glossaryTerm: 'pseudonymizedColumns' },
    { label: 'Opaque token columns', value: privacyReport.opaqueTokenColumns, glossaryTerm: 'opaqueTokenColumns' },
    { label: 'Masked columns', value: privacyReport.maskedColumns, glossaryTerm: 'maskedColumns' },
    { label: 'Redacted columns', value: privacyReport.redactedColumns, glossaryTerm: 'redactedColumns' },
    { label: 'Unique pseudonyms', value: privacyReport.uniquePseudonymValues, glossaryTerm: 'uniquePseudonyms' },
    { label: 'Opaque token values', value: privacyReport.opaqueTokenValues, glossaryTerm: 'opaqueTokenValues' },
    {
      label: 'Repeated source reuses',
      value: privacyReport.reusedPseudonymValues,
      glossaryTerm: 'repeatedSourceReuses',
    },
    { label: 'Collisions avoided', value: privacyReport.collisionsAvoided, glossaryTerm: 'collisionsAvoided' },
    { label: 'Pool exhaustions', value: privacyReport.exhaustedPseudonymPools, glossaryTerm: 'poolExhaustions' },
    { label: 'Format fallbacks', value: privacyReport.shapeFallbackValues, glossaryTerm: 'formatFallbacks' },
  ])
  const smartMetrics = nonZeroMetrics([
    {
      label: 'Smart replacement columns',
      value: privacyReport.smartReplacementColumns,
      glossaryTerm: 'smartReplacementColumns',
    },
    {
      label: 'Smart replacement values',
      value: privacyReport.smartReplacementValues,
      glossaryTerm: 'smartReplacementValues',
    },
    {
      label: 'Smart rejections',
      value: privacyReport.smartReplacementRejections,
      glossaryTerm: 'smartRejections',
    },
    { label: 'Smart fallbacks', value: privacyReport.smartReplacementFallbacks, glossaryTerm: 'smartFallbacks' },
  ])
  const hasSmartReplacementActivity =
    smartMetrics.length > 0 || privacyReport.smartReplacementRejectionReasons.length > 0
  const overviewMetrics: PrivacyMetric[] = [
    {
      label: 'Readiness',
      value: statusLabel(privacyReport.readiness.status),
      detail: readinessSummary(privacyReport),
    },
    {
      label: 'Columns transformed',
      value: transformedColumns,
      detail: transformationSummary(privacyReport),
    },
    {
      label: 'Sensitive columns',
      value: sensitiveColumnTotal,
      detail: sensitiveSummary(privacyReport),
    },
    {
      label: 'Pass-through/no-op',
      value: privacyReport.passThroughColumns,
      glossaryTerm: 'passThroughNoOp',
      detail: 'Left unchanged by the selected strategy.',
    },
  ]

  return (
    <div className="preview-group">
      <div className="section-heading-row">
        <h3>Privacy Report</h3>
        <SectionHelp topic="privacyReport" label="How to read this report" />
      </div>
      <div className="preview-frame privacy-report-frame">
        <PrivacyMetricGrid metrics={overviewMetrics} variant="overview" />
        <ReadinessNotes privacyReport={privacyReport} />

        {hasSmartReplacementActivity ? (
          <ReportDisclosure title="Smart Replacement" countLabel={pluralize(smartMetrics.length, 'metric')}>
            {smartMetrics.length > 0 ? <PrivacyMetricGrid metrics={smartMetrics} /> : null}
            {privacyReport.smartReplacementRejectionReasons.length > 0 ? (
              <div className="privacy-metrics">
                {privacyReport.smartReplacementRejectionReasons.map((item) => (
                  <div className="privacy-metric" key={item.reason}>
                    <span className="privacy-metric-label muted-text text-sm">
                      {smartRejectionReasonLabel(item.reason)}
                    </span>
                    <strong>{item.count.toLocaleString()}</strong>
                  </div>
                ))}
              </div>
            ) : null}
          </ReportDisclosure>
        ) : null}

        {privacyReport.columnReports.length > 0 ? (
          <PrivacyReportColumnDecisions columns={privacyReport.columnReports} />
        ) : null}

        {advancedMetrics.length > 0 ? (
          <ReportDisclosure title="Advanced Counts" countLabel={pluralize(advancedMetrics.length, 'metric')}>
            <PrivacyMetricGrid metrics={advancedMetrics} />
          </ReportDisclosure>
        ) : null}

        {privacyReport.utilityMetrics.length > 0 ? (
          <ReportDisclosure title="Utility" countLabel={pluralize(privacyReport.utilityMetrics.length, 'check')}>
            <PrivacyMetricGrid
              metrics={privacyReport.utilityMetrics.map((metric) => ({
                label: metric.label,
                value: metric.value,
                status: metric.status,
                detail: metric.detail,
              }))}
            />
          </ReportDisclosure>
        ) : null}

        {privacyReport.evidence.length > 0 ? (
          <ReportDisclosure title="Evidence" countLabel={pluralize(privacyReport.evidence.length, 'item')}>
            <div className="privacy-models">
              {privacyReport.evidence.map((item) => (
                <div className="privacy-model-row" key={item.id}>
                  <span>
                    <strong>{item.label}</strong>
                    <span className="muted-text text-sm">{item.detail}</span>
                  </span>
                  <span className={statusPillClass(item.status)}>{statusLabel(item.status)}</span>
                </div>
              ))}
            </div>
          </ReportDisclosure>
        ) : null}

        {privacyReport.notes.length > 0 ? (
          <div className="report-note-list">
            {privacyReport.notes.map((note) => (
              <p className="muted-text text-sm" key={note}>
                {note}
              </p>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  )
}

function ReadinessNotes({ privacyReport }: { privacyReport: PrivacyReport }) {
  const readiness = privacyReport.readiness
  const items = readiness.status === 'blocked'
    ? readiness.blockers
    : readiness.status === 'review'
      ? readiness.reviewItems
      : []

  if (items.length === 0) return null

  return (
    <div className="report-readiness-notes">
      <strong>{readiness.status === 'blocked' ? 'Blocked by' : 'Needs review'}</strong>
      <ul>
        {items.slice(0, 3).map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  )
}
