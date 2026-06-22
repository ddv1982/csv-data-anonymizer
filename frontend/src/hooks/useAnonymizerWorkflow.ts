import { useEffect, useMemo, useState } from 'react'
import {
  applyBudgetSettingsToPrivacyConfig,
  defaultSettings,
  privacyConfigFromSettings,
  settingsWithPrivacyBudget,
} from '../defaults'
import { useLocalAi } from './useLocalAi'
import {
  cancelAnonymizeJob,
  analyzeCsv,
  countCsvRows,
  getAnonymizeJobStatus,
  loadSettings,
  pickInputCsv,
  pickOutputCsv,
  previewAnonymization,
  resetDpBudgetLedger,
  saveSettings,
  startAnonymizeJob,
} from '../tauri'
import type {
  AnalyzeResponse,
  AnonymizeData,
  AnonymizeJobStatus,
  AppSettings,
  ColumnControl,
  ColumnMetadata,
  ColumnRole,
  DataType,
  PrivacyConfig,
  PreviewData,
  AnonymizationStrategy,
} from '../types'
import { isSelectableColumn, maxVisibleColumns } from '../utils/columns'
import { messageFrom } from '../utils/errors'
import { directoryOf } from '../utils/paths'
import { getPrivacyConfigValidation } from '../utils/privacy'

type BusyState = 'idle' | 'picking' | 'loading' | 'preview' | 'running'

