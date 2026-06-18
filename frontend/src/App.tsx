import {
  AlertCircle,
  ChevronDown,
  FolderOpen,
  Loader2,
  Shield,
  X,
} from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { Alert } from './components/Alert'
import { Card } from './components/Card'
import { ColumnTable } from './components/ColumnTable'
import { PreviewTable } from './components/PreviewTable'
import { ProcessingStatus } from './components/ProcessingStatus'
import { ResultDisplay } from './components/ResultDisplay'
import { SwitchRow } from './components/SwitchRow'
import {
  cancelAnonymizeJob,
  analyzeCsv,
  countCsvRows,
  getAnonymizeJobStatus,
  loadSettings,
  pickInputCsv,
  pickOutputCsv,
  previewAnonymization,
  saveSettings,
  startAnonymizeJob,
} from './tauri'
import type {
  AnalyzeResponse,
  AnonymizeData,
  AnonymizeJobStatus,
  AppSettings,
  ColumnMetadata,
  PreviewData,
} from './types'
import { isSelectableColumn, maxVisibleColumns } from './utils/columns'
import { messageFrom } from './utils/errors'
import { formatRowCount } from './utils/format'
import { clampNumber } from './utils/numbers'
import { directoryOf } from './utils/paths'

const defaultSettings: AppSettings = {
  schemaVersion: 1,
  deterministicDefault: false,
  seed: '',
  overwriteOutput: false,
  sampleRowCount: 100,
  previewSampleCount: 5,
  defaultOutputSuffix: '_anonymized',
  rememberLastPaths: true,
  lastInputDirectory: null,
  lastOutputDirectory: null,
}

type BusyState = 'idle' | 'picking' | 'loading' | 'preview' | 'running'

