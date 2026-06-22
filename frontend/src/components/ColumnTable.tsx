import { ChevronDown, ChevronUp } from 'lucide-react'
import type {
  AnonymizationStrategy,
  ColumnControl,
  ColumnMetadata,
  ColumnRole,
  DataType,
  PrivacyColumnRole,
} from '../types'
import { formatToken } from '../utils/format'
import { hasSampleData, isSelectableColumn, maxVisibleColumns } from '../utils/columns'
import { GlossaryLabel } from './GlossaryPopover'
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
  roles,
  onTypeChange,
  onStrategyChange,
  onRoleChange,
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
  roles: Record<number, PrivacyColumnRole>
  onTypeChange: (column: ColumnMetadata, value: DataType | 'auto') => void
  onStrategyChange: (column: ColumnMetadata, value: AnonymizationStrategy) => void
  onRoleChange: (column: ColumnMetadata, value: ColumnRole) => void
  onToggleShowAll: () => void
}) {
  return (
    <div className="table-frame">
      <table className="column-table">
        <thead>
          <tr>
            <th className="checkbox-column" aria-label="Selected"></th>
            <th className="index-column">#</th>
            <th className="column-title-column">Column Name</th>
            <th>Type</th>
            <th>Type Override</th>
            <th>
              <GlossaryLabel term="strategy">Strategy</GlossaryLabel>
            </th>
            <th>
              <GlossaryLabel term="role">Role</GlossaryLabel>
            </th>
            <th>Risk</th>
          </tr>
        </thead>
        <tbody>
          {loading ? <ColumnSkeletonRows /> : null}
          {!loading && allColumnCount === 0 ? (
            <tr>
              <td colSpan={8} className="empty-table-cell">
                No columns to display
              </td>
            </tr>
          ) : null}
          {!loading
            ? columns.map((column) => {
                const selectable = isSelectableColumn(column)
                const sampleDataAvailable = hasSampleData(column)
                const control = controls[column.index]
                const role = roles[column.index]
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
                    <td className="column-title-cell">
                      <span className={sampleDataAvailable ? 'column-name' : 'column-name no-data'}>
                        {column.name}
                      </span>
                      {!sampleDataAvailable ? (
                        <span className="column-note">(no sample data)</span>
                      ) : column.piiRisk === 'low' ? (
                        <span className="column-note">
                          (low risk - no <GlossaryLabel term="pii">PII</GlossaryLabel>)
                        </span>
                      ) : null}
                    </td>
                    <td className="detected-type-cell">
                      <span className="mobile-cell-label">Detected type</span>
                      <span className="muted-text">{formatToken(column.detectedType)}</span>
                    </td>
                    <td className="control-cell">
                      <span className="mobile-cell-label">Type Override</span>
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
                    <td className="control-cell">
                      <span className="mobile-cell-label">Strategy</span>
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
                    <td className="control-cell">
                      <span className="mobile-cell-label">Role</span>
                      <select
                        value={role?.role ?? 'auto'}
                        disabled={!selectable || loading}
                        aria-label={`Privacy role for ${column.name}`}
                        onClick={(event) => event.stopPropagation()}
                        onChange={(event) => onRoleChange(column, event.target.value as ColumnRole)}
                      >
                        {rolesList.map((roleValue) => (
                          <option key={roleValue} value={roleValue}>
                            {roleLabel(roleValue)}
                          </option>
                        ))}
                      </select>
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
              <td colSpan={8} className="show-more-cell">
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
const rolesList: ColumnRole[] = [
  'auto',
  'directIdentifier',
  'quasiIdentifier',
  'sensitive',
  'attribute',
  'exclude',
]

function strategyLabel(strategy: AnonymizationStrategy) {
  if (strategy === 'localAi') {
    return 'Smart replacement (Local AI)'
  }
  return formatToken(strategy)
}

function roleLabel(role: ColumnRole) {
  if (role === 'auto') return 'Auto'
  if (role === 'directIdentifier') return 'Direct ID'
  if (role === 'quasiIdentifier') return 'Quasi-ID'
  return formatToken(role)
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
          <td>
            <span className="skeleton skeleton-badge" />
          </td>
        </tr>
      ))}
    </>
  )
}
