import type { ReactNode } from 'react'
import type { AnonymizationStrategy, ColumnControl, ColumnMetadata } from '../types'
import { csvStrategies } from '../dataOptions'
import { ColumnTable } from './ColumnTable'

type SelectionAction = {
  label: string
  disabled: boolean
  onClick: () => void
}

export function ColumnSelectionPanel({
  actions,
  notice,
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
  footer,
}: {
  actions: SelectionAction[]
  notice?: ReactNode
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
  footer: ReactNode
}) {
  return (
    <div className="columns-stack">
      <div className="bulk-actions">
        {actions.map((action) => (
          <button
            key={action.label}
            type="button"
            className="button button-outline button-sm"
            disabled={action.disabled}
            onClick={action.onClick}
          >
            {action.label}
          </button>
        ))}
      </div>

      {notice}

      <ColumnTable
        columns={columns}
        allColumnCount={allColumnCount}
        selectedSet={selectedSet}
        loading={loading}
        showAllColumns={showAllColumns}
        hiddenColumnCount={hiddenColumnCount}
        onToggleColumn={onToggleColumn}
        controls={controls}
        onStrategyChange={onStrategyChange}
        onToggleShowAll={onToggleShowAll}
        availableStrategies={availableStrategies}
      />

      {footer}
    </div>
  )
}