function App() {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings)
  const [inputPath, setInputPath] = useState('')
  const [outputPath, setOutputPath] = useState('')
  const [headers, setHeaders] = useState<AnalyzeResponse['headers'] | null>(null)
  const [selectedColumns, setSelectedColumns] = useState<number[]>([])
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<AnonymizeData | null>(null)
  const [activeJobId, setActiveJobId] = useState<string | null>(null)
  const [jobStatus, setJobStatus] = useState<AnonymizeJobStatus | null>(null)
  const [busy, setBusy] = useState<BusyState>('idle')
  const [error, setError] = useState<string | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [showAllColumns, setShowAllColumns] = useState(false)

  useEffect(() => {
    let isMounted = true
    loadSettings()
      .then((loaded) => {
        if (isMounted) setSettings(loaded)
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

  const hasFile = Boolean(inputPath.trim())
  const hasColumns = Boolean(headers)
  const hasSelectedColumns = selectedColumns.length > 0
  const isLoading = busy !== 'idle'
  const canPreview = Boolean(hasColumns && hasSelectedColumns && inputPath && busy === 'idle')
  const canAnonymize = Boolean(hasColumns && hasSelectedColumns && inputPath && outputPath && busy === 'idle')

  async function persistSettings(next: AppSettings) {
    setSettings(next)
    try {
      await saveSettings(next)
    } catch (caught) {
      setError(messageFrom(caught))
    }
  }

  function updateSetting<K extends keyof AppSettings>(key: K, value: AppSettings[K]) {
    void persistSettings({ ...settings, [key]: value })
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

    setBusy('preview')
    setError(null)
    try {
      const nextPreview = await previewAnonymization(
        path,
        columnsToPreview,
        settings.deterministicDefault,
        settings.seed,
        settings.previewSampleCount,
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
      if (settings.rememberLastPaths) {
        void persistSettings({ ...settings, lastOutputDirectory: directoryOf(status.result.outputPath) })
      }
      return true
    }

    setJobStatus(null)
    if (status.state === 'canceled') {
      setError('Anonymization canceled.')
    } else {
      setError(status.error ?? 'Anonymization failed.')
    }
    return true
  }

  async function runAnonymization() {
    if (!canAnonymize) {
      setError('Load a CSV, select at least one column, and choose an output path.')
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
        settings.deterministicDefault,
        settings.seed,
        settings.overwriteOutput,
        settings.sampleRowCount,
        headers?.rowCountIsComplete ? headers.rowCount : null,
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

  return (
    <div className="app-root">
      <header className="app-topbar">
        <div className="container app-topbar-inner">
          <Shield className="brand-icon" aria-hidden="true" />
          <h1>CSV Anonymizer</h1>
        </div>
      </header>

      <main className="container app-main">
        <div className="workflow-stack">
          {error ? (
            <Alert variant="destructive" icon={<AlertCircle aria-hidden="true" />}>
              <div className="alert-line">
                <span>{error}</span>
                <button type="button" className="button button-ghost button-sm" onClick={() => setError(null)}>
                  Dismiss
                </button>
              </div>
            </Alert>
          ) : null}

          {result ? (
            <ResultDisplay result={result} onReset={clearFile} onError={setError} />
          ) : (
            <>
              <Card title="1. Select File">
                <div className="file-row">
                  <button
                    type="button"
                    className="button button-outline"
                    onClick={handlePickInput}
                    disabled={isLoading}
                    aria-label="Browse for CSV file"
                  >
                    {busy === 'picking' ? <Loader2 className="spin" aria-hidden="true" /> : <FolderOpen aria-hidden="true" />}
                    Browse
                  </button>
                  <input
                    type="text"
                    value={inputPath}
                    disabled={isLoading}
                    placeholder="Select a CSV file..."
                    aria-label="File path input"
                    onChange={(event) => handleInputChange(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === 'Enter') maybeLoadManualPath()
                    }}
                  />
                  {inputPath ? (
                    <button
                      type="button"
                      className="button button-ghost button-icon"
                      onClick={clearFile}
                      disabled={isLoading}
                      aria-label="Clear file selection"
                    >
                      <X aria-hidden="true" />
                    </button>
                  ) : null}
                </div>
              </Card>

              <Card title="2. Select Columns" disabled={!hasFile}>
                <div className="columns-stack">
                  <div className="bulk-actions">
                    <button
                      type="button"
                      className="button button-outline button-sm"
                      disabled={busy === 'loading' || allSelected || selectableColumns.length === 0}
                      onClick={() => setColumnSelection(selectableColumns.map((column) => column.index))}
                    >
                      Select All
                    </button>
                    <button
                      type="button"
                      className="button button-outline button-sm"
                      disabled={busy === 'loading' || selectedColumns.length === 0}
                      onClick={() => setColumnSelection([])}
                    >
                      Deselect All
                    </button>
                    <button
                      type="button"
                      className="button button-outline button-sm"
                      disabled={busy === 'loading' || highRiskColumns.length === 0}
                      onClick={() => setColumnSelection(highRiskColumns)}
                    >
                      Select High Risk
                    </button>
                    <button
                      type="button"
                      className="button button-outline button-sm"
                      disabled={busy === 'loading' || selectableColumns.length === 0}
                      onClick={() =>
                        setColumnSelection(
                          selectableColumns
                            .filter((column) => column.piiRisk === 'high' || column.piiRisk === 'medium')
                            .map((column) => column.index),
                        )
                      }
                    >
                      Select PII Risk
                    </button>
                  </div>

                  <ColumnTable
                    columns={visibleColumns}
                    allColumnCount={columns.length}
                    selectedSet={selectedSet}
                    loading={busy === 'loading'}
                    showAllColumns={showAllColumns}
                    hiddenColumnCount={hiddenColumnCount}
                    onToggleColumn={toggleColumn}
                    onToggleShowAll={() => setShowAllColumns((current) => !current)}
                  />

                  <p className="muted-text text-sm">
                    {selectedColumns.length} of {columns.length} columns selected
                    {headers ? `, ${formatRowCount(headers)} loaded` : ''}
                  </p>
                </div>
              </Card>

              <Card title="3. Configuration" disabled={!hasColumns}>
                <div className="config-stack">
                  <div className="field">
                    <label htmlFor="output-path">Output Path</label>
                    <div className="file-row">
                      <input
                        id="output-path"
                        type="text"
                        value={outputPath}
                        disabled={!hasColumns || isLoading}
                        placeholder="e.g., data_anonymized.csv"
                        aria-describedby="output-path-description"
                        onChange={(event) => {
                          setOutputPath(event.target.value)
                          setResult(null)
                        }}
                      />
                      <button
                        type="button"
                        className="button button-outline"
                        disabled={!hasColumns || isLoading}
                        onClick={handlePickOutput}
                        aria-label="Choose output CSV file"
                      >
                        <FolderOpen aria-hidden="true" />
                        Browse
                      </button>
                    </div>
                    <p id="output-path-description" className="muted-text text-sm">
                      The path where the anonymized file will be saved
                    </p>
                  </div>

                  <div className="collapsible">
                    <button
                      type="button"
                      className="button button-ghost settings-trigger"
                      disabled={!hasColumns || isLoading}
                      onClick={() => setSettingsOpen((current) => !current)}
                      aria-expanded={settingsOpen}
                    >
                      <span>App Settings</span>
                      <ChevronDown className={settingsOpen ? 'chevron open' : 'chevron'} aria-hidden="true" />
                    </button>
                    {settingsOpen ? (
                      <div className="settings-panel">
                        <SwitchRow
                          id="deterministic-mode"
                          label="Deterministic Mode"
                          description="The same input value produces the same anonymized output."
                          checked={settings.deterministicDefault}
                          disabled={!hasColumns || isLoading}
                          onChange={(checked) => updateSetting('deterministicDefault', checked)}
                        />
                        <div className={settings.deterministicDefault ? 'field' : 'field disabled-soft'}>
                          <label htmlFor="seed-input">Seed</label>
                          <input
                            id="seed-input"
                            type="text"
                            value={settings.seed}
                            disabled={!hasColumns || isLoading || !settings.deterministicDefault}
                            placeholder="Enter seed for reproducible results"
                            aria-describedby="seed-description"
                            onChange={(event) => updateSetting('seed', event.target.value)}
                          />
                          <p id="seed-description" className="muted-text text-sm">
                            Use the same seed to repeat anonymization across sessions.
                          </p>
                        </div>
                        <SwitchRow
                          id="overwrite-output"
                          label="Overwrite Output"
                          description="Replace the output file when it already exists."
                          checked={settings.overwriteOutput}
                          disabled={!hasColumns || isLoading}
                          onChange={(checked) => updateSetting('overwriteOutput', checked)}
                        />
                        <div className="settings-grid">
                          <div className="field">
                            <label htmlFor="output-suffix">Output suffix</label>
                            <input
                              id="output-suffix"
                              value={settings.defaultOutputSuffix}
                              disabled={!hasColumns || isLoading}
                              onChange={(event) => updateSetting('defaultOutputSuffix', event.target.value)}
                            />
                          </div>
                          <div className="field">
                            <label htmlFor="sample-rows">Sample rows</label>
                            <input
                              id="sample-rows"
                              type="number"
                              min={1}
                              max={10000}
                              value={settings.sampleRowCount}
                              disabled={!hasColumns || isLoading}
                              onChange={(event) =>
                                updateSetting('sampleRowCount', clampNumber(event.target.valueAsNumber, 1, 10000))
                              }
                            />
                          </div>
                          <div className="field">
                            <label htmlFor="preview-rows">Preview rows</label>
                            <input
                              id="preview-rows"
                              type="number"
                              min={1}
                              max={100}
                              value={settings.previewSampleCount}
                              disabled={!hasColumns || isLoading}
                              onChange={(event) =>
                                updateSetting('previewSampleCount', clampNumber(event.target.valueAsNumber, 1, 100))
                              }
                            />
                          </div>
                          <SwitchRow
                            id="remember-paths"
                            label="Remember paths"
                            checked={settings.rememberLastPaths}
                            disabled={!hasColumns || isLoading}
                            compact
                            onChange={(checked) => updateSetting('rememberLastPaths', checked)}
                          />
                        </div>
                      </div>
                    ) : null}
                  </div>
                </div>
              </Card>

              <Card
                title="4. Preview (Optional)"
                disabled={!hasSelectedColumns}
                action={
                  <button
                    type="button"
                    className="button button-outline button-sm"
                    disabled={!canPreview}
                    onClick={() => void previewCsv()}
                  >
                    {busy === 'preview' ? <Loader2 className="spin" aria-hidden="true" /> : null}
                    Show Preview
                  </button>
                }
              >
                <PreviewTable preview={preview} loading={busy === 'preview'} />
              </Card>

              <Card contentClassName="anonymize-card-content">
                {busy === 'running' ? (
                  <ProcessingStatus
                    status={jobStatus}
                    fallbackRowCount={headers?.rowCountIsComplete ? headers.rowCount : 0}
                    onCancel={() => void cancelCurrentJob()}
                  />
                ) : (
                  <button
                    type="button"
                    className="button button-primary button-lg full-width"
                    disabled={!canAnonymize}
                    onClick={runAnonymization}
                  >
                    <Shield aria-hidden="true" />
                    Anonymize File
                  </button>
                )}
              </Card>
            </>
          )}
        </div>
      </main>

      <footer className="app-footer">
        <div className="container">
          <p>CSV Anonymizer - Protect sensitive data in your CSV files</p>
        </div>
      </footer>
    </div>
  )
}

export default App
