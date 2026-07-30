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
    metricPart(privacyReport.labelledColumns, 'labelled'),
    metricPart(privacyReport.smartReplacementColumns, 'smart replacement'),
  ].filter(Boolean)

  return parts.length > 0 ? parts.join(', ') : 'No transformed columns'
}

export function sensitiveSummary(privacyReport: PrivacyReport) {
  const parts = [
    metricPart(privacyReport.directIdentifiers, 'direct'),
    metricPart(privacyReport.quasiIdentifiers, 'quasi'),
  ].filter(Boolean)

  return parts.length > 0 ? parts.join(', ') : 'No sensitive columns detected'
}

export function nonZeroMetrics(metrics: PrivacyMetric[]) {
  return metrics.filter((metric) => {
    if (typeof metric.value === 'number') return metric.value > 0
    return metric.value.trim().length > 0 && metric.value !== '0'
  })
}

// This panel prints numbers exactly as Rust produced them, and that is a rule about this
// surface rather than a style preference. Rust builds whole sentences that are rendered here
// verbatim — evidence details, readiness review items, report notes — and some of them state
// the same figure a React component states a few lines away. `drop_column_advice` is the
// clearest case: Rust's "…would leave 3 of them unique instead of 4123" lands in the readiness
// list while `DropColumnAdvice` renders the same sentence in the disclosure above it. With
// grouping on the React side a reader saw "4,123" and "4123" in one panel, and had to work out
// whether those were two numbers.
//
// So the digits are left alone. It also makes the panel locale-invariant: `toLocaleString()`
// with no argument follows the host, which rendered this same figure "4,123" in English and
// "4.123" in Dutch, and forced the tests to compute their expectations with `toLocaleString`
// at assertion time just to survive being run on a different machine. Rust has no locale here
// and cannot be taught one, so the only rendering both sides can agree on is the raw integer.
export function pluralize(count: number, singular: string, plural = `${singular}s`) {
  return `${count} ${count === 1 ? singular : plural}`
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
  // Same wording as the Rust release report ("another row's source value"), so the
  // exported report and the on-screen report name the same event the same way.
  if (reason === 'matchesOtherOriginal') return "Another row's source value"
  if (reason === 'controlCharacter') return 'Control character'
  if (reason === 'duplicateOriginal') return 'Duplicate source'
  if (reason === 'duplicateOutput') return 'Duplicate output'
  return formatToken(reason)
}

function metricPart(count: number, label: string) {
  if (count === 0) return null
  return `${count} ${label}`
}
