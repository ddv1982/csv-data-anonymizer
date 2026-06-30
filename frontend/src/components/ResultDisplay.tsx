import { CheckCircle2, FolderOpen, RefreshCcw } from 'lucide-react'
import type { GlossaryKey } from '../glossary'
import { openOutputLocation } from '../tauri'
import type { AnonymizeData, PrivacyReport, ReleaseEvidenceStatus } from '../types'
import { messageFrom } from '../utils/errors'
import { formatResultStats, formatToken } from '../utils/format'
import { Alert } from './Alert'
import { GlossaryPopover } from './GlossaryPopover'
import { RiskBadge } from './RiskBadge'
import { SectionHelp } from './SectionHelp'

export function ResultDisplay({
  result,
  onReset,
  onError,
}: {
  result: AnonymizeData
  onReset: () => void
  onError: (message: string) => void
}) {
  async function handleOpenFolder() {
    try {
      await openOutputLocation(result.outputPath)
    } catch (caught) {
      onError(messageFrom(caught))
    }
  }

  return (
    <div className="result-stack">
      <Alert variant="success" icon={<CheckCircle2 aria-hidden="true" />}>
        <h2>Output created</h2>
        <div className="result-description">
          <p>Selected data was transformed in the protected CSV.</p>
          <p className="mono muted-text result-path">{result.outputPath}</p>
          <p className="muted-text text-sm">{formatResultStats(result)}</p>
        </div>
      </Alert>

      <PrivacyReportSummary privacyReport={result.privacyReport} />

      <div className="result-actions">
        <button type="button" className="button button-outline" onClick={() => void handleOpenFolder()}>
          <FolderOpen aria-hidden="true" />
          Open Folder
        </button>
        <button type="button" className="button button-primary" onClick={onReset}>
          <RefreshCcw aria-hidden="true" />
          Transform Another File
        </button>
      </div>
    </div>
  )
}

