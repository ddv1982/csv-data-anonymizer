import { ChevronDown, ChevronUp } from 'lucide-react'
import type {
  AnonymizationStrategy,
  ColumnControl,
  ColumnMetadata,
} from '../types'
import { columnStrategies, strategyLabel } from '../dataOptions'
import {
  detectorConfidenceLabel,
  detectorSourceSummary,
  privacyFindingKindLabel,
} from '../utils/detectorEvidence'
import { formatToken } from '../utils/format'
import { hasSampleData, maxVisibleColumns } from '../utils/columns'
import { columnRedactionPlaceholder } from '../utils/redactionPlaceholder'
import { completeEvidenceProfile } from '../utils/analysisContract'
import { GlossaryLabel, HelpPopover } from './GlossaryPopover'
import { RiskBadge } from './RiskBadge'

export function ColumnTable({
  columns,
  allColumnCount,
  selectedSet,
  loading,
  disabled = false,
  showAllColumns,
  hiddenColumnCount,
  onToggleColumn,
  controls,
  onStrategyChange,
  onToggleShowAll,
}: {
  columns: ColumnMetadata[]
  allColumnCount: number
  selectedSet: Set<number>
  loading: boolean
  disabled?: boolean
  showAllColumns: boolean
  hiddenColumnCount: number
  onToggleColumn: (column: ColumnMetadata) => void
  controls: Record<number, ColumnControl>
  onStrategyChange: (column: ColumnMetadata, value: AnonymizationStrategy) => void
  onToggleShowAll: () => void
}) {
  const columnSpan = 7

  return (
    <div className="table-frame">
      <table className="column-table">
        <colgroup>
          <col className="column-select-col" />
          <col className="column-index-col" />
          <col className="column-name-col" />
          <col className="column-detected-type-col" />
          <col className="column-strategy-col" />
          <col className="column-evidence-col" />
          <col className="column-risk-col" />
        </colgroup>
        <thead>
          <tr>
            <th className="checkbox-column" aria-label="Selected"></th>
            <th className="index-column">#</th>
            <th className="column-title-column">Column Name</th>
            <th className="detected-type-heading">Detected Format</th>
            <th className="strategy-heading">
              <GlossaryLabel term="strategy">Strategy</GlossaryLabel>
            </th>
            <th className="privacy-evidence-heading">Evidence</th>
            <th className="risk-heading">Risk</th>
          </tr>
        </thead>
        <tbody>
          {loading ? <ColumnSkeletonRows /> : null}
          {!loading && allColumnCount === 0 ? (
            <tr>
              <td colSpan={columnSpan} className="empty-table-cell">
                No columns to display
              </td>
            </tr>
          ) : null}
          {!loading
            ? columns.map((column) => {
                const sampleDataAvailable = hasSampleData(column)
                const control = controls[column.index]
                const selected = selectedSet.has(column.index)
                const redactionPlaceholder = columnRedactionPlaceholder(column)
                const rowClassName = ['clickable-row', selected ? 'selected-row' : '']
                  .filter(Boolean)
                  .join(' ')
                return (
                  <tr
                    key={`${column.index}-${column.name}`}
                    className={rowClassName}
                    onClick={disabled ? undefined : () => onToggleColumn(column)}
                  >
                    <td className="checkbox-column">
                      <input
                        type="checkbox"
                        className="table-checkbox"
                        checked={selected}
                        disabled={disabled}
                        onChange={() => {
                          onToggleColumn(column)
                        }}
                        onClick={(event) => event.stopPropagation()}
                        aria-label={`Select column ${column.name}`}
                      />
                    </td>
                    <td className="index-column mono muted-text">{column.index}</td>
                    <td className="column-title-cell">
                      <span className="column-title-content">
                        <span className={sampleDataAvailable ? 'column-name' : 'column-name no-data'}>
                          {column.name}
                        </span>
                        {column.reviewReasons?.length ? (
                          <span className="status-pill warning">Review</span>
                        ) : null}
                        {!sampleDataAvailable ? (
                          <span className="column-note">No sample data</span>
                        ) : column.piiRisk === 'low' ? (
                          <span className="column-note">
                            No obvious sensitive fields detected
                          </span>
                        ) : null}
                      </span>
                    </td>
                    <td className="detected-type-cell">
                      <span className="mobile-cell-label">Detected format</span>
                      <span className="detected-type-value">
                        <span className="muted-text">{formatToken(column.detectedType)}</span>
                        <DetectionTracePopover column={column} />
                      </span>
                    </td>
                    <td className="control-cell">
                      <span className="mobile-cell-label">Strategy</span>
                      <select
                        value={control?.strategy ?? column.strategy ?? 'auto'}
                        disabled={loading || disabled}
                        aria-label={`Strategy for ${column.name}`}
                        onClick={(event) => event.stopPropagation()}
                        onChange={(event) => onStrategyChange(column, event.target.value as AnonymizationStrategy)}
                      >
                        {columnStrategies.map((strategy) => (
                          <option key={strategy} value={strategy}>
                            {strategyLabel(strategy)}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td className="privacy-evidence-column">
                      <span className="mobile-cell-label">Evidence</span>
                      <DecisionEvidenceCell column={column} />
                      {(control?.strategy ?? column.strategy) === 'redact' && sampleDataAvailable && redactionPlaceholder ? (
                        <span className="column-note redaction-output">
                          Output: <span className="mono">{redactionPlaceholder}</span>
                        </span>
                      ) : null}
                    </td>
                    <td className="risk-cell">
                      <span className="mobile-cell-label">Risk</span>
                      <RiskBadge risk={column.piiRisk} />
                    </td>
                  </tr>
                )
              })
            : null}
          {!loading && allColumnCount > maxVisibleColumns ? (
            <tr className="show-more-row">
              <td colSpan={columnSpan} className="show-more-cell">
                <button type="button" className="button button-ghost button-sm" disabled={disabled} onClick={onToggleShowAll}>
                  {showAllColumns ? <ChevronUp aria-hidden="true" /> : <ChevronDown aria-hidden="true" />}
                  {showAllColumns ? 'Show Less' : `Show ${hiddenColumnCount} More Columns`}
                </button>
              </td>
            </tr>
          ) : null}
        </tbody>
      </table>
    </div>
  )
}

function DecisionEvidenceCell({ column }: { column: ColumnMetadata }) {
  const profile = completeEvidenceProfile(column.evidenceProfile)
  if (!profile) {
    return (
      <span className="decision-evidence-cell">
        <span className="decision-status status-uncertain">Unavailable</span>
        <span className="decision-meaning">Decision evidence could not be displayed</span>
        <span className="column-note">Re-select the file or update the application.</span>
        <HelpPopover title="Column decision unavailable" triggerLabel={`Explain unavailable column decision for ${column.name}`}>
          <div className="detector-popover-content">
            <p>The analysis returned an incomplete decision profile for this column.</p>
            <RawEvidenceDetails column={column} />
          </div>
        </HelpPopover>
      </span>
    )
  }
  const semantic = profile.semanticDecision
  const format = profile.formatEvidence
  const statusLabel = semantic.status[0].toLocaleUpperCase() + semantic.status.slice(1)
  const meaning = semantic.kind === 'unknown'
    ? 'Meaning unknown'
    : privacyFindingKindLabel(semantic.kind)
  const coverage = format.basis === 'userOverride'
    ? `User override; detector examined ${format.sampleCount.toLocaleString()} samples`
    : `${format.matchCount.toLocaleString()} of ${format.sampleCount.toLocaleString()} ${
        format.basis === 'retainedPreviewValues' ? 'retained values' : 'samples'
      }`

  return (
    <span className="decision-evidence-cell">
      <span
        className={`decision-status status-${semantic.status}`}
        aria-label={`Semantic decision: ${statusLabel}`}
      >
        {statusLabel}
      </span>
      <span className="decision-meaning">
        {meaning}
        <span className="muted-text">{detectorConfidenceLabel(semantic.confidence)}</span>
      </span>
      <span className="column-note">Coverage: {coverage}</span>
      <HelpPopover title="Column decision" triggerLabel={`Explain column decision for ${column.name}`}>
        <div className="detector-popover-content">
          <p>
            <strong>Meaning:</strong> {meaning} ({detectorConfidenceLabel(semantic.confidence)})
          </p>
          <p>
            <strong>Format coverage:</strong> {coverage}
          </p>
          <p>{semantic.reason}</p>
          <p>
            <strong>Privacy action:</strong>{' '}
            {profile.privacyDecision.recommendedStrategy} — {profile.privacyDecision.reason}
          </p>
          <p>
            <strong>Redaction output:</strong>{' '}
            <span className="mono">{profile.redactionDecision.placeholder}</span>
          </p>
          {semantic.supportingEvidence.length > 0 ? (
            <EvidenceSources title="Supporting sources" sources={semantic.supportingEvidence} />
          ) : null}
          {semantic.conflictingEvidence.length > 0 ? (
            <EvidenceSources title="Conflicting sources" sources={semantic.conflictingEvidence} />
          ) : null}
          <RawEvidenceDetails column={column} />
        </div>
      </HelpPopover>
    </span>
  )
}

function EvidenceSources({ title, sources }: { title: string; sources: string[] }) {
  return (
    <div>
      <strong>{title}:</strong>
      <ul className="decision-reason-list">
        {sources.map((source) => <li key={source} className="mono">{source}</li>)}
      </ul>
    </div>
  )
}

function RawEvidenceDetails({ column }: { column: ColumnMetadata }) {
  const evidence = column.privacyEvidence ?? []
  if (evidence.length === 0) {
    return (
      <div>
        <strong>Evidence details:</strong>
        <p className="muted-text text-sm">No privacy evidence was recorded.</p>
      </div>
    )
  }

  return (
    <div>
      <strong>Evidence details:</strong>
      {evidence.map((item, index) => (
        <div
          className="detector-candidate"
          key={`${item.kind}-${item.dataType}-${item.detector}-${index}`}
        >
          <span className={`status-pill ${item.confidence === 'high' ? 'success' : ''}`}>
            {detectorConfidenceLabel(item.confidence)}
          </span>
          <span>
            <strong>{privacyFindingKindLabel(item.kind)}</strong>
            <span className="muted-text text-sm">
              {detectorSourceSummary(item)} ·{' '}
              {item.matchCount.toLocaleString()} of {item.sampleCount.toLocaleString()} samples ·{' '}
              {formatToken(item.dataType)}
            </span>
          </span>
          <p className="muted-text text-sm">{item.reason}</p>
        </div>
      ))}
    </div>
  )
}

function DetectionTracePopover({ column }: { column: ColumnMetadata }) {
  const trace = column.detectionTrace
  if (!trace) return null

  const candidates = trace.candidates.slice(0, 5)

  return (
    <HelpPopover title="Detector evidence" triggerLabel={`Explain detector evidence for ${column.name}`}>
      <div className="detector-popover-content">
        <p>{trace.summary}</p>
        <p>
          <strong>Selected:</strong> {trace.selectedReason}
        </p>
        <div className="detector-candidates" aria-label="Detector candidates">
          {candidates.map((candidate) => (
            <div className="detector-candidate" key={`${candidate.dataType}-${candidate.reason}`}>
              <span className={candidate.accepted ? 'status-pill success' : 'status-pill'}>
                {candidate.accepted ? 'Selected' : 'Checked'}
              </span>
              <span>
                <strong>{formatToken(candidate.dataType)}</strong>
                <span className="muted-text text-sm">
                  {candidate.matchCount.toLocaleString()} of {candidate.totalConsidered.toLocaleString()} values,
                  {` ${formatToken(candidate.confidence)} confidence`}
                </span>
              </span>
              <p className="muted-text text-sm">{candidate.reason}</p>
            </div>
          ))}
        </div>
      </div>
    </HelpPopover>
  )
}

function ColumnSkeletonRows() {
  return (
    <>
      {Array.from({ length: 5 }, (_, index) => (
        <tr key={index}>
          <td>
            <span className="skeleton skeleton-checkbox" />
          </td>
          <td>
            <span className="skeleton skeleton-index" />
          </td>
          <td>
            <span className="skeleton skeleton-wide" />
          </td>
          <td>
            <span className="skeleton skeleton-medium" />
          </td>
          <td>
            <span className="skeleton skeleton-badge" />
          </td>
          <td>
            <span className="skeleton skeleton-badge" />
          </td>
          <td>
            <span className="skeleton skeleton-badge" />
          </td>
        </tr>
      ))}
    </>
  )
}
