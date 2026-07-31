import { useState } from 'react'
import type {
  AnonymizeData,
  AppSettings,
  PreparedAnalysis,
  PreviewData,
} from '../types'
import { useAnonymizeJob } from './useAnonymizeJob'
import { useCsvAnalysis } from './useCsvAnalysis'
import { useCsvSelection } from './useCsvSelection'
import { useLocalAi } from './useLocalAi'
import { usePersistentSettings } from './usePersistentSettings'
import { usePreviewWorkflow } from './usePreviewWorkflow'
import { useWorkflowArtifacts, useSelectionInvalidation } from './useWorkflowArtifacts'
import type { BusyState, WorkflowShell } from './workflowTypes'

export function useAnonymizerWorkflow() {
  const [inputPath, setInputPath] = useState('')
  const [outputPath, setOutputPath] = useState('')
  const [busy, setBusy] = useState<BusyState>('idle')
  const [error, setError] = useState<string | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [preparedAnalysis, setPreparedAnalysis] = useState<PreparedAnalysis | null>(null)
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
    highRiskColumns,
    detectedRiskColumns,
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
    updateColumnStrategy: updateCsvColumnStrategy,
    toggleColumn: toggleCsvColumn,
  } = useCsvSelection()
  const { preview, result, setPreview, setResult, clearArtifacts } =
    useWorkflowArtifacts<PreviewData, AnonymizeData>()
  const { settings, settingsLoaded, latestSettingsRef, persistSettings, refreshSettings } =
    usePersistentSettings({
      onError: setError,
    })
  const localAi = useLocalAi(settings, setError)
  const shell: WorkflowShell = { busy, setBusy, setError, setResult, settings, localAi }
  const csvAnalysis = useCsvAnalysis(shell, {
    settingsLoaded,
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
    setPreparedAnalysis,
  })

  const hasFile = Boolean(inputPath.trim())
  const isLoading = busy !== 'idle'
  const settingsDisabled = isLoading || !settingsLoaded
  const localAiBlocked =
    selectionUsesLocalAi(selectedColumns) && (!localAi.ready || localAi.downloadRunning)
  const previewWorkflow = usePreviewWorkflow(shell, {
    inputPath,
    selectedColumns,
    hasColumns,
    hasSelectedColumns,
    localAiBlocked,
    controlsForColumns,
    selectionUsesLocalAi,
    setPreview,
    preparedAnalysis,
  })
  const anonymizeJob = useAnonymizeJob(shell, {
    inputPath,
    outputPath,
    selectedColumns,
    selectedControls,
    hasColumns,
    hasSelectedColumns,
    headers,
    previewSmartReplacements: preview?.smartReplacements ?? [],
    localAiBlocked,
    persistSettings,
    refreshSettings,
    preparedAnalysis,
  })
  const invalidatingSelection = useSelectionInvalidation(
    { setSelectedColumns: setCsvSelectedColumns, toggleColumn: toggleCsvColumn, updateColumnStrategy: updateCsvColumnStrategy },
    clearArtifacts,
  )

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    if (!settingsLoaded) return

    const nextSettings = { ...latestSettingsRef.current, [key]: value }
    if (
      key === 'previewSampleCount' ||
      key === 'localAiEnabled'
    ) {
      clearArtifacts()
    }
    if (key === 'localNerEnabled' || key === 'localAiModel' || key === 'sampleRowCount') {
      // Only detector inputs invalidate the prepared snapshot. Presentation,
      // destination and Smart-replacement settings do not change its evidence.
      resetData()
    }
    if (key === 'defaultOutputSuffix') {
      csvAnalysis.updateOutputPathSuffix(String(value))
    }
    void persistSettings(nextSettings)
  }

  function resetData() {
    setPreparedAnalysis(null)
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
    preparedAnalysis,
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
    localAiBlocked,
    columns,
    selectedSet,
    highRiskColumns,
    detectedRiskColumns,
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
    setColumnSelection: invalidatingSelection.setColumnSelection,
    updateColumnStrategy: invalidatingSelection.updateColumnStrategy,
    toggleColumn: invalidatingSelection.toggleColumn,
    clearFile: csvAnalysis.clearFile,
    handleInputChange: csvAnalysis.handleInputChange,
    maybeLoadManualPath: csvAnalysis.maybeLoadManualPath,
  }
}

export type AnonymizerWorkflowState = ReturnType<typeof useAnonymizerWorkflow>