export function PrivacyReportSummary({ privacyReport }: { privacyReport: PrivacyReport }) {
  const privacyMetrics: Array<{ label: string; value: string | number; glossaryTerm: GlossaryKey }> = [
    { label: 'Direct identifiers', value: privacyReport.directIdentifiers, glossaryTerm: 'directIdentifier' },
    { label: 'Quasi-identifiers', value: privacyReport.quasiIdentifiers, glossaryTerm: 'quasiIdentifier' },
    { label: 'Sensitive columns', value: privacyReport.sensitiveColumns, glossaryTerm: 'sensitive' },
    { label: 'Pseudonymized columns', value: privacyReport.pseudonymizedColumns, glossaryTerm: 'pseudonymizedColumns' },
    {
      label: 'Smart replacement columns',
      value: privacyReport.smartReplacementColumns,
      glossaryTerm: 'smartReplacementColumns',
    },
    { label: 'Opaque token columns', value: privacyReport.opaqueTokenColumns, glossaryTerm: 'opaqueTokenColumns' },
    { label: 'Masked columns', value: privacyReport.maskedColumns, glossaryTerm: 'maskedColumns' },
    { label: 'Redacted columns', value: privacyReport.redactedColumns, glossaryTerm: 'redactedColumns' },
    { label: 'Pass-through/no-op', value: privacyReport.passThroughColumns, glossaryTerm: 'passThroughNoOp' },
    { label: 'Unique pseudonyms', value: privacyReport.uniquePseudonymValues, glossaryTerm: 'uniquePseudonyms' },
    { label: 'Opaque token values', value: privacyReport.opaqueTokenValues, glossaryTerm: 'opaqueTokenValues' },
    {
      label: 'Repeated source reuses',
      value: privacyReport.reusedPseudonymValues,
      glossaryTerm: 'repeatedSourceReuses',
    },
    { label: 'Collisions avoided', value: privacyReport.collisionsAvoided, glossaryTerm: 'collisionsAvoided' },
    { label: 'Pool exhaustions', value: privacyReport.exhaustedPseudonymPools, glossaryTerm: 'poolExhaustions' },
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
  ]

  return (
    <div className="preview-group">
      <div className="section-heading-row">
        <h3>Privacy Report</h3>
        <SectionHelp topic="privacyReport" label="How to read this report" />
      </div>
      <div className="preview-frame">
        <div className="privacy-metrics">
          {privacyMetrics.map(({ label, value, glossaryTerm }) => (
            <div className="privacy-metric" key={label}>
              <span className="privacy-metric-label muted-text text-sm">
                <span>{label}</span>
                <GlossaryPopover term={glossaryTerm} />
              </span>
              <strong>{typeof value === 'number' ? value.toLocaleString() : value}</strong>
            </div>
          ))}
        </div>
        {privacyReport.utilityMetrics.length > 0 ? (
          <div className="report-subsection">
            <h4>Utility</h4>
            <div className="privacy-metrics">
              {privacyReport.utilityMetrics.map((metric) => (
                <div className="privacy-metric" key={`${metric.label}-${metric.value}`}>
                  <span className="privacy-metric-label muted-text text-sm">{metric.label}</span>
                  <strong>{metric.value}</strong>
                  <span className={statusPillClass(metric.status)}>{statusLabel(metric.status)}</span>
                  {metric.detail ? <p className="muted-text text-sm">{metric.detail}</p> : null}
                </div>
              ))}
            </div>
          </div>
        ) : null}
        {privacyReport.evidence.length > 0 ? (
          <div className="report-subsection">
            <h4>Evidence</h4>
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
          </div>
        ) : null}
        {privacyReport.smartReplacementRejectionReasons.length > 0 ? (
          <div className="report-subsection">
            <h4>Local AI Rejections</h4>
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
          </div>
        ) : null}
        {privacyReport.columnReports.length > 0 ? (
          <div className="report-subsection">
            <h4>Column Decisions</h4>
            <div className="table-frame release-column-frame">
              <table className="release-column-table">
                <thead>
                  <tr>
                    <th>Column</th>
                    <th>Risk</th>
                    <th>Strategy</th>
                    <th>Status</th>
                    <th>Action</th>
                  </tr>
                </thead>
                <tbody>
                  {privacyReport.columnReports.slice(0, 12).map((column) => (
                    <tr key={`${column.columnIndex}-${column.columnName}`}>
                      <td>
                        <strong>{column.columnName}</strong>
                        <span className="muted-text text-sm">
                          #{column.columnIndex} · {formatToken(column.detectedType)}
                        </span>
                      </td>
                      <td>
                        <RiskBadge risk={column.piiRisk} />
                      </td>
                      <td>{formatToken(column.strategy)}</td>
                      <td>
                        <span className={statusPillClass(column.status)}>{statusLabel(column.status)}</span>
                      </td>
                      <td>
                        <strong>{column.action}</strong>
                        <p className="muted-text text-sm">{column.detail}</p>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {privacyReport.columnReports.length > 12 ? (
              <p className="muted-text text-sm">
                Showing 12 of {privacyReport.columnReports.length.toLocaleString()} column decisions.
              </p>
            ) : null}
          </div>
        ) : null}
        {privacyReport.notes.map((note) => (
          <p className="muted-text text-sm" key={note}>
            {note}
          </p>
        ))}
      </div>
    </div>
  )
}

function statusPillClass(status: ReleaseEvidenceStatus) {
  if (status === 'verified') return 'status-pill success'
  if (status === 'blocked') return 'status-pill blocked'
  if (status === 'review') return 'status-pill warning'
  return 'status-pill'
}

function statusLabel(status: ReleaseEvidenceStatus) {
  if (status === 'verified') return 'Verified'
  if (status === 'blocked') return 'Blocked'
  if (status === 'review') return 'Review'
  return 'Info'
}

function smartRejectionReasonLabel(reason: PrivacyReport['smartReplacementRejectionReasons'][number]['reason']) {
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
