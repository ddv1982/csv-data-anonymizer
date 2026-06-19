import { ChevronDown, ChevronUp } from 'lucide-react'
import type { AnonymizationStrategy, ColumnControl, ColumnMetadata, DataType } from '../types'
import { formatToken } from '../utils/format'
import { hasSampleData, isSelectableColumn, maxVisibleColumns } from '../utils/columns'
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
  onTypeChange,
  onStrategyChange,
  onToggleShowAll,
}: {
  columns: ColumnMetadata[]
  allColumnCount: number
  selectedSet: Set<number>
  loading: boolean
  showAllColumns: boolean
  hiddenColumnCount: number
  onToggleColumn: (column: ColumnMetadata) => void
  controls: Record<number, ColumnControl>
  onTypeChange: (column: ColumnMetadata, value: DataType | 'auto') => void
  onStrategyChange: (column: ColumnMetadata, value: AnonymizationStrategy) => void
  onToggleShowAll: () => void
}) {
  return (
    <div className="table-frame">
      <table>
        <thead>
          <tr>
            <th className="checkbox-column" aria-label="Selected"></th>
            <th className="index-column">#</th>
            <th>Column Name</th>
            <th>Type</th>
            <th>Type Override</th>
            <th>Strategy</th>
            <th>Risk</th>
          </tr>
        </thead>
        <tbody>
          {loading ? <ColumnSkeletonRows /> : null}
          {!loading && allColumnCount === 0 ? (
            <tr>
              <td colSpan={7} className="empty-table-cell">
                No columns to display
              </td>
            </tr>
          ) : null}
          {!loading
            ? columns.map((column) => {
                const selectable = isSelectableColumn(column)
                const sampleDataAvailable = hasSampleData(column)
                const control = controls[column.index]
                return (
                  <tr
                    key={`${column.index}-${column.name}`}
                    className={selectable ? 'clickable-row' : 'muted-row'}
                    onClick={() => onToggleColumn(column)}
                  >
                    <td className="checkbox-column">
                      {selectable ? (
                        <input
                          type="checkbox"
                          className="table-checkbox"
                          checked={selectedSet.has(column.index)}
                          onChange={() => onToggleColumn(column)}
                          onClick={(event) => event.stopPropagation()}
                          aria-label={`Select column ${column.name}`}
                        />
                      ) : (
                        <span className="checkbox-placeholder" aria-hidden="true" />
                      )}
                    </td>
                    <td className="index-column mono muted-text">{column.index}</td>
                    <td>
                      <span className={sampleDataAvailable ? 'column-name' : 'column-name no-data'}>
                        {column.name}
                      </span>
                      {!sampleDataAvailable ? (
                        <span className="column-note">(no sample data)</span>
                      ) : column.piiRisk === 'low' ? (
                        <span className="column-note">(low risk - no PII)</span>
                      ) : null}
                    </td>
                    <td className="muted-text">{formatToken(column.detectedType)}</td>
                    <td>
                      <select
                        value={control?.typeOverride ?? 'auto'}
                        disabled={!selectable || loading}
                        aria-label={`Type override for ${column.name}`}
                        onClick={(event) => event.stopPropagation()}
                        onChange={(event) => onTypeChange(column, event.target.value as DataType | 'auto')}
                      >
                        <option value="auto">Auto</option>
                        {dataTypes.map((dataType) => (
                          <option key={dataType} value={dataType}>
                            {formatToken(dataType)}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td>
                      <select
                        value={control?.strategy ?? column.strategy ?? 'auto'}
                        disabled={!selectable || loading}
                        aria-label={`Strategy for ${column.name}`}
                        onClick={(event) => event.stopPropagation()}
                        onChange={(event) => onStrategyChange(column, event.target.value as AnonymizationStrategy)}
                      >
                        {strategies.map((strategy) => (
                          <option key={strategy} value={strategy}>
                            {strategyLabel(strategy)}
                          </option>
                        ))}
                      </select>
                    </td>
                    <td>
                      <RiskBadge risk={column.piiRisk} />
                    </td>
                  </tr>
                )
              })
            : null}
          {!loading && allColumnCount > maxVisibleColumns ? (
            <tr>
              <td colSpan={7} className="show-more-cell">
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

const dataTypes: DataType[] = [
  'email',
  'uuid',
  'timestamp',
  'numericId',
  'numericValue',
  'postalCode',
  'address',
  'ipAddress',
  'url',
  'macAddress',
  'taxId',
  'boolean',
  'currency',
  'percentage',
  'countryCode',
  'phone',
  'firstName',
  'lastName',
  'fullName',
  'enum',
  'string',
  'unknown',
]

const strategies: AnonymizationStrategy[] = ['auto', 'pseudonymize', 'tokenize', 'localAi', 'mask', 'passThrough']

function strategyLabel(strategy: AnonymizationStrategy) {
  if (strategy === 'localAi') {
    return 'Smart replacement (Local AI)'
  }
  return formatToken(strategy)
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
        </tr>
      ))}
    </>
  )
}
