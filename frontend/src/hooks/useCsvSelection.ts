import { useMemo, useState } from 'react'
import type {
  AnalyzeResponse,
  AnonymizationStrategy,
  ColumnControl,
  ColumnMetadata,
  ColumnRole,
  DataType,
  PrivacyConfig,
} from '../types'
import { isSelectableColumn, maxVisibleColumns } from '../utils/columns'

const EMPTY_COLUMNS: ColumnMetadata[] = []

export function useCsvSelection() {
  const [headers, setHeaders] = useState<AnalyzeResponse['headers'] | null>(null)
  const [selectedColumns, setSelectedColumnsState] = useState<number[]>([])
  const [columnControls, setColumnControls] = useState<Record<number, ColumnControl>>({})
  const [showAllColumns, setShowAllColumns] = useState(false)

  const columns = headers?.columns ?? EMPTY_COLUMNS
  const selectedSet = useMemo(() => new Set(selectedColumns), [selectedColumns])
  const selectedControls = useMemo(
    () => selectedColumns.map((index) => columnControls[index]).filter(Boolean),
    [columnControls, selectedColumns],
  )
  const selectableColumns = useMemo(() => columns.filter(isSelectableColumn), [columns])
  const highRiskColumns = useMemo(
    () => selectableColumns.filter((column) => column.piiRisk === 'high').map((column) => column.index),
    [selectableColumns],
  )
  const visibleColumns =
    showAllColumns || columns.length <= maxVisibleColumns ? columns : columns.slice(0, maxVisibleColumns)
  const hiddenColumnCount = Math.max(columns.length - maxVisibleColumns, 0)
  const allSelected =
    selectableColumns.length > 0 && selectableColumns.every((column) => selectedSet.has(column.index))
  const hasColumns = Boolean(headers)
  const hasSelectedColumns = selectedColumns.length > 0

  function setSelectedColumns(nextColumns: number[]) {
    const uniqueSorted = [...new Set(nextColumns)].sort((left, right) => left - right)
    setSelectedColumnsState(uniqueSorted)
  }

  function setLoadedCsv(nextHeaders: AnalyzeResponse['headers'], nextSelectedColumns: number[]) {
    setHeaders(nextHeaders)
    setSelectedColumns(nextSelectedColumns)
    setColumnControls({})
    setShowAllColumns(false)
  }

  function resetCsvSelection() {
    setHeaders(null)
    setSelectedColumnsState([])
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
    patch: Partial<Pick<ColumnControl, 'typeOverride' | 'strategy'>>,
  ) {
    setColumnControls((current) => ({
      ...current,
      [column.index]: { ...defaultControl(column), ...current[column.index], ...patch },
    }))
  }

  function updateColumnType(column: ColumnMetadata, value: DataType | 'auto') {
    updateColumnControl(column, { typeOverride: value === 'auto' ? null : value })
  }

  function updateColumnStrategy(column: ColumnMetadata, strategy: AnonymizationStrategy) {
    updateColumnControl(column, { strategy })
  }

  function updateColumnRole(privacyConfig: PrivacyConfig, column: ColumnMetadata, role: ColumnRole): PrivacyConfig {
    const existing = privacyConfig.columnRoles.find((candidate) => candidate.columnIndex === column.index)
    const nextRole = {
      columnIndex: column.index,
      role,
      generalizationLevel: existing?.generalizationLevel ?? 0,
    }
    return {
      ...privacyConfig,
      columnRoles:
        role === 'auto' && nextRole.generalizationLevel === 0
          ? privacyConfig.columnRoles.filter((candidate) => candidate.columnIndex !== column.index)
          : [
              ...privacyConfig.columnRoles.filter((candidate) => candidate.columnIndex !== column.index),
              nextRole,
            ].sort((left, right) => left.columnIndex - right.columnIndex),
    }
  }

  function toggleColumn(column: ColumnMetadata) {
    if (!isSelectableColumn(column)) return

    const next = selectedSet.has(column.index)
      ? selectedColumns.filter((index) => index !== column.index)
      : [...selectedColumns, column.index]

    setSelectedColumns(next)
  }

  return {
    headers,
    setHeaders,
    selectedColumns,
    columnControls,
    showAllColumns,
    setShowAllColumns,
    columns,
    selectedSet,
    selectedControls,
    selectableColumns,
    highRiskColumns,
    visibleColumns,
    hiddenColumnCount,
    allSelected,
    hasColumns,
    hasSelectedColumns,
    setSelectedColumns,
    setLoadedCsv,
    resetCsvSelection,
    setColumnControls,
    controlsForColumns,
    selectionUsesLocalAi,
    updateColumnType,
    updateColumnStrategy,
    updateColumnRole,
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
