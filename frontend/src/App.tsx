import {
  CheckCircle2,
  Eye,
  FileUp,
  FolderOpen,
  Loader2,
  Play,
  RefreshCw,
  RotateCcw,
  Save,
  ShieldCheck,
  XCircle,
} from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import {
  analyzeCsv,
  anonymizeCsv,
  loadSettings,
  openOutputLocation,
  pickInputCsv,
  pickOutputCsv,
  previewAnonymization,
  saveSettings,
} from './tauri'
import type { AnalyzeResponse, AnonymizeData, AppSettings, ColumnMetadata, PreviewData } from './types'

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

type BusyState = 'idle' | 'loading' | 'preview' | 'running'

function App() {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings)
  const [inputPath, setInputPath] = useState('')
  const [outputPath, setOutputPath] = useState('')
  const [headers, setHeaders] = useState<AnalyzeResponse['headers'] | null>(null)
  const [selectedColumns, setSelectedColumns] = useState<number[]>([])
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<AnonymizeData | null>(null)
  const [busy, setBusy] = useState<BusyState>('idle')
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)

  useEffect(() => {
    let isMounted = true
    loadSettings()
      .then((loaded) => {
        if (!isMounted) return
        setSettings(loaded)
        if (loaded.lastInputDirectory) {
          setInputPath(loaded.lastInputDirectory)
        }
        if (loaded.lastOutputDirectory) {
          setOutputPath(loaded.lastOutputDirectory)
        }
      })
      .catch((caught: unknown) => {
        if (isMounted) {
          setError(messageFrom(caught))
        }
      })

    return () => {
      isMounted = false
    }
  }, [])

  const selectedColumnData = useMemo(() => {
    const selected = new Set(selectedColumns)
    return headers?.columns.filter((column) => selected.has(column.index)) ?? []
  }, [headers, selectedColumns])

  const canPreview = Boolean(headers && inputPath && selectedColumns.length > 0)
  const canRun = Boolean(canPreview && outputPath && busy !== 'running')

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
    setError(null)
    const picked = await pickInputCsv()
    if (picked) {
      await loadCsv(picked)
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
    setNotice(null)
    setPreview(null)
    setResult(null)

    try {
      const response = await analyzeCsv(
        normalized,
        settings.sampleRowCount,
        settings.defaultOutputSuffix,
      )
      setInputPath(response.headers.filePath)
      setHeaders(response.headers)
      setSelectedColumns(response.selectedColumns)
      setOutputPath(response.suggestedOutputPath)
      setNotice(
        response.selectedColumns.length > 0
          ? `Selected ${response.selectedColumns.length} likely sensitive columns.`
          : 'No columns were auto-selected. Select columns to anonymize.',
      )

      if (settings.rememberLastPaths) {
        void persistSettings({
          ...settings,
          lastInputDirectory: directoryOf(response.headers.filePath),
          lastOutputDirectory: directoryOf(response.suggestedOutputPath),
        })
      }

      if (response.selectedColumns.length > 0) {
        await previewCsv(response.headers.filePath, response.selectedColumns)
      }
    } catch (caught) {
      resetData()
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handlePickOutput() {
    setError(null)
    const picked = await pickOutputCsv(outputPath || null)
    if (picked) {
      setOutputPath(picked)
      setResult(null)
      if (settings.rememberLastPaths) {
        void persistSettings({ ...settings, lastOutputDirectory: directoryOf(picked) })
      }
    }
  }

  async function previewCsv(path = inputPath, columns = selectedColumns) {
    if (!path || columns.length === 0) {
      setPreview(null)
      return
    }

    setBusy('preview')
    setError(null)
    try {
      const nextPreview = await previewAnonymization(
        path,
        columns,
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

  async function runAnonymization() {
    if (!canRun) {
      setError('Load a CSV, select at least one column, and choose an output path.')
      return
    }

    setBusy('running')
    setError(null)
    setNotice(null)

    try {
      const nextResult = await anonymizeCsv(
        inputPath,
        outputPath,
        selectedColumns,
        settings.deterministicDefault,
        settings.seed,
        settings.overwriteOutput,
        settings.sampleRowCount,
      )
      setResult(nextResult)
      setNotice(`Wrote ${nextResult.rowCount.toLocaleString()} rows to ${nextResult.outputPath}.`)
      if (settings.rememberLastPaths) {
        void persistSettings({ ...settings, lastOutputDirectory: directoryOf(nextResult.outputPath) })
      }
    } catch (caught) {
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  function toggleColumn(column: ColumnMetadata) {
    const next = selectedColumns.includes(column.index)
      ? selectedColumns.filter((index) => index !== column.index)
      : [...selectedColumns, column.index].sort((left, right) => left - right)

    setSelectedColumns(next)
    setPreview(null)
    setResult(null)
  }

  function resetData() {
    setHeaders(null)
    setSelectedColumns([])
    setPreview(null)
    setResult(null)
    setNotice(null)
  }

  function clearFile() {
    setInputPath('')
    setOutputPath('')
    resetData()
    setError(null)
  }

  return (
    <main className="app-shell">
      <header className="app-header">
        <div className="brand-block">
          <img src="/icon.png" alt="" className="app-icon" />
          <div>
            <h1>CSV Anonymizer</h1>
            <p>{headers ? `${headers.rowCount.toLocaleString()} rows loaded` : 'Local CSV privacy workflow'}</p>
          </div>
        </div>
        <div className="header-actions">
          <Metric label="Columns" value={selectedColumns.length.toString()} />
          <Metric label="Mode" value={settings.deterministicDefault ? 'Deterministic' : 'Random'} />
          <button type="button" className="btn ghost" onClick={clearFile} title="Reset current file">
            <RotateCcw size={17} aria-hidden="true" />
            Reset
          </button>
        </div>
      </header>

      <section className="workflow-grid">
        <section className="panel file-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">Files</span>
              <h2>Input and output</h2>
            </div>
            {busy === 'loading' ? <SpinnerLabel label="Reading" /> : null}
          </div>

          <label className="field">
            <span>Input CSV</span>
            <div className="field-row">
              <input
                value={inputPath}
                onChange={(event) => setInputPath(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') {
                    void loadCsv()
                  }
                }}
                placeholder="Select or paste a CSV path"
              />
              <button type="button" className="btn icon" onClick={handlePickInput} title="Choose CSV file">
                <FileUp size={18} aria-hidden="true" />
                Open
              </button>
              <button type="button" className="btn primary" onClick={() => void loadCsv()}>
                <RefreshCw size={18} aria-hidden="true" />
                Load
              </button>
            </div>
          </label>

          <label className="field">
            <span>Output CSV</span>
            <div className="field-row">
              <input
                value={outputPath}
                onChange={(event) => {
                  setOutputPath(event.target.value)
                  setResult(null)
                }}
                placeholder="Choose where to write the anonymized CSV"
              />
              <button
                type="button"
                className="btn icon"
                onClick={handlePickOutput}
                disabled={!headers}
                title="Choose output file"
              >
                <Save size={18} aria-hidden="true" />
                Save As
              </button>
            </div>
          </label>
        </section>

        <section className="panel settings-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">Settings</span>
              <h2>Transform rules</h2>
            </div>
            <ShieldCheck size={22} aria-hidden="true" />
          </div>

          <div className="toggle-grid">
            <label className="switch-row">
              <input
                type="checkbox"
                checked={settings.deterministicDefault}
                onChange={(event) => updateSetting('deterministicDefault', event.target.checked)}
              />
              <span>Deterministic</span>
            </label>
            <label className="switch-row">
              <input
                type="checkbox"
                checked={settings.overwriteOutput}
                onChange={(event) => updateSetting('overwriteOutput', event.target.checked)}
              />
              <span>Overwrite output</span>
            </label>
            <label className="switch-row">
              <input
                type="checkbox"
                checked={settings.rememberLastPaths}
                onChange={(event) => updateSetting('rememberLastPaths', event.target.checked)}
              />
              <span>Remember paths</span>
            </label>
          </div>

          <div className="number-grid">
            <label className="field compact">
              <span>Seed</span>
              <input
                value={settings.seed}
                onChange={(event) => updateSetting('seed', event.target.value)}
                placeholder="Optional deterministic seed"
              />
            </label>
            <label className="field compact">
              <span>Output suffix</span>
              <input
                value={settings.defaultOutputSuffix}
                onChange={(event) => updateSetting('defaultOutputSuffix', event.target.value)}
              />
            </label>
            <label className="field compact">
              <span>Sample rows</span>
              <input
                type="number"
                min={1}
                max={10000}
                value={settings.sampleRowCount}
                onChange={(event) =>
                  updateSetting('sampleRowCount', clampNumber(event.target.valueAsNumber, 1, 10000))
                }
              />
            </label>
            <label className="field compact">
              <span>Preview rows</span>
              <input
                type="number"
                min={1}
                max={100}
                value={settings.previewSampleCount}
                onChange={(event) =>
                  updateSetting('previewSampleCount', clampNumber(event.target.valueAsNumber, 1, 100))
                }
              />
            </label>
          </div>
        </section>
      </section>

      <section className="workspace-grid">
        <section className="panel columns-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">Detected columns</span>
              <h2>{headers ? `${headers.columns.length} columns` : 'No file loaded'}</h2>
            </div>
            <button
              type="button"
              className="btn ghost"
              disabled={!canPreview || busy !== 'idle'}
              onClick={() => void previewCsv()}
            >
              <Eye size={17} aria-hidden="true" />
              Preview
            </button>
          </div>

          <div className="columns-table" role="table" aria-label="Detected columns">
            <div className="columns-row columns-head" role="row">
              <span>Select</span>
              <span>Column</span>
              <span>Type</span>
              <span>Risk</span>
              <span>Samples</span>
            </div>
            {headers?.columns.map((column) => (
              <button
                type="button"
                className={`columns-row column-button risk-${column.piiRisk}`}
                key={`${column.index}-${column.name}`}
                onClick={() => toggleColumn(column)}
              >
                <span className="check-cell">
                  <input type="checkbox" checked={selectedColumns.includes(column.index)} readOnly />
                </span>
                <span>
                  <strong>{column.name}</strong>
                  <small>#{column.index}</small>
                </span>
                <span>{formatToken(column.detectedType)}</span>
                <span>
                  <RiskBadge risk={column.piiRisk} />
                </span>
                <span className="sample-list">{column.sampleValues.slice(0, 3).join(', ') || 'No samples'}</span>
              </button>
            ))}
            {!headers ? <div className="empty-state">Load a CSV to inspect columns.</div> : null}
          </div>
        </section>

        <section className="panel preview-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">Preview</span>
              <h2>{selectedColumnData.length} selected</h2>
            </div>
            {busy === 'preview' ? <SpinnerLabel label="Previewing" /> : null}
          </div>

          <div className="preview-list">
            {preview?.previews.map((columnPreview) => (
              <article className="preview-column" key={columnPreview.columnIndex}>
                <h3>{columnPreview.columnName}</h3>
                {columnPreview.samples.length > 0 ? (
                  columnPreview.samples.map((sample, index) => (
                    <div className="preview-sample" key={`${columnPreview.columnIndex}-${index}`}>
                      <code>{sample.original}</code>
                      <span>to</span>
                      <code>{sample.anonymized}</code>
                    </div>
                  ))
                ) : (
                  <p className="muted">No non-empty sample values found.</p>
                )}
              </article>
            ))}
            {!preview ? <div className="empty-state">Select columns and preview transformations.</div> : null}
          </div>
        </section>
      </section>

      <section className="run-strip">
        <div className="run-copy">
          <span className="eyebrow">Run</span>
          <strong>{result ? 'Anonymized output is ready' : 'Write anonymized CSV output'}</strong>
          <p>
            {result
              ? `${result.columnsAnonymized} columns in ${result.durationMs} ms`
              : 'Selected columns are transformed locally and written to the output path.'}
          </p>
        </div>
        <div className="run-actions">
          {result ? (
            <button
              type="button"
              className="btn icon"
              onClick={() => void openOutputLocation(result.outputPath)}
              title="Open output folder"
            >
              <FolderOpen size={18} aria-hidden="true" />
              Open Folder
            </button>
          ) : null}
          <button type="button" className="btn primary run-button" onClick={runAnonymization} disabled={!canRun}>
            {busy === 'running' ? <Loader2 className="spin" size={18} aria-hidden="true" /> : <Play size={18} aria-hidden="true" />}
            Anonymize
          </button>
        </div>
      </section>

      {notice ? (
        <div className="status-banner ok">
          <CheckCircle2 size={18} aria-hidden="true" />
          {notice}
        </div>
      ) : null}
      {error ? (
        <div className="status-banner error">
          <XCircle size={18} aria-hidden="true" />
          {error}
        </div>
      ) : null}
    </main>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  )
}

function RiskBadge({ risk }: { risk: string }) {
  return <span className={`risk-badge risk-${risk}`}>{formatToken(risk)}</span>
}

function SpinnerLabel({ label }: { label: string }) {
  return (
    <span className="spinner-label">
      <Loader2 className="spin" size={16} aria-hidden="true" />
      {label}
    </span>
  )
}

function clampNumber(value: number, min: number, max: number) {
  if (!Number.isFinite(value)) return min
  return Math.min(max, Math.max(min, Math.trunc(value)))
}

function directoryOf(path: string) {
  const slashIndex = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return slashIndex > 0 ? path.slice(0, slashIndex) : null
}

function formatToken(value: string) {
  return value
    .replace(/([A-Z])/g, ' $1')
    .replace(/^./, (first) => first.toUpperCase())
    .trim()
}

function messageFrom(value: unknown) {
  if (value instanceof Error) return value.message
  if (typeof value === 'string') return value
  return 'Unexpected application error.'
}

export default App
