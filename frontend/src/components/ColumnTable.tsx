import { ChevronDown, ChevronUp } from 'lucide-react'
import type {
  AnonymizationStrategy,
  ColumnControl,
  ColumnMetadata,
} from '../types'
import { csvStrategies, strategyLabel } from '../dataOptions'
import {
  detectorConfidenceLabel,
  detectorSourceSummary,
  privacyFindingKindLabel,
  visibleEvidence,
} from '../utils/detectorEvidence'
import { formatToken } from '../utils/format'
import { hasSampleData, isSelectableColumn, maxVisibleColumns } from '../utils/columns'
import { GlossaryLabel, HelpPopover } from './GlossaryPopover'
import { RiskBadge } from './RiskBadge'

export function ColumnTable({
  columns,
  allColumnCount,
  selectedSet,
  loading,
  showAllColumns,
  hiddenColumnCount,
  onToggleColumn,
  controls,
  onStrategyChange,
  onToggleShowAll,
  availableStrategies = csvStrategies,
  strategyControlsDisabled = false,
  strategyControlsDisabledReason,
}: {
  columns: ColumnMetadata[]
  allColumnCount: number
  selectedSet: Set<number>
  loading: boolean
  showAllColumns: boolean
  hiddenColumnCount: number
  onToggleColumn: (column: ColumnMetadata) => void
  controls: Record<number, ColumnControl>
  onStrategyChange: (column: ColumnMetadata, value: AnonymizationStrategy) => void
  onToggleShowAll: () => void
  availableStrategies?: AnonymizationStrategy[]
  strategyControlsDisabled?: boolean
  strategyControlsDisabledReason?: string
}) {
  const columnSpan = 7

  return (
    <div className="table-frame">
      <table className="column-table">
        <thead>
          <tr>
            <th className="checkbox-column" aria-label="Selected"></th>
            <th className="index-column">#</th>
            <th className="column-title-column">Column Name</th>
            <th>Detected Type</th>
            <th>
              <GlossaryLabel term="strategy">Strategy</GlossaryLabel>
            </th>
            <th>Evidence</th>
            <th>Risk</th>
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
                const selectable = isSelectableColumn(column)
                const sampleDataAvailable = hasSampleData(column)
                const control = controls[column.index]
                const selected = selectedSet.has(column.index)
                const canToggleSelection = selectable
                const rowClassName = [canToggleSelection ? 'clickable-row' : '', !selectable ? 'muted-row' : '', selected ? 'selected-row' : '']
                  .filter(Boolean)
                  .join(' ')
                return (
                  <tr
                    key={`${column.index}-${column.name}`}
                    className={rowClassName}
                    onClick={() => {
                      if (canToggleSelection) onToggleColumn(column)
                    }}
                  >
                    <td className="checkbox-column">
                      {selectable ? (
                        <input
                          type="checkbox"
                          className="table-checkbox"
                          checked={selected}
                          onChange={() => {
                            onToggleColumn(column)
                          }}
                          onClick={(event) => event.stopPropagation()}
                          aria-label={`Select column ${column.name}`}
                        />
                      ) : (
                        <span className="checkbox-placeholder" aria-hidden="true" />
                      )}
                    </td>
                    <td className="index-column mono muted-text">{column.index}</td>
                    <td className="column-title-cell">
                      <span className={sampleDataAvailable ? 'column-name' : 'column-name no-data'}>
                        {column.name}
                      </span>
                      {!sampleDataAvailable ? (
                        <span className="column-note">(no sample data)</span>
                      ) : column.piiRisk === 'low' ? (
                        <span className="column-note">
                          (no obvious sensitive fields detected)
                        </span>
                      ) : null}
                    </td>
                    <td className="detected-type-cell">
                      <span className="mobile-cell-label">Detected type</span>
                      <span className="detected-type-value">
                        <span className="muted-text">{formatToken(column.detectedType)}</span>
                        <DetectionTracePopover column={column} />
                      </span>
                    </td>
                    <td className="control-cell">
                      <span className="mobile-cell-label">Strategy</span>
                      <select
                        value={strategyControlsDisabled ? 'auto' : (control?.strategy ?? column.strategy ?? 'auto')}
                        disabled={!selectable || loading || strategyControlsDisabled}
                        title={strategyControlsDisabled ? strategyControlsDisabledReason : undefined}
                        aria-label={`Strategy for ${column.name}`}
                        onClick={(event) => event.stopPropagation()}
                        onChange={(event) => onStrategyChange(column, event.target.value as AnonymizationStrategy)}
                      >
                        {availableStrategies.map((strategy) => (
                          <option key={strategy} value={strategy}>
                            {strategyLabel(strategy)}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td className="privacy-evidence-column">
                      <span className="mobile-cell-label">Evidence</span>
                      <PrivacyEvidenceCell column={column} />
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
                <button type="button" className="button button-ghost button-sm" onClick={onToggleShowAll}>
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

function PrivacyEvidenceCell({
  column,
}: {
  column: ColumnMetadata
}) {
  const evidence = visibleEvidence(column.privacyEvidence, 'balanced')
  if (evidence.length === 0) {
    return <span className="muted-text text-sm">None</span>
  }

  const visible = evidence.slice(0, 2)
  const hiddenCount = Math.max(evidence.length - visible.length, 0)

  return (
    <span className="privacy-evidence-cell">
      {visible.map((item) => (
        <span
          key={`${item.kind}-${item.dataType}`}
          className={`privacy-evidence-chip confidence-${item.confidence}`}
          title={`${privacyFindingKindLabel(item.kind)}: ${item.matchCount} of ${item.sampleCount} sampled values`}
        >
          {privacyFindingKindLabel(item.kind)}
          <span>{item.matchCount}</span>
        </span>
      ))}
      {hiddenCount > 0 ? <span className="privacy-evidence-more">+{hiddenCount}</span> : null}
      <HelpPopover title="Privacy evidence" triggerLabel={`Explain privacy evidence for ${column.name}`}>
        <div className="detector-popover-content">
          {evidence.map((item) => (
            <div className="detector-candidate" key={`${item.kind}-${item.dataType}`}>
              <span className={`status-pill ${item.confidence === 'high' ? 'success' : ''}`}>
                {detectorConfidenceLabel(item.confidence)}
              </span>
              <span>
                <strong>{privacyFindingKindLabel(item.kind)}</strong>
                <span className="muted-text text-sm">
                  {detectorSourceSummary(item)} ·{' '}
                  {item.matchCount.toLocaleString()} of {item.sampleCount.toLocaleString()} samples,
                  {` ${formatToken(item.dataType)}`}
                </span>
              </span>
              <p className="muted-text text-sm">{item.reason}</p>
            </div>
          ))}
        </div>
      </HelpPopover>
    </span>
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
