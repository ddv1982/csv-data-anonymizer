import { useEffect, useRef } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { firstPreflightBlocker, preflightAnonymization, previewAnonymization } from '../tauri'
import type {
  ColumnControl,
  PreviewData,
  PreparedAnalysis,
} from '../types'
import { messageFrom } from '../utils/errors'
import { isValidTokenizationKey } from '../utils/tokenizationKey'
import type { WorkflowShell } from './workflowTypes'

type PreviewWorkflowArgs = {
  inputPath: string
  selectedColumns: number[]
  hasColumns: boolean
  hasSelectedColumns: boolean
  localAiBlocked: boolean
  controlsForColumns: (columns: number[]) => ColumnControl[]
  selectionUsesLocalAi: (columns: number[]) => boolean
  selectionUsesTokenization: (columns: number[]) => boolean
  setPreview: Dispatch<SetStateAction<PreviewData | null>>
  preparedAnalysis: PreparedAnalysis | null
  tokenizationKey: string | null
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
    selectionUsesTokenization,
    setPreview,
    preparedAnalysis,
    tokenizationKey,
  }: PreviewWorkflowArgs,
) {
  const { busy, setBusy, setError, setResult, settings, localAi } = shell
  const localAiRequest = localAi.request
  const localAiReady = localAi.ready
  const selectedUsesTokenization = selectionUsesTokenization(selectedColumns)
  const selectedTokenizationKey = selectedUsesTokenization ? tokenizationKey : null
  const operationSequence = useRef(0)
  const selectedColumnsFingerprint = selectedColumns.join(',')

  useEffect(() => {
    operationSequence.current += 1
    if (busy === 'preview') setBusy('idle')
  }, [inputPath, selectedColumnsFingerprint, tokenizationKey, preparedAnalysis])
  useEffect(() => () => {
    operationSequence.current += 1
  }, [])

  const canPreview = Boolean(
    hasColumns &&
      hasSelectedColumns &&
      inputPath &&
      busy === 'idle' &&
      (!settings.localNerEnabled || Boolean(preparedAnalysis)) &&
      !localAiBlocked &&
      isValidTokenizationKey(selectedTokenizationKey),
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
    const tokenizationKeyForPreview = selectionUsesTokenization(columnsToPreview)
      ? tokenizationKey
      : null
    if (!isValidTokenizationKey(tokenizationKeyForPreview)) {
      setError('Enter a valid 64-character hexadecimal tokenization key before previewing.')
      return
    }

    const sequence = ++operationSequence.current
    setBusy('preview')
    setError(null)
    try {
      const controls = controlsForColumns(columnsToPreview)
      const preflight = await preflightAnonymization({
        mode: 'preview',
        filePath: path,
        outputPath: null,
        columns: columnsToPreview,
        controls,
        force: false,
        sampleRowCount: settings.sampleRowCount,
        previewSmartReplacements: [],
        localAi: localAiRequest,
        preparedAnalysis,
      })
      if (sequence !== operationSequence.current) return
      const blocker = firstPreflightBlocker(preflight)
      if (blocker) {
        setPreview(null)
        setError(blocker)
        return
      }
      const nextPreview = await previewAnonymization({
        filePath: path,
        columns: columnsToPreview,
        controls,
        sampleCount: settings.previewSampleCount,
        sampleRowCount: settings.sampleRowCount,
        localAi: localAiRequest,
        preparedAnalysis,
        tokenizationKey: tokenizationKeyForPreview,
      })
      if (sequence !== operationSequence.current) return
      setPreview(nextPreview)
      setResult(null)
    } catch (caught) {
      if (sequence === operationSequence.current) setError(messageFrom(caught))
    } finally {
      if (sequence === operationSequence.current) setBusy('idle')
    }
  }

  return {
    canPreview,
    previewCsv,
  }
}
