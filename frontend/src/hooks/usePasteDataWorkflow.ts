import { useEffect, useMemo, useRef, useState } from 'react'
import { byteLength, MAX_PASTE_CONTENT_BYTES } from '../limits'
import { analyzePasteData, previewPasteData, transformPasteData } from '../tauri'
import type {
  AppSettings,
  PasteAnalyzeData,
  PasteDataFormat,
  PasteTransformData,
  PreviewData,
} from '../types'
import { messageFrom } from '../utils/errors'
import { confirmEphemeralTokenizationKey, isValidTokenizationKey } from '../utils/tokenizationKey'
import { useColumnSelection } from './useColumnSelection'
import { useCopyOutput } from './useCopyOutput'
import type { LocalAiState } from './useLocalAi'
import { useSelectionInvalidation } from './useWorkflowArtifacts'

export type PasteBusyState = 'idle' | 'analyzing' | 'previewing' | 'transforming' | 'copying'

type PasteDataWorkflowOptions = {
  settings: AppSettings
  settingsLoaded: boolean
  localAi: LocalAiState
  onError: (message: string | null) => void
  tokenizationKey?: string | null
}

export function usePasteDataWorkflow({
  settings,
  settingsLoaded,
  localAi,
  onError,
  tokenizationKey = null,
}: PasteDataWorkflowOptions) {
  const [format, setFormatState] = useState<PasteDataFormat>('auto')
  const [content, setContentState] = useState('')
  const [analysis, setAnalysis] = useState<PasteAnalyzeData | null>(null)
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<PasteTransformData | null>(null)
  const [busy, setBusy] = useState<PasteBusyState>('idle')
  const operationSequence = useRef(0)
  const detectionSettingsFingerprint = `${settings.sampleRowCount}:${settings.localNerEnabled}:${settings.localAiModel}`
  const previousDetectionSettings = useRef(detectionSettingsFingerprint)
  const selection = useColumnSelection(analysis?.columns, { pruneDefaultControls: true })

  const isBusy = busy !== 'idle'
  const { copyOutput, copyStatus, setCopyStatus } = useCopyOutput({ isBusy, onError, setBusy })
  const contentByteLength = useMemo(() => byteLength(content), [content])
  const isContentTooLarge = contentByteLength > MAX_PASTE_CONTENT_BYTES
  const selectedUsesLocalAi = selection.selectionUsesLocalAi(selection.selectedColumns)
  const selectedUsesTokenization = selection.selectionUsesTokenization(selection.selectedColumns)
  const activeTokenizationKey = selectedUsesTokenization ? tokenizationKey : null
  const localAiBlocked = selectedUsesLocalAi && (!localAi.ready || localAi.downloadRunning)
  const canAnalyze = settingsLoaded && content.trim().length > 0 && !isBusy && !isContentTooLarge
  const canClear = !isBusy && (content.length > 0 || analysis !== null || preview !== null || result !== null || copyStatus !== null)
  // Preview and transform are gated on exactly the same conditions: both send the
  // current selection to the backend, so anything that makes one unsafe makes the
  // other unsafe too. They were two identical expressions that could drift apart.
  const canRun =
    settingsLoaded &&
    Boolean(analysis) &&
    selection.selectedColumns.length > 0 &&
    (!settings.localNerEnabled || Boolean(analysis?.preparedAnalysis)) &&
    !isBusy &&
    !localAiBlocked &&
    isValidTokenizationKey(activeTokenizationKey)

  useEffect(() => () => {
    operationSequence.current += 1
  }, [])

  useEffect(() => {
    if (previousDetectionSettings.current === detectionSettingsFingerprint) return
    previousDetectionSettings.current = detectionSettingsFingerprint
    // Content and its chosen format are user input. Everything else was derived
    // under the old detector mode and must be detected again.
    operationSequence.current += 1
    setBusy('idle')
    setAnalysis(null)
    selection.resetColumnSelection()
    setPreview(null)
    setResult(null)
    setCopyStatus(null)
  }, [detectionSettingsFingerprint, selection, setCopyStatus])

  function resetDerivedState() {
    operationSequence.current += 1
    setBusy('idle')
    setAnalysis(null)
    selection.resetColumnSelection()
    setPreview(null)
    setResult(null)
    setCopyStatus(null)
  }

  function setContent(nextContent: string) {
    setContentState(nextContent)
    resetDerivedState()
  }

  function setFormat(nextFormat: PasteDataFormat) {
    setFormatState(nextFormat)
    resetDerivedState()
  }

  async function analyze() {
    if (!canAnalyze) return
    const sequence = ++operationSequence.current
    onError(null)
    setBusy('analyzing')
    setCopyStatus(null)
    setPreview(null)
    setResult(null)
    try {
      const nextAnalysis = await analyzePasteData({
        content,
        format,
        sampleRowCount: settings.sampleRowCount,
      })
      if (sequence !== operationSequence.current) return
      setAnalysis(nextAnalysis)
      selection.setSelectedColumns(
        nextAnalysis.columns.filter((column) => column.isSelected).map((column) => column.index),
      )
      selection.resetColumnControls()
    } catch (caught) {
      if (sequence === operationSequence.current) onError(messageFrom(caught))
    } finally {
      if (sequence === operationSequence.current) setBusy('idle')
    }
  }

  function clear() {
    if (!canClear) return
    onError(null)
    setContentState('')
    resetDerivedState()
  }

  async function showPreview() {
    if (!settingsLoaded || !analysis || selection.selectedColumns.length === 0 || isBusy) return
    if (localAiBlocked) {
      onError('Set up Local AI before previewing Smart replacement fields.')
      return
    }
    if (settings.localNerEnabled && !analysis.preparedAnalysis) {
      onError('Analyze the content again before previewing.')
      return
    }
    if (!isValidTokenizationKey(activeTokenizationKey)) {
      onError('Enter a valid 64-character hexadecimal tokenization key before previewing.')
      return
    }
    const sequence = ++operationSequence.current
    onError(null)
    setBusy('previewing')
    setCopyStatus(null)
    setResult(null)
    try {
      const nextPreview = await previewPasteData({
        content,
        format: analysis.format,
        columns: selection.selectedColumns,
        controls: selection.controlsForColumns(selection.selectedColumns),
        sampleCount: settings.previewSampleCount,
        sampleRowCount: settings.sampleRowCount,
        localAi: localAi.request,
        preparedAnalysis: analysis.preparedAnalysis,
        tokenizationKey: activeTokenizationKey,
      })
      if (sequence === operationSequence.current) setPreview(nextPreview)
    } catch (caught) {
      if (sequence === operationSequence.current) onError(messageFrom(caught))
    } finally {
      if (sequence === operationSequence.current) setBusy('idle')
    }
  }

  async function transform() {
    if (!settingsLoaded || !analysis || selection.selectedColumns.length === 0 || isBusy) return
    if (localAiBlocked) {
      onError('Set up Local AI before anonymizing Smart replacement fields.')
      return
    }
    if (settings.localNerEnabled && !analysis.preparedAnalysis) {
      onError('Analyze the content again before transforming.')
      return
    }
    if (!isValidTokenizationKey(activeTokenizationKey)) {
      onError('Enter a valid 64-character hexadecimal tokenization key before transforming.')
      return
    }
    if (!confirmEphemeralTokenizationKey(activeTokenizationKey)) return
    const sequence = ++operationSequence.current
    onError(null)
    setBusy('transforming')
    setCopyStatus(null)
    try {
      const nextResult = await transformPasteData({
        content,
        format: analysis.format,
        columns: selection.selectedColumns,
        controls: selection.controlsForColumns(selection.selectedColumns),
        sampleRowCount: settings.sampleRowCount,
        previewSmartReplacements: preview?.smartReplacements ?? [],
        localAi: localAi.request,
        preparedAnalysis: analysis.preparedAnalysis,
        tokenizationKey: activeTokenizationKey,
      })
      if (sequence === operationSequence.current) setResult(nextResult)
    } catch (caught) {
      if (sequence === operationSequence.current) onError(messageFrom(caught))
    } finally {
      if (sequence === operationSequence.current) setBusy('idle')
    }
  }

  function clearOutput() {
    operationSequence.current += 1
    setBusy('idle')
    setResult(null)
    setPreview(null)
  }

  const invalidatingSelection = useSelectionInvalidation(
    selection,
    () => {
      clearOutput()
    },
    () => isBusy,
  )

  return {
    format,
    content,
    analysis,
    preview,
    result,
    busy,
    selection,
    copyStatus,
    contentByteLength,
    isContentTooLarge,
    selectedUsesLocalAi,
    localAiBlocked,
    isBusy,
    canAnalyze,
    canClear,
    canRun,
    setFormat,
    setContent,
    analyze,
    clear,
    showPreview,
    transform,
    copyOutput: () => copyOutput(result?.output),
    setColumnSelection: invalidatingSelection.setColumnSelection,
    toggleColumn: invalidatingSelection.toggleColumn,
    updateColumnStrategy: invalidatingSelection.updateColumnStrategy,
  }
}

export type PasteDataWorkflowState = ReturnType<typeof usePasteDataWorkflow>
