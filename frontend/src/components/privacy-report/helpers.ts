import type { PrivacyReport } from '../../types'
import { formatToken } from '../../utils/format'
import type { PrivacyMetric, ReportStatus } from './types'

export function readinessSummary(privacyReport: PrivacyReport) {
  const readiness = privacyReport.readiness
  if (readiness.status === 'blocked') return pluralize(readiness.blockers.length, 'blocker')
  if (readiness.status === 'review') return pluralize(readiness.reviewItems.length, 'review item')
  if (readiness.verifiedItems.length > 0) return pluralize(readiness.verifiedItems.length, 'verified check')
  return 'No blockers'
}

export function transformationSummary(privacyReport: PrivacyReport) {
  const parts = [
    metricPart(privacyReport.redactedColumns, 'redacted'),
    metricPart(privacyReport.maskedColumns, 'masked'),
    metricPart(privacyReport.pseudonymizedColumns, 'pseudonymized'),
    metricPart(privacyReport.opaqueTokenColumns, 'tokenized'),
    metricPart(privacyReport.smartReplacementColumns, 'smart replacement'),
  ].filter(Boolean)

  return parts.length > 0 ? parts.join(', ') : 'No transformed columns'
}

export function sensitiveSummary(privacyReport: PrivacyReport) {
  const parts = [
    metricPart(privacyReport.directIdentifiers, 'direct'),
    metricPart(privacyReport.quasiIdentifiers, 'quasi'),
    metricPart(privacyReport.sensitiveColumns, 'sensitive'),
  ].filter(Boolean)

  return parts.length > 0 ? parts.join(', ') : 'No sensitive columns detected'
}

export function nonZeroMetrics(metrics: PrivacyMetric[]) {
  return metrics.filter((metric) => {
    if (typeof metric.value === 'number') return metric.value > 0
    return metric.value.trim().length > 0 && metric.value !== '0'
  })
}

export function formatMetricValue(value: string | number) {
  return typeof value === 'number' ? value.toLocaleString() : value
}

export function pluralize(count: number, singular: string, plural = `${singular}s`) {
  return `${count.toLocaleString()} ${count === 1 ? singular : plural}`
}

export function statusPillClass(status: ReportStatus) {
  if (status === 'verified') return 'status-pill success'
  if (status === 'blocked') return 'status-pill blocked'
  if (status === 'review') return 'status-pill warning'
  return 'status-pill'
}

export function statusLabel(status: ReportStatus) {
  if (status === 'verified') return 'Verified'
  if (status === 'blocked') return 'Blocked'
  if (status === 'review') return 'Review'
  return 'Info'
}

export function smartRejectionReasonLabel(reason: PrivacyReport['smartReplacementRejectionReasons'][number]['reason']) {
  if (reason === 'unexpectedOriginal') return 'Unexpected source'
  if (reason === 'missingOutput') return 'Missing output'
  if (reason === 'emptyOutput') return 'Empty output'
  if (reason === 'sameAsOriginal') return 'Copied source'
  if (reason === 'containsOriginal') return 'Source text included'
  if (reason === 'controlCharacter') return 'Control character'
  if (reason === 'duplicateOriginal') return 'Duplicate source'
  if (reason === 'duplicateOutput') return 'Duplicate output'
  return formatToken(reason)
}

function metricPart(count: number, label: string) {
  if (count === 0) return null
  return `${count.toLocaleString()} ${label}`
}
