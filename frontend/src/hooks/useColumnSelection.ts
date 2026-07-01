import { useMemo, useState } from 'react'
import type { AnonymizationStrategy, ColumnControl, ColumnMetadata } from '../types'
import { maxVisibleColumns } from '../utils/columns'

const EMPTY_COLUMNS: ColumnMetadata[] = []

export function useColumnSelection(
  nextColumns: ColumnMetadata[] | null | undefined,
  options: { pruneDefaultControls?: boolean } = {},
) {
  const columns = nextColumns ?? EMPTY_COLUMNS
  const [selectedColumns, setSelectedColumnsState] = useState<number[]>([])
  const [columnControls, setColumnControls] = useState<Record<number, ColumnControl>>({})
  const [showAllColumns, setShowAllColumns] = useState(false)

  const selectedSet = useMemo(() => new Set(selectedColumns), [selectedColumns])
  const columnControlList = useMemo(
    () => Object.values(columnControls).sort((left, right) => left.columnIndex - right.columnIndex),
    [columnControls],
  )
  const selectedControls = useMemo(
    () => selectedColumns.map((index) => columnControls[index]).filter(Boolean),
    [columnControls, selectedColumns],
  )
  const highRiskColumns = useMemo(
    () => columns.filter((column) => column.piiRisk === 'high').map((column) => column.index),
    [columns],
  )
  const detectedRiskColumns = useMemo(
    () => columns.filter((column) => column.piiRisk === 'high' || column.piiRisk === 'medium').map((column) => column.index),
    [columns],
  )
  const visibleColumns = showAllColumns ? columns : columns.slice(0, maxVisibleColumns)
  const hiddenColumnCount = Math.max(0, columns.length - visibleColumns.length)
  const allSelected = columns.length > 0 && columns.every((column) => selectedSet.has(column.index))

  function setSelectedColumns(nextSelectedColumns: number[]) {
    const uniqueSorted = [...new Set(nextSelectedColumns)].sort((left, right) => left - right)
    setSelectedColumnsState(uniqueSorted)
  }

  function resetColumnSelection() {
    setSelectedColumnsState([])
    setColumnControls({})
    setShowAllColumns(false)
  }

  function resetColumnControls() {
    setColumnControls({})
    setShowAllColumns(false)
  }

  function controlsForColumns(columnIndexes: number[]) {
    return columnIndexes.map((index) => columnControls[index]).filter(Boolean)
  }

  function selectionUsesLocalAi(columnIndexes: number[]) {
    return columnIndexes.some((index) => {
      const column = columns.find((candidate) => candidate.index === index)
      return (columnControls[index]?.strategy ?? column?.strategy ?? 'auto') === 'localAi'
    })
  }

  function updateColumnControl(
    column: ColumnMetadata,
    patch: Partial<Pick<ColumnControl, 'strategy'>>,
  ) {
    setColumnControls((current) => {
      const next = { ...defaultControl(column), ...current[column.index], ...patch }
      if (!options.pruneDefaultControls || next.strategy !== (column.strategy ?? 'auto')) {
        return { ...current, [column.index]: next }
      }

      const nextControls = { ...current }
      delete nextControls[column.index]
      return nextControls
    })
  }

  function updateColumnStrategy(column: ColumnMetadata, strategy: AnonymizationStrategy) {
    updateColumnControl(column, { strategy })
  }

  function toggleColumn(column: ColumnMetadata) {
    const next = selectedSet.has(column.index)
      ? selectedColumns.filter((index) => index !== column.index)
      : [...selectedColumns, column.index]

    setSelectedColumns(next)
  }

  return {
    selectedColumns,
    columnControls,
    columnControlList,
    showAllColumns,
    setShowAllColumns,
    columns,
    selectedSet,
    selectedControls,
    highRiskColumns,
    detectedRiskColumns,
    visibleColumns,
    hiddenColumnCount,
    allSelected,
    setSelectedColumns,
    setColumnControls,
    resetColumnSelection,
    resetColumnControls,
    controlsForColumns,
    selectionUsesLocalAi,
    updateColumnStrategy,
    toggleColumn,
  }
}

function defaultControl(column: ColumnMetadata): ColumnControl {
  return {
    columnIndex: column.index,
    typeOverride: null,
    strategy: column.strategy ?? 'auto',
  }
}