export function useAnonymizerWorkflow() {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings)
  const [inputPath, setInputPath] = useState('')
  const [outputPath, setOutputPath] = useState('')
  const [headers, setHeaders] = useState<AnalyzeResponse['headers'] | null>(null)
  const [selectedColumns, setSelectedColumns] = useState<number[]>([])
  const [columnControls, setColumnControls] = useState<Record<number, ColumnControl>>({})
  const [privacyConfig, setPrivacyConfig] = useState<PrivacyConfig>(() => privacyConfigFromSettings(defaultSettings))
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<AnonymizeData | null>(null)
  const [activeJobId, setActiveJobId] = useState<string | null>(null)
  const [jobStatus, setJobStatus] = useState<AnonymizeJobStatus | null>(null)
  const [busy, setBusy] = useState<BusyState>('idle')
  const [error, setError] = useState<string | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [showAllColumns, setShowAllColumns] = useState(false)
  const localAi = useLocalAi(settings, setError)

  useEffect(() => {
    let isMounted = true
    loadSettings()
      .then((loaded) => {
        if (isMounted) {
          setSettings(loaded)
          setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, loaded))
        }
      })
      .catch((caught: unknown) => {
        if (isMounted) setError(messageFrom(caught))
      })

    return () => {
      isMounted = false
    }
  }, [])

  useEffect(() => {
    setShowAllColumns(false)
  }, [headers?.columns.length])

  useEffect(() => {
    if (busy !== 'running' || !activeJobId) return

    const jobId = activeJobId
    let isMounted = true
    let timeoutId: number | undefined

    async function pollJob() {
      try {
        const status = await getAnonymizeJobStatus(jobId)
        if (!isMounted) return
        const finished = handleJobStatus(status)
        if (!finished) {
          timeoutId = window.setTimeout(pollJob, 300)
        }
      } catch (caught) {
        if (!isMounted) return
        setActiveJobId(null)
        setJobStatus(null)
        setBusy('idle')
        setError(messageFrom(caught))
      }
    }

    timeoutId = window.setTimeout(pollJob, 300)

    return () => {
      isMounted = false
      if (timeoutId) window.clearTimeout(timeoutId)
    }
  }, [activeJobId, busy])

  const columns = headers?.columns ?? []
  const selectedSet = useMemo(() => new Set(selectedColumns), [selectedColumns])
  const selectedControls = useMemo(
    () => selectedColumns.map((index) => columnControls[index]).filter(Boolean),
    [columnControls, selectedColumns],
  )
  const columnRoleControls = useMemo(
    () =>
      Object.fromEntries(privacyConfig.columnRoles.map((role) => [role.columnIndex, role])) as Record<
        number,
        PrivacyConfig['columnRoles'][number]
      >,
    [privacyConfig.columnRoles],
  )
  const selectableColumns = useMemo(() => columns.filter(isSelectableColumn), [columns])
  const highRiskColumns = useMemo(
    () => selectableColumns.filter((column) => column.piiRisk === 'high').map((column) => column.index),
    [selectableColumns],
  )
  const visibleColumns =
    showAllColumns || columns.length <= maxVisibleColumns ? columns : columns.slice(0, maxVisibleColumns)
  const hiddenColumnCount = Math.max(columns.length - maxVisibleColumns, 0)
  const allSelected =
    selectableColumns.length > 0 && selectableColumns.every((column) => selectedSet.has(column.index))
  const localAiSelected = useMemo(
    () =>
      selectedColumns.some((index) => {
        const column = columns.find((candidate) => candidate.index === index)
        return (columnControls[index]?.strategy ?? column?.strategy ?? 'auto') === 'localAi'
      }),
    [columnControls, columns, selectedColumns],
  )
  const localAiReady = localAi.ready
  const localAiDownloadRunning = localAi.downloadRunning

  const hasFile = Boolean(inputPath.trim())
  const hasColumns = Boolean(headers)
  const hasSelectedColumns = selectedColumns.length > 0
  const isLoading = busy !== 'idle'
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
  const canPreview = Boolean(hasColumns && hasSelectedColumns && inputPath && busy === 'idle' && !localAiBlocked)
  const canAnonymize = Boolean(
    hasColumns &&
      hasSelectedColumns &&
      inputPath &&
      outputPath &&
      busy === 'idle' &&
      !localAiBlocked &&
      privacyConfigValid,
  )

  async function persistSettings(next: AppSettings) {
    setSettings(next)
    try {
      const saved = await saveSettings(next)
      setSettings(saved)
      setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, saved))
    } catch (caught) {
      setError(messageFrom(caught))
    }
  }

  async function refreshSettings() {
    try {
      const loaded = await loadSettings()
      setSettings(loaded)
      setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, loaded))
    } catch (caught) {
      setError(messageFrom(caught))
    }
  }

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    const nextSettings = { ...settings, [key]: value }
    if (
      key === 'deterministicDefault' ||
      key === 'seed' ||
      key === 'previewSampleCount' ||
      key === 'localAiEnabled' ||
      key === 'localAiModel' ||
      isDpBudgetSetting(key)
    ) {
      setPreview(null)
      setResult(null)
    }
    if (isDpBudgetSetting(key)) {
      setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, nextSettings))
    }
    void persistSettings(nextSettings)
  }

  async function handlePickInput() {
    if (busy !== 'idle') return

    setError(null)
    setBusy('picking')
    try {
      const picked = await pickInputCsv(settings.rememberLastPaths ? settings.lastInputDirectory : null)
      if (picked) {
        await loadCsv(picked)
      }
    } catch (caught) {
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function loadCsv(path = inputPath) {
    const normalized = path.trim()
    if (!normalized) {
      setError('Select or enter a CSV file path first.')
      return
    }

    setBusy('loading')
    setError(null)
    setPreview(null)
    setResult(null)
    setColumnControls({})
    setPrivacyConfig(privacyConfigFromSettings(settings))

    try {
      const response = await analyzeCsv(normalized, settings.sampleRowCount, settings.defaultOutputSuffix)
      setInputPath(response.headers.filePath)
      setHeaders(response.headers)
      setSelectedColumns(response.selectedColumns)
      setOutputPath(response.suggestedOutputPath)

      if (settings.rememberLastPaths) {
        void persistSettings({
          ...settings,
          lastInputDirectory: directoryOf(response.headers.filePath),
          lastOutputDirectory: directoryOf(response.suggestedOutputPath),
        })
      }

      if (!response.headers.rowCountIsComplete) {
        void refreshExactRowCount(response.headers.filePath)
      }
    } catch (caught) {
      resetData()
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handlePickOutput() {
    if (!hasColumns || isLoading) return

    setError(null)
    setBusy('picking')
    try {
      const picked = await pickOutputCsv(
        outputPath || (settings.rememberLastPaths ? settings.lastOutputDirectory : null),
      )
      if (picked) {
        setOutputPath(picked)
        setResult(null)
        if (settings.rememberLastPaths) {
          void persistSettings({ ...settings, lastOutputDirectory: directoryOf(picked) })
        }
      }
    } catch (caught) {
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

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
      const nextPreview = await previewAnonymization(
        path,
        columnsToPreview,
        controlsForColumns(columnsToPreview),
        settings.deterministicDefault,
        settings.seed,
        settings.previewSampleCount,
        localAi.request,
      )
      setPreview(nextPreview)
      setResult(null)
    } catch (caught) {
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function refreshExactRowCount(path: string) {
    try {
      const rowCount = await countCsvRows(path)
      setHeaders((current) =>
        current?.filePath === path ? { ...current, rowCount, rowCountIsComplete: true } : current,
      )
    } catch {
      setHeaders((current) =>
        current?.filePath === path ? { ...current, rowCountIsComplete: false } : current,
      )
    }
  }

  function handleJobStatus(status: AnonymizeJobStatus) {
    setJobStatus(status)

    if (status.state === 'running') {
      return false
    }

    setActiveJobId(null)
    setBusy('idle')

    if (status.state === 'succeeded' && status.result) {
      setResult(status.result)
      setJobStatus(null)
      const nextSettings = settingsAfterSuccessfulRun(settings, status.result)
      if (nextSettings !== settings) {
        void persistSettings(nextSettings)
      } else {
        void refreshSettings()
      }
      return true
    }

    setJobStatus(null)
    if (status.state === 'canceled') {
      setError('Output creation canceled.')
    } else {
      setError(status.error ?? 'Output creation failed.')
    }
    return true
  }

  async function runAnonymization() {
    if (!canAnonymize) {
      setError(
        localAiBlocked
          ? 'Set up Local AI before creating output with Smart replacement columns.'
          : !privacyConfigValid
            ? (privacyValidation.reason ?? 'Complete the privacy release settings before running.')
            : 'Load a CSV, select at least one column, and choose an output path.',
      )
      return
    }

    setBusy('running')
    setError(null)
    setResult(null)
    setJobStatus(null)

    try {
      const status = await startAnonymizeJob(
        inputPath,
        outputPath,
        selectedColumns,
        selectedControls,
        settings.deterministicDefault,
        settings.seed,
        settings.overwriteOutput,
        settings.sampleRowCount,
        privacyConfig.releaseMode === 'differentialPrivacyAggregate'
          ? null
          : headers?.rowCountIsComplete
            ? headers.rowCount
            : null,
        preview?.smartReplacements ?? [],
        privacyConfig,
        localAi.request,
      )
      setActiveJobId(status.jobId)
      handleJobStatus(status)
    } catch (caught) {
      setActiveJobId(null)
      setJobStatus(null)
      setBusy('idle')
      setError(messageFrom(caught))
    }
  }

  async function cancelCurrentJob() {
    if (!activeJobId || busy !== 'running') return

    try {
      const status = await cancelAnonymizeJob(activeJobId)
      handleJobStatus(status)
    } catch (caught) {
      setError(messageFrom(caught))
    }
  }

  function setColumnSelection(nextColumns: number[]) {
    const uniqueSorted = [...new Set(nextColumns)].sort((left, right) => left - right)
    setSelectedColumns(uniqueSorted)
    setPreview(null)
    setResult(null)
  }

  function controlsForColumns(columnIndexes: number[]) {
    return columnIndexes.map((index) => columnControls[index]).filter(Boolean)
  }

  function selectionUsesLocalAi(columnIndexes: number[]) {
    return columnIndexes.some((index) => {
      const column = columns.find((candidate) => candidate.index === index)
      return (columnControls[index]?.strategy ?? column?.strategy ?? 'auto') === 'localAi'
    })
  }

  function defaultControl(column: ColumnMetadata): ColumnControl {
    return {
      columnIndex: column.index,
      typeOverride: null,
      strategy: column.strategy ?? 'auto',
    }
  }

  function updateColumnControl(
    column: ColumnMetadata,
    patch: Partial<Pick<ColumnControl, 'typeOverride' | 'strategy'>>,
  ) {
    setColumnControls((current) => ({
      ...current,
      [column.index]: { ...defaultControl(column), ...current[column.index], ...patch },
    }))
    setPreview(null)
    setResult(null)
  }

  function updateColumnType(column: ColumnMetadata, value: DataType | 'auto') {
    updateColumnControl(column, { typeOverride: value === 'auto' ? null : value })
  }

  function updateColumnStrategy(column: ColumnMetadata, strategy: AnonymizationStrategy) {
    updateColumnControl(column, { strategy })
  }

  function updatePrivacyConfig(nextConfig: PrivacyConfig) {
    setPrivacyConfig(nextConfig)
    setPreview(null)
    setResult(null)
    const nextSettings = settingsWithPrivacyBudget(settings, nextConfig)
    if (!sameDpBudgetSettings(settings, nextSettings)) {
      void persistSettings(nextSettings)
    }
  }

  async function resetDpBudget() {
    setError(null)
    try {
      const reset = await resetDpBudgetLedger()
      setSettings(reset)
      setPrivacyConfig((current) => applyBudgetSettingsToPrivacyConfig(current, reset))
      setPreview(null)
      setResult(null)
    } catch (caught) {
      setError(messageFrom(caught))
    }
  }

  function updateColumnRole(column: ColumnMetadata, role: ColumnRole) {
    const existing = privacyConfig.columnRoles.find((candidate) => candidate.columnIndex === column.index)
    const nextRole = {
      columnIndex: column.index,
      role,
      generalizationLevel: existing?.generalizationLevel ?? 0,
    }
    updatePrivacyConfig({
      ...privacyConfig,
      columnRoles:
        role === 'auto' && nextRole.generalizationLevel === 0
          ? privacyConfig.columnRoles.filter((candidate) => candidate.columnIndex !== column.index)
          : [
              ...privacyConfig.columnRoles.filter((candidate) => candidate.columnIndex !== column.index),
              nextRole,
            ].sort((left, right) => left.columnIndex - right.columnIndex),
    })
  }

  function toggleColumn(column: ColumnMetadata) {
    if (!isSelectableColumn(column)) return

    const next = selectedSet.has(column.index)
      ? selectedColumns.filter((index) => index !== column.index)
      : [...selectedColumns, column.index]

    setColumnSelection(next)
  }

  function resetData() {
    setHeaders(null)
    setSelectedColumns([])
    setColumnControls({})
    setPrivacyConfig(privacyConfigFromSettings(settings))
    setPreview(null)
    setResult(null)
    setActiveJobId(null)
    setJobStatus(null)
    setShowAllColumns(false)
  }

  function clearFile() {
    setInputPath('')
    setOutputPath('')
    resetData()
    setError(null)
  }

  function handleInputChange(value: string) {
    setInputPath(value)
    if (headers && value.trim() !== headers.filePath) {
      resetData()
    }
  }

  function maybeLoadManualPath() {
    const normalized = inputPath.trim()
    if (busy === 'idle' && normalized && normalized !== headers?.filePath) {
      void loadCsv(normalized)
    }
  }

  function updateOutputPath(value: string) {
    setOutputPath(value)
    setResult(null)
  }


  return {
    settings,
    inputPath,
    outputPath,
    headers,
    selectedColumns,
    columnControls,
    privacyConfig,
    preview,
    result,
    jobStatus,
    busy,
    error,
    settingsOpen,
    showAllColumns,
    localAi,
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
    canPreview,
    canAnonymize,
    privacyValidation,
    setError,
    setSettingsOpen,
    setShowAllColumns,
    updateSetting,
    updateOutputPath,
    handlePickInput,
    handlePickOutput,
    previewCsv,
    runAnonymization,
    cancelCurrentJob,
    setColumnSelection,
    updateColumnType,
    updateColumnStrategy,
    updatePrivacyConfig,
    resetDpBudget,
    updateColumnRole,
    toggleColumn,
    clearFile,
    handleInputChange,
    maybeLoadManualPath,
  }
}

function settingsAfterSuccessfulRun(settings: AppSettings, result: AnonymizeData): AppSettings {
  let nextSettings = settings
  if (settings.rememberLastPaths) {
    nextSettings = { ...nextSettings, lastOutputDirectory: directoryOf(result.outputPath) }
  }

  return nextSettings
}

function isDpBudgetSetting(key: keyof AppSettings) {
  return (
    key === 'dpBudgetEnabled' ||
    key === 'dpBudgetLimitEpsilon' ||
    key === 'dpBudgetAction'
  )
}

function sameDpBudgetSettings(left: AppSettings, right: AppSettings) {
  return (
    left.dpBudgetEnabled === right.dpBudgetEnabled &&
    left.dpBudgetLimitEpsilon === right.dpBudgetLimitEpsilon &&
    left.dpBudgetAction === right.dpBudgetAction
  )
}
