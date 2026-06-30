import { useState } from 'react'
import type {
  AnonymizationStrategy,
  AppSettings,
  ColumnMetadata,
  DataType,
} from '../types'
import { useAnonymizeJob } from './useAnonymizeJob'
import { useCsvAnalysis } from './useCsvAnalysis'
import { useCsvSelection } from './useCsvSelection'
import { useLocalAi } from './useLocalAi'
import { usePersistentSettings } from './usePersistentSettings'
import { usePreviewWorkflow } from './usePreviewWorkflow'
import { useWorkflowArtifacts } from './useWorkflowArtifacts'
import type { BusyState } from './workflowTypes'

export function useAnonymizerWorkflow() {
  const [inputPath, setInputPath] = useState('')
  const [outputPath, setOutputPath] = useState('')
  const [busy, setBusy] = useState<BusyState>('idle')
  const [error, setError] = useState<string | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const {
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
    setSelectedColumns: setCsvSelectedColumns,
    setLoadedCsv,
    resetCsvSelection,
    setColumnControls,
    controlsForColumns,
    selectionUsesLocalAi,
    updateColumnType: updateCsvColumnType,
    updateColumnStrategy: updateCsvColumnStrategy,
    toggleColumn: toggleCsvColumn,
  } = useCsvSelection()
  const { preview, result, setPreview, setResult, clearArtifacts } = useWorkflowArtifacts()
  const { settings, settingsLoaded, latestSettingsRef, persistSettings, refreshSettings } =
    usePersistentSettings({
      onError: setError,
    })
  const localAi = useLocalAi(settings, setError)
  const csvAnalysis = useCsvAnalysis({
    settings,
    settingsLoaded,
    busy,
    setBusy,
    setError,
    setResult,
    clearArtifacts,
    persistSettings,
    onResetData: resetData,
    inputPath,
    setInputPath,
    outputPath,
    setOutputPath,
    selection: {
      headers,
      setHeaders,
      setLoadedCsv,
      setColumnControls,
    },
  })

  const localAiSelected = selectionUsesLocalAi(selectedColumns)
  const localAiReady = localAi.ready
  const localAiDownloadRunning = localAi.downloadRunning

  const hasFile = Boolean(inputPath.trim())
  const isLoading = busy !== 'idle'
  const settingsDisabled = isLoading || !settingsLoaded
  const localAiBlocked = localAiSelected && (!localAiReady || localAiDownloadRunning)
  const previewWorkflow = usePreviewWorkflow({
    inputPath,
    selectedColumns,
    hasColumns,
    hasSelectedColumns,
    busy,
    localAiReady,
    localAiBlocked,
    settings,
    localAiRequest: localAi.request,
    controlsForColumns,
    selectionUsesLocalAi,
    setBusy,
    setError,
    setPreview,
    setResult,
  })
  const anonymizeJob = useAnonymizeJob({
    inputPath,
    outputPath,
    selectedColumns,
    selectedControls,
    hasColumns,
    hasSelectedColumns,
    headers,
    settings,
    previewSmartReplacements: preview?.smartReplacements ?? [],
    localAiRequest: localAi.request,
    localAiBlocked,
    busy,
    setBusy,
    setError,
    setResult,
    persistSettings,
    refreshSettings,
  })

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    if (!settingsLoaded) return

    const nextSettings = { ...latestSettingsRef.current, [key]: value }
    if (key === 'deterministicDefault' && value === false) {
      nextSettings.seed = ''
    }
    if (
      key === 'deterministicDefault' ||
      key === 'seed' ||
      key === 'previewSampleCount' ||
      key === 'localAiEnabled' ||
      key === 'localAiModel'
    ) {
      clearArtifacts()
    }
    if (key === 'defaultOutputSuffix') {
      csvAnalysis.updateOutputPathSuffix(String(value))
    }
    void persistSettings(nextSettings)
  }

  function setColumnSelection(nextColumns: number[]) {
    setCsvSelectedColumns(nextColumns)
    clearArtifacts()
  }

  function updateColumnType(column: ColumnMetadata, value: DataType | 'auto') {
    updateCsvColumnType(column, value)
    clearArtifacts()
  }

  function updateColumnStrategy(column: ColumnMetadata, strategy: AnonymizationStrategy) {
    updateCsvColumnStrategy(column, strategy)
    clearArtifacts()
  }

  function toggleColumn(column: ColumnMetadata) {
    toggleCsvColumn(column)
    clearArtifacts()
  }

  function resetData() {
    resetCsvSelection()
    clearArtifacts()
    anonymizeJob.clearJobState()
  }

  return {
    settings,
    settingsLoaded,
    inputPath,
    outputPath,
    headers,
    selectedColumns,
    columnControls,
    preview,
    result,
    jobStatus: anonymizeJob.jobStatus,
    busy,
    error,
    settingsOpen,
    showAllColumns,
    localAi,
    localAiSelected,
    localAiBlocked,
    columns,
    selectedSet,
    selectableColumns,
    highRiskColumns,
    visibleColumns,
    hiddenColumnCount,
    allSelected,
    hasFile,
    hasColumns,
    hasSelectedColumns,
    isLoading,
    settingsDisabled,
    canPreview: previewWorkflow.canPreview,
    canAnonymize: anonymizeJob.canAnonymize,
    setError,
    setSettingsOpen,
    setShowAllColumns,
    updateSetting,
    updateOutputPath: csvAnalysis.updateOutputPath,
    handlePickInput: csvAnalysis.handlePickInput,
    handlePickOutput: csvAnalysis.handlePickOutput,
    previewCsv: previewWorkflow.previewCsv,
    runAnonymization: anonymizeJob.runAnonymization,
    cancelCurrentJob: anonymizeJob.cancelCurrentJob,
    setColumnSelection,
    updateColumnType,
    updateColumnStrategy,
    toggleColumn,
    clearFile: csvAnalysis.clearFile,
    handleInputChange: csvAnalysis.handleInputChange,
    maybeLoadManualPath: csvAnalysis.maybeLoadManualPath,
  }
}

export type AnonymizerWorkflowState = ReturnType<typeof useAnonymizerWorkflow>
