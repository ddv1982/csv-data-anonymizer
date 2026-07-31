import type { Dispatch, SetStateAction } from 'react'
import { firstPreflightBlocker, preflightAnonymization, previewAnonymization } from '../tauri'
import type {
  ColumnControl,
  PreviewData,
  PreparedAnalysis,
} from '../types'
import { messageFrom } from '../utils/errors'
import type { WorkflowShell } from './workflowTypes'

type PreviewWorkflowArgs = {
  inputPath: string
  selectedColumns: number[]
  hasColumns: boolean
  hasSelectedColumns: boolean
  localAiBlocked: boolean
  controlsForColumns: (columns: number[]) => ColumnControl[]
  selectionUsesLocalAi: (columns: number[]) => boolean
  setPreview: Dispatch<SetStateAction<PreviewData | null>>
  preparedAnalysis: PreparedAnalysis | null
}

export function usePreviewWorkflow(
  shell: WorkflowShell,
  {
    inputPath,
    selectedColumns,
    hasColumns,
    hasSelectedColumns,
    localAiBlocked,
    controlsForColumns,
    selectionUsesLocalAi,
    setPreview,
    preparedAnalysis,
  }: PreviewWorkflowArgs,
) {
  const { busy, setBusy, setError, setResult, settings, localAi } = shell
  const localAiRequest = localAi.request
  const localAiReady = localAi.ready

  const canPreview = Boolean(
    hasColumns &&
      hasSelectedColumns &&
      inputPath &&
      busy === 'idle' &&
      (!settings.localNerEnabled || Boolean(preparedAnalysis)) &&
      !localAiBlocked,
  )

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
        false,
        settings.sampleRowCount,
        [],
        localAiRequest,
        ...(preparedAnalysis ? [preparedAnalysis] : []),
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
        settings.previewSampleCount,
        settings.sampleRowCount,
        localAiRequest,
        ...(preparedAnalysis ? [preparedAnalysis] : []),
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
