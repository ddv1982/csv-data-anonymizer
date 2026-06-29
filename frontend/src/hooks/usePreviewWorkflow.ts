import type { Dispatch, SetStateAction } from 'react'
import { firstPreflightBlocker, preflightAnonymization, previewAnonymization } from '../tauri'
import type {
  AnonymizeData,
  AppSettings,
  ColumnControl,
  LocalAiRequest,
  PreviewData,
} from '../types'
import { messageFrom } from '../utils/errors'
import type { BusyState } from './workflowTypes'

type PreviewWorkflowOptions = {
  inputPath: string
  selectedColumns: number[]
  hasColumns: boolean
  hasSelectedColumns: boolean
  busy: BusyState
  localAiReady: boolean
  localAiBlocked: boolean
  settings: AppSettings
  localAiRequest: LocalAiRequest
  controlsForColumns: (columns: number[]) => ColumnControl[]
  selectionUsesLocalAi: (columns: number[]) => boolean
  setBusy: Dispatch<SetStateAction<BusyState>>
  setError: Dispatch<SetStateAction<string | null>>
  setPreview: Dispatch<SetStateAction<PreviewData | null>>
  setResult: Dispatch<SetStateAction<AnonymizeData | null>>
}

export function usePreviewWorkflow({
  inputPath,
  selectedColumns,
  hasColumns,
  hasSelectedColumns,
  busy,
  localAiReady,
  localAiBlocked,
  settings,
  localAiRequest,
  controlsForColumns,
  selectionUsesLocalAi,
  setBusy,
  setError,
  setPreview,
  setResult,
}: PreviewWorkflowOptions) {
  const canPreview = Boolean(hasColumns && hasSelectedColumns && inputPath && busy === 'idle' && !localAiBlocked)

  async function previewCsv(path = inputPath, columnsToPreview = selectedColumns) {
    if (!path || columnsToPreview.length === 0) {
      setPreview(null)
      return
    }
    if (selectionUsesLocalAi(columnsToPreview) && !localAiReady) {
      setError('Set up Local AI before previewing Smart replacement columns.')
      return
    }

    setBusy('preview')
    setError(null)
    try {
      const controls = controlsForColumns(columnsToPreview)
      const preflight = await preflightAnonymization(
        'preview',
        path,
        null,
        columnsToPreview,
        controls,
        settings.deterministicDefault,
        settings.seed,
        false,
        settings.previewSampleCount,
        null,
        [],
        localAiRequest,
      )
      const blocker = firstPreflightBlocker(preflight)
      if (blocker) {
        setPreview(null)
        setError(blocker)
        return
      }
      const nextPreview = await previewAnonymization(
        path,
        columnsToPreview,
        controls,
        settings.deterministicDefault,
        settings.seed,
        settings.previewSampleCount,
        localAiRequest,
      )
      setPreview(nextPreview)
      setResult(null)
    } catch (caught) {
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  return {
    canPreview,
    previewCsv,
  }
}
