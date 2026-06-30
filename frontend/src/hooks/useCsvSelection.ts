import { useState } from 'react'
import type { AnalyzeResponse } from '../types'
import { useColumnSelection } from './useColumnSelection'

export function useCsvSelection() {
  const [headers, setHeaders] = useState<AnalyzeResponse['headers'] | null>(null)
  const selection = useColumnSelection(headers?.columns)
  const hasColumns = Boolean(headers)
  const hasSelectedColumns = selection.selectedColumns.length > 0

  function setLoadedCsv(nextHeaders: AnalyzeResponse['headers'], nextSelectedColumns: number[]) {
    setHeaders(nextHeaders)
    selection.setSelectedColumns(nextSelectedColumns)
    selection.resetColumnControls()
  }

  function resetCsvSelection() {
    setHeaders(null)
    selection.resetColumnSelection()
  }

  return {
    headers,
    setHeaders,
    ...selection,
    hasColumns,
    hasSelectedColumns,
    setLoadedCsv,
    resetCsvSelection,
  }
}
