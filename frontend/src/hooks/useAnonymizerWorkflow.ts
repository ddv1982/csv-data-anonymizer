import { useMemo, useState } from 'react'
import {
  applyBudgetSettingsToPrivacyConfig,
  defaultSettings,
  privacyConfigFromSettings,
  settingsWithPrivacyBudget,
} from '../defaults'
import { DP_BUDGET_RESET_CONFIRMATION_PHRASE, resetDpBudgetLedger } from '../tauri'
import type {
  AnonymizationStrategy,
  AppSettings,
  ColumnMetadata,
  ColumnRole,
  DataType,
  PrivacyConfig,
} from '../types'
import { messageFrom } from '../utils/errors'
import { getPrivacyConfigValidation, getPrivacyScaleWarning } from '../utils/privacy'
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
  const [privacyConfig, setPrivacyConfig] = useState<PrivacyConfig>(() => privacyConfigFromSettings(defaultSettings))
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
    updateColumnRole: updateCsvColumnRole,
    toggleColumn: toggleCsvColumn,
  } = useCsvSelection()
  const { preview, result, setPreview, setResult, clearArtifacts } = useWorkflowArtifacts()
  const { settings, settingsLoaded, latestSettingsRef, applyAuthoritativeSettings, persistSettings, refreshSettings } =
    usePersistentSettings({
      onError: setError,
      onAcceptedSettings: applySettingsBudget,
    })
  const localAi = useLocalAi(settings, setError)
  const csvAnalysis = useCsvAnalysis({
    settings,
    settingsLoaded,
    busy,
    setBusy,
    setError,
    setResult,
    setPrivacyConfig,
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

  const columnRoleControls = useMemo(
    () =>
      Object.fromEntries(privacyConfig.columnRoles.map((role) => [role.columnIndex, role])) as Record<
        number,
        PrivacyConfig['columnRoles'][number]
      >,
    [privacyConfig.columnRoles],
  )
  const localAiSelected = selectionUsesLocalAi(selectedColumns)
  const localAiReady = localAi.ready
  const localAiDownloadRunning = localAi.downloadRunning

  const hasFile = Boolean(inputPath.trim())
  const isLoading = busy !== 'idle'
  const settingsDisabled = isLoading || !settingsLoaded
  const localAiBlocked = localAiSelected && (!localAiReady || localAiDownloadRunning)
  const basePrivacyValidation = getPrivacyConfigValidation(privacyConfig, selectedSet, columns.length)
  const privacyValidation =
    privacyConfig.releaseMode === 'differentialPrivacyAggregate' && settings.deterministicDefault
      ? {
          valid: false,
          reason: 'Turn off Repeatable replacements before creating DP aggregate output.',
        }
      : basePrivacyValidation
  const privacyConfigValid = privacyValidation.valid
  const privacyScaleWarning = getPrivacyScaleWarning(privacyConfig, headers)
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
    privacyConfig,
    privacyConfigValid,
    privacyValidation,
    localAiBlocked,
    settings,
    previewSmartReplacements: preview?.smartReplacements ?? [],
    localAiRequest: localAi.request,
    busy,
    setBusy,
    setError,
    setResult,
    persistSettings,
    refreshSettings,
  })

  function applySettingsBudget(next: AppSettings) {
    setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, next))
  }

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    if (!settingsLoaded) return

    const nextSettings = { ...latestSettingsRef.current, [key]: value }
    if (
      key === 'deterministicDefault' ||
      key === 'seed' ||
      key === 'previewSampleCount' ||
      key === 'localAiEnabled' ||
      key === 'localAiModel' ||
      isDpBudgetSetting(key)
    ) {
      clearArtifacts()
    }
    if (isDpBudgetSetting(key)) {
      setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, nextSettings))
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

  function updatePrivacyConfig(nextConfig: PrivacyConfig) {
    setPrivacyConfig(nextConfig)
    clearArtifacts()
    const currentSettings = latestSettingsRef.current
    const nextSettings = settingsWithPrivacyBudget(currentSettings, nextConfig)
    if (!sameDpBudgetSettings(currentSettings, nextSettings)) {
      void persistSettings(nextSettings)
    }
  }

  async function resetDpBudget() {
    setError(null)
    try {
      const reset = await resetDpBudgetLedger(DP_BUDGET_RESET_CONFIRMATION_PHRASE)
      applyAuthoritativeSettings(reset)
      setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, reset))
      clearArtifacts()
    } catch (caught) {
      setError(messageFrom(caught))
    }
  }

  function updateColumnRole(column: ColumnMetadata, role: ColumnRole) {
    updatePrivacyConfig(updateCsvColumnRole(privacyConfig, column, role))
  }

  function toggleColumn(column: ColumnMetadata) {
    toggleCsvColumn(column)
    clearArtifacts()
  }

  function resetData() {
    resetCsvSelection()
    setPrivacyConfig(privacyConfigFromSettings(settings))
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
    privacyConfig,
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
    columnRoleControls,
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
    privacyValidation,
    privacyScaleWarning,
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
    updatePrivacyConfig,
    resetDpBudget,
    updateColumnRole,
    toggleColumn,
    clearFile: csvAnalysis.clearFile,
    handleInputChange: csvAnalysis.handleInputChange,
    maybeLoadManualPath: csvAnalysis.maybeLoadManualPath,
  }
}

function isDpBudgetSetting(key: keyof AppSettings) {
  return key === 'dpBudgetEnabled' || key === 'dpBudgetLimitEpsilon' || key === 'dpBudgetAction'
}

function sameDpBudgetSettings(left: AppSettings, right: AppSettings) {
  return (
    left.dpBudgetEnabled === right.dpBudgetEnabled &&
    left.dpBudgetLimitEpsilon === right.dpBudgetLimitEpsilon &&
    left.dpBudgetAction === right.dpBudgetAction
  )
}

export type AnonymizerWorkflowState = ReturnType<typeof useAnonymizerWorkflow>
