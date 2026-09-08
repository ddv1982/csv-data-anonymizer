import { useEffect, useRef } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { firstPreflightBlocker, preflightAnonymization, previewAnonymization } from '../tauri'
import type {
  ColumnControl,
  PreviewData,
  PreparedAnalysis,
} from '../types'
import { messageFrom } from '../utils/errors'
import { getProtectionReadiness } from './workflowReadiness'
import type { WorkflowShell } from './workflowTypes'

type PreviewWorkflowArgs = {
  inputPath: string
  selectedColumns: number[]
  hasColumns: boolean
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
  const operationSequence = useRef(0)
  const selectedColumnsFingerprint = selectedColumns.join(',')

  useEffect(() => {
    operationSequence.current += 1
    if (busy === 'preview') setBusy('idle')
  }, [inputPath, selectedColumnsFingerprint, tokenizationKey, preparedAnalysis])
  useEffect(() => () => {
    operationSequence.current += 1
  }, [])

  function getPreviewReadiness(path: string, columns: number[]) {
    const protection = getProtectionReadiness({
      usesLocalAi: selectionUsesLocalAi(columns),
      localAi,
      requiresPreparedAnalysis: settings.localNerEnabled,
      hasPreparedAnalysis: Boolean(preparedAnalysis),
      usesTokenization: selectionUsesTokenization(columns),
      tokenizationKey,
    })
    const blocker = !path
      ? 'missingSource'
      : columns.length === 0
        ? 'noSelection'
        : !hasColumns
          ? 'missingColumns'
          : busy !== 'idle'
            ? 'busy'
            : protection.blocker
    return { blocker, protection }
  }

  const canPreview = getPreviewReadiness(inputPath, selectedColumns).blocker === null

  async function previewCsv(path = inputPath, columnsToPreview = selectedColumns) {
    const readiness = getPreviewReadiness(path, columnsToPreview)
    if (readiness.blocker === 'missingSource' || readiness.blocker === 'noSelection') {
      setPreview(null)
      return
    }
    if (readiness.blocker === 'missingColumns') {
      setError('Load a CSV file first.')
      return
    }
    if (readiness.blocker === 'busy') {
      setError('Wait for the current operation to finish.')
      return
    }
    if (readiness.blocker === 'localAi') {
      setError('Set up Local AI before previewing Smart replacement columns.')
      return
    }
    if (readiness.blocker === 'preparedAnalysis') {
      setError('Analyze the source again before previewing.')
      return
    }
    if (readiness.blocker === 'tokenizationKey') {
      setError('Enter a valid 64-character hexadecimal tokenization key before previewing.')
      return
    }
    const tokenizationKeyForPreview = readiness.protection.tokenizationKey
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
