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
  ColumnControl,
  ColumnMetadata,
  ColumnRole,
  DataType,
  PrivacyConfig,
  ReleaseReadiness,
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
  const allSelectableColumnIndexes = useMemo(
    () => selectableColumns.map((column) => column.index),
    [selectableColumns],
  )
  const syntheticSelectionLocked = privacyConfig.releaseMode === 'syntheticData'
  const localAiSelected = !syntheticSelectionLocked && selectionUsesLocalAi(selectedColumns)
  const localAiReady = localAi.ready
  const localAiDownloadRunning = localAi.downloadRunning
  const selectedReleaseControls = useMemo(
    () =>
      syntheticSelectionLocked
        ? syntheticControlsForColumns(selectedColumns, columns, columnControls)
        : selectedControls,
    [columnControls, columns, selectedColumns, selectedControls, syntheticSelectionLocked],
  )

  const hasFile = Boolean(inputPath.trim())
  const isLoading = busy !== 'idle'
  const settingsDisabled = isLoading || !settingsLoaded
  const localAiBlocked = localAiSelected && (!localAiReady || localAiDownloadRunning)
  const privacyValidation = useMemo(() => {
    const baseValidation = getPrivacyConfigValidation(privacyConfig, selectedSet, columns.length)
    if (privacyConfig.releaseMode === 'differentialPrivacyAggregate' && settings.deterministicDefault) {
      return {
        valid: false,
        reason: 'Turn off Repeatable replacements before creating DP aggregate output.',
      }
    }
    return baseValidation
  }, [columns.length, privacyConfig, selectedSet, settings.deterministicDefault])
  const privacyConfigValid = privacyValidation.valid
  const privacyScaleWarning = getPrivacyScaleWarning(privacyConfig, headers)
  const releaseReadiness = useMemo(
    () =>
      buildReleaseReadiness({
        settingsLoaded,
        hasFile,
        hasColumns,
        hasSelectedColumns,
        outputPath,
        columns,
        selectedSet,
        settings,
        localAiBlocked,
        localAiSelected,
        localAiReady,
        privacyValidation,
        privacyScaleWarning,
      }),
    [
      settingsLoaded,
      hasFile,
      hasColumns,
      hasSelectedColumns,
      outputPath,
      columns,
      selectedSet,
      settings,
      localAiBlocked,
      localAiSelected,
      localAiReady,
      privacyValidation,
      privacyScaleWarning,
    ],
  )
  const previewWorkflow = usePreviewWorkflow({
    inputPath,
    selectedColumns,
    hasColumns,
    hasSelectedColumns,
    releaseMode: privacyConfig.releaseMode,
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
    selectedControls: selectedReleaseControls,
    hasColumns,
    hasSelectedColumns,
    headers,
    privacyConfig,
    privacyConfigValid,
    privacyValidation,
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

  function applySettingsBudget(next: AppSettings) {
    setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, next))
  }

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    if (!settingsLoaded) return

    const nextSettings = { ...latestSettingsRef.current, [key]: value }
    if (key === 'deterministicDefault' && value === false) {
      nextSettings.seed = ''
      nextSettings.rememberSeed = false
    }
    if (key === 'rememberSeed' && value === false) {
      nextSettings.seed = ''
    }
    if (
      key === 'deterministicDefault' ||
      key === 'rememberSeed' ||
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
    setCsvSelectedColumns(syntheticSelectionLocked ? allSelectableColumnIndexes : nextColumns)
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
    if (nextConfig.releaseMode === 'syntheticData') {
      setCsvSelectedColumns(allSelectableColumnIndexes)
    }
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
    if (syntheticSelectionLocked) {
      setCsvSelectedColumns(allSelectableColumnIndexes)
      clearArtifacts()
      return
    }
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
    syntheticSelectionLocked,
    hasFile,
    hasColumns,
    hasSelectedColumns,
    isLoading,
    settingsDisabled,
    canPreview: previewWorkflow.canPreview,
    canAnonymize: anonymizeJob.canAnonymize,
    privacyValidation,
    privacyScaleWarning,
    releaseReadiness,
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

function buildReleaseReadiness({
  settingsLoaded,
  hasFile,
  hasColumns,
  hasSelectedColumns,
  outputPath,
  columns,
  selectedSet,
  settings,
  localAiBlocked,
  localAiSelected,
  localAiReady,
  privacyValidation,
  privacyScaleWarning,
}: {
  settingsLoaded: boolean
  hasFile: boolean
  hasColumns: boolean
  hasSelectedColumns: boolean
  outputPath: string
  columns: ColumnMetadata[]
  selectedSet: Set<number>
  settings: AppSettings
  localAiBlocked: boolean
  localAiSelected: boolean
  localAiReady: boolean
  privacyValidation: { valid: boolean; reason: string | null }
  privacyScaleWarning: string | null
}): ReleaseReadiness {
  const blockers: string[] = []
  const reviewItems: string[] = []
  const verifiedItems: string[] = []

  if (!settingsLoaded) blockers.push('Settings are still loading.')
  else verifiedItems.push('Settings loaded.')

  if (!hasFile) blockers.push('Select an input file.')
  else verifiedItems.push('Input file selected.')

  if (!hasColumns) blockers.push('Analyze a file before creating output.')
  else verifiedItems.push(`${columns.length.toLocaleString()} columns analyzed.`)

  if (!hasSelectedColumns) blockers.push('Select at least one column to transform or release.')
  else verifiedItems.push(`${selectedSet.size.toLocaleString()} columns selected.`)

  if (!outputPath.trim()) blockers.push('Choose an output path.')
  else verifiedItems.push('Output path is set.')

  if (settings.deterministicDefault && !settings.seed.trim()) {
    blockers.push('Repeatable replacements need a non-empty private seed.')
  } else if (settings.deterministicDefault) {
    verifiedItems.push('Repeatable replacements have a private seed.')
    if (!settings.rememberSeed) {
      reviewItems.push('Seed is session-only; keep it available if this output must be reproduced later.')
    }
  } else {
    verifiedItems.push('Repeatable replacements are off.')
  }

  if (localAiBlocked) {
    blockers.push('Local AI is not ready for selected Smart replacement columns.')
  } else if (localAiSelected && localAiReady) {
    verifiedItems.push('Local AI is ready for Smart replacement columns.')
  } else {
    verifiedItems.push('No selected column requires Local AI.')
  }

  if (!privacyValidation.valid) {
    blockers.push(privacyValidation.reason ?? 'Complete the privacy release settings before creating output.')
  } else {
    verifiedItems.push('Privacy release settings are valid.')
  }

  if (privacyScaleWarning) reviewItems.push(privacyScaleWarning)

  const unselectedRiskColumns = columns.filter(
    (column) => (column.piiRisk === 'high' || column.piiRisk === 'medium') && !selectedSet.has(column.index),
  )
  if (unselectedRiskColumns.length > 0) {
    reviewItems.push(`Review ${formatColumnList(unselectedRiskColumns.map((column) => column.name))} before release.`)
  } else if (columns.length > 0) {
    verifiedItems.push('Detector-flagged risk columns are selected or explicitly absent.')
  }

  return {
    status: blockers.length > 0 ? 'blocked' : reviewItems.length > 0 ? 'review' : 'verified',
    blockers,
    reviewItems,
    verifiedItems,
  }
}

function formatColumnList(names: string[]) {
  if (names.length === 0) return 'detector-flagged columns'
  if (names.length <= 3) return names.join(', ')
  return `${names.slice(0, 3).join(', ')} and ${names.length - 3} more detector-flagged columns`
}

function syntheticControlsForColumns(
  columnIndexes: number[],
  columns: ColumnMetadata[],
  controls: Record<number, ColumnControl>,
): ColumnControl[] {
  return columnIndexes.flatMap((index) => {
    const column = columns.find((candidate) => candidate.index === index)
    const control = controls[index]
    const strategy = control?.strategy ?? column?.strategy ?? 'auto'
    const typeOverride = control?.typeOverride ?? null

    if (strategy === 'auto' && typeOverride === null) return []

    return [
      {
        columnIndex: index,
        typeOverride,
        strategy: 'auto',
      },
    ]
  })
}

export type AnonymizerWorkflowState = ReturnType<typeof useAnonymizerWorkflow>
