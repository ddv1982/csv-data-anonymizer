import { useState } from 'react'
import type { AnonymizationStrategy, ColumnMetadata } from '../types'

/**
 * The preview and result a workflow has produced for the current selection.
 *
 * Both are artifacts of one exact set of columns and strategies, so any change to
 * that set makes them stale — see `useSelectionInvalidation`.
 */
export function useWorkflowArtifacts<TPreview, TResult>() {
  const [preview, setPreview] = useState<TPreview | null>(null)
  const [result, setResult] = useState<TResult | null>(null)

  function clearArtifacts() {
    setPreview(null)
    setResult(null)
  }

  return {
    preview,
    result,
    setPreview,
    setResult,
    clearArtifacts,
  }
}

type SelectionLike = {
  setSelectedColumns: (columns: number[]) => void
  toggleColumn: (column: ColumnMetadata) => void
  updateColumnStrategy: (column: ColumnMetadata, strategy: AnonymizationStrategy) => void
}

/**
 * Wraps the three selection mutators so each one throws away the stale preview and
 * result.
 *
 * This is the rule that must not be forgotten anywhere: showing a preview built from
 * a different column set beside a changed selection would tell the user data is being
 * protected when it no longer is. Both workflows had written the same three wrappers
 * by hand.
 *
 * `isBlocked` lets a workflow refuse selection edits while an operation is running,
 * which the paste workflow needs and the CSV workflow handles through disabled controls.
 */
export function useSelectionInvalidation(
  selection: SelectionLike,
  invalidate: () => void,
  isBlocked: () => boolean = () => false,
) {
  return {
    setColumnSelection(nextColumns: number[]) {
      if (isBlocked()) return
      selection.setSelectedColumns(nextColumns)
      invalidate()
    },
    toggleColumn(column: ColumnMetadata) {
      if (isBlocked()) return
      selection.toggleColumn(column)
      invalidate()
    },
    updateColumnStrategy(column: ColumnMetadata, strategy: AnonymizationStrategy) {
      if (isBlocked()) return
      selection.updateColumnStrategy(column, strategy)
      invalidate()
    },
  }
}
