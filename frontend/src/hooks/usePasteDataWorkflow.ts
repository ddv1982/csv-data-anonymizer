import { useMemo, useState } from 'react'
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
import { useColumnSelection } from './useColumnSelection'
import { useCopyOutput } from './useCopyOutput'
import type { LocalAiState } from './useLocalAi'

export type PasteBusyState = 'idle' | 'analyzing' | 'previewing' | 'transforming' | 'copying'

type PasteDataWorkflowOptions = {
  settings: AppSettings
  settingsLoaded: boolean
  localAi: LocalAiState
  onError: (message: string | null) => void
}

export function usePasteDataWorkflow({
  settings,
  settingsLoaded,
  localAi,
  onError,
}: PasteDataWorkflowOptions) {
  const [format, setFormatState] = useState<PasteDataFormat>('auto')
  const [content, setContentState] = useState('')
  const [analysis, setAnalysis] = useState<PasteAnalyzeData | null>(null)
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<PasteTransformData | null>(null)
  const [busy, setBusy] = useState<PasteBusyState>('idle')
  const selection = useColumnSelection(analysis?.columns, { pruneDefaultControls: true })

  const isBusy = busy !== 'idle'
  const { copyOutput, copyStatus, setCopyStatus } = useCopyOutput({ isBusy, onError, setBusy })
  const contentByteLength = useMemo(() => byteLength(content), [content])
  const isContentTooLarge = contentByteLength > MAX_PASTE_CONTENT_BYTES
  const selectedUsesLocalAi = selection.selectionUsesLocalAi(selection.selectedColumns)
  const localAiBlocked = selectedUsesLocalAi && (!localAi.ready || localAi.downloadRunning)
  const canAnalyze = settingsLoaded && content.trim().length > 0 && !isBusy && !isContentTooLarge
  const canClear = !isBusy && (content.length > 0 || analysis !== null || preview !== null || result !== null || copyStatus !== null)
  const canPreview = settingsLoaded && Boolean(analysis) && selection.selectedColumns.length > 0 && !isBusy && !localAiBlocked
  const canTransform = settingsLoaded && Boolean(analysis) && selection.selectedColumns.length > 0 && !isBusy && !localAiBlocked

  function resetDerivedState() {
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
    onError(null)
    setBusy('analyzing')
    setCopyStatus(null)
    setPreview(null)
    setResult(null)
    try {
      const nextAnalysis = await analyzePasteData(content, format, settings.sampleRowCount)
      setAnalysis(nextAnalysis)
      selection.setSelectedColumns(
        nextAnalysis.columns.filter((column) => column.isSelected).map((column) => column.index),
      )
      selection.resetColumnControls()
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
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
    onError(null)
    setBusy('previewing')
    setCopyStatus(null)
    setResult(null)
    try {
      setPreview(await previewPasteData(
        content,
        analysis.format,
        selection.selectedColumns,
        selection.columnControlList,
        settings.previewSampleCount,
        localAi.request,
      ))
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function transform() {
    if (!settingsLoaded || !analysis || selection.selectedColumns.length === 0 || isBusy) return
    if (localAiBlocked) {
      onError('Set up Local AI before anonymizing Smart replacement fields.')
      return
    }
    onError(null)
    setBusy('transforming')
    setCopyStatus(null)
    try {
      setResult(await transformPasteData(
        content,
        analysis.format,
        selection.selectedColumns,
        selection.columnControlList,
        preview?.smartReplacements ?? [],
        localAi.request,
      ))
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  function clearOutput() {
    setResult(null)
    setPreview(null)
  }

  function setColumnSelection(nextColumns: number[]) {
    selection.setSelectedColumns(nextColumns)
    clearOutput()
  }

  function toggleColumn(column: Parameters<typeof selection.toggleColumn>[0]) {
    selection.toggleColumn(column)
    clearOutput()
  }

  function updateColumnStrategy(
    column: Parameters<typeof selection.updateColumnStrategy>[0],
    strategy: Parameters<typeof selection.updateColumnStrategy>[1],
  ) {
    selection.updateColumnStrategy(column, strategy)
    clearOutput()
  }

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
    canPreview,
    canTransform,
    setFormat,
    setContent,
    analyze,
    clear,
    showPreview,
    transform,
    copyOutput: () => copyOutput(result?.output),
    setColumnSelection,
    toggleColumn,
    updateColumnStrategy,
  }
}
