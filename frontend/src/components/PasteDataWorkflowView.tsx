import { AlertCircle, Check, Clipboard, Loader2, Wand2 } from 'lucide-react'
import { useMemo, useState } from 'react'
import { directInputStrategies } from '../dataOptions'
import { byteLength, formatByteLimit, MAX_PASTE_CONTENT_BYTES } from '../limits'
import { analyzePasteData, previewPasteData, transformPasteData } from '../tauri'
import { useCopyOutput } from '../hooks/useCopyOutput'
import type {
  AnonymizationStrategy,
  AppSettings,
  ColumnControl,
  ColumnMetadata,
  DataType,
  PasteAnalyzeData,
  PasteDataFormat,
  PasteTransformData,
  PreviewData,
} from '../types'
import type { LocalAiState } from '../hooks/useLocalAi'
import { maxVisibleColumns } from '../utils/columns'
import { messageFrom } from '../utils/errors'
import { formatRowCount } from '../utils/format'
import { Alert } from './Alert'
import { Card } from './Card'
import { ColumnTable } from './ColumnTable'
import { PreviewTable } from './PreviewTable'

type PasteBusyState = 'idle' | 'analyzing' | 'previewing' | 'transforming' | 'copying'

const formatOptions: Array<{ value: PasteDataFormat; label: string }> = [
  { value: 'auto', label: 'Auto detect' },
  { value: 'json', label: 'JSON' },
  { value: 'xml', label: 'XML' },
  { value: 'yaml', label: 'YAML' },
  { value: 'csv', label: 'CSV text' },
  { value: 'plainText', label: 'Plain text' },
  { value: 'logs', label: 'Logs' },
]
const EMPTY_COLUMNS: ColumnMetadata[] = []

export function PasteDataWorkflowView({
  settings,
  localAi,
  onOpenLocalAiSettings,
  onError,
}: {
  settings: AppSettings
  localAi: LocalAiState
  onOpenLocalAiSettings: () => void
  onError: (message: string | null) => void
}) {
  const [format, setFormat] = useState<PasteDataFormat>('auto')
  const [content, setContent] = useState('')
  const [analysis, setAnalysis] = useState<PasteAnalyzeData | null>(null)
  const [selectedColumns, setSelectedColumns] = useState<number[]>([])
  const [controls, setControls] = useState<Record<number, ColumnControl>>({})
  const [showAllColumns, setShowAllColumns] = useState(false)
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<PasteTransformData | null>(null)
  const [busy, setBusy] = useState<PasteBusyState>('idle')

  const isBusy = busy !== 'idle'
  const { copyOutput, copyStatus, setCopyStatus } = useCopyOutput({ isBusy, onError, setBusy })
  const selectedSet = useMemo(() => new Set(selectedColumns), [selectedColumns])
  const columns = analysis?.columns ?? EMPTY_COLUMNS
  const visibleColumns = showAllColumns ? columns : columns.slice(0, maxVisibleColumns)
  const hiddenColumnCount = Math.max(0, columns.length - visibleColumns.length)
  const contentByteLength = useMemo(() => byteLength(content), [content])
  const contentLimitLabel = formatByteLimit(MAX_PASTE_CONTENT_BYTES)
  const isContentTooLarge = contentByteLength > MAX_PASTE_CONTENT_BYTES
  const controlList = useMemo(
    () => Object.values(controls).sort((left, right) => left.columnIndex - right.columnIndex),
    [controls],
  )
  const selectedUsesLocalAi = useMemo(
    () =>
      selectedColumns.some((index) => {
        const column = columns.find((candidate) => candidate.index === index)
        return (controls[index]?.strategy ?? column?.strategy ?? 'auto') === 'localAi'
      }),
    [columns, controls, selectedColumns],
  )
  const localAiBlocked = selectedUsesLocalAi && (!localAi.ready || localAi.downloadRunning)
  const canAnalyze = content.trim().length > 0 && !isBusy && !isContentTooLarge
  const canPreview = Boolean(analysis) && selectedColumns.length > 0 && !isBusy && !localAiBlocked
  const canTransform = Boolean(analysis) && selectedColumns.length > 0 && !isBusy && !localAiBlocked

  function resetDerivedState() {
    setAnalysis(null)
    setSelectedColumns([])
    setControls({})
    setPreview(null)
    setResult(null)
    setCopyStatus(null)
    setShowAllColumns(false)
  }

  async function handleAnalyze() {
    if (!canAnalyze) return
    onError(null)
    setBusy('analyzing')
    setCopyStatus(null)
    setPreview(null)
    setResult(null)
    try {
      const nextAnalysis = await analyzePasteData(content, format, settings.sampleRowCount)
      setAnalysis(nextAnalysis)
      setSelectedColumns(autoSelectedColumns(nextAnalysis.columns))
      setControls({})
      setShowAllColumns(false)
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handlePreview() {
    if (!analysis || selectedColumns.length === 0 || isBusy) return
    if (localAiBlocked) {
      onError('Set up Local AI before previewing Smart replacement fields.')
      return
    }
    onError(null)
    setBusy('previewing')
    setCopyStatus(null)
    setResult(null)
    try {
      const nextPreview = await previewPasteData(
        content,
        analysis.format,
        selectedColumns,
        controlList,
        settings.deterministicDefault,
        settings.seed,
        settings.previewSampleCount,
        localAi.request,
      )
      setPreview(nextPreview)
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handleTransform() {
    if (!analysis || selectedColumns.length === 0 || isBusy) return
    if (localAiBlocked) {
      onError('Set up Local AI before anonymizing Smart replacement fields.')
      return
    }
    onError(null)
    setBusy('transforming')
    setCopyStatus(null)
    try {
      const transformed = await transformPasteData(
        content,
        analysis.format,
        selectedColumns,
        controlList,
        settings.deterministicDefault,
        settings.seed,
        preview?.smartReplacements ?? [],
        localAi.request,
      )
      setResult(transformed)
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handleCopy() {
    await copyOutput(result?.output)
  }

  function toggleColumn(column: ColumnMetadata) {
    setSelectedColumns((current) =>
      current.includes(column.index)
        ? current.filter((index) => index !== column.index)
        : [...current, column.index].sort((left, right) => left - right),
    )
    setResult(null)
    setPreview(null)
  }

  function setColumnSelection(nextColumns: number[]) {
    setSelectedColumns([...new Set(nextColumns)].sort((left, right) => left - right))
    setResult(null)
    setPreview(null)
  }

  function updateColumnType(column: ColumnMetadata, value: DataType | 'auto') {
    updateColumnControl(column, { typeOverride: value === 'auto' ? null : value })
  }

  function updateColumnStrategy(column: ColumnMetadata, strategy: AnonymizationStrategy) {
    updateColumnControl(column, { strategy })
  }

  function updateColumnControl(column: ColumnMetadata, patch: Partial<ColumnControl>) {
    setControls((current) => {
      const existing = current[column.index] ?? {
        columnIndex: column.index,
        typeOverride: null,
        strategy: column.strategy ?? 'auto',
      }
      const next = { ...existing, ...patch }
      const nextControls = { ...current }
      if (next.typeOverride === null && next.strategy === (column.strategy ?? 'auto')) {
        delete nextControls[column.index]
      } else {
        nextControls[column.index] = next
      }
      return nextControls
    })
    setResult(null)
    setPreview(null)
  }

  return (
    <div className="workflow-stack">
      <Card
        title="1. Paste Data"
        action={
          <button type="button" className="button button-outline button-sm" disabled={!canAnalyze} onClick={handleAnalyze}>
            {busy === 'analyzing' ? <Loader2 className="spin" aria-hidden="true" /> : null}
            Detect Fields
          </button>
        }
      >
        <div className="direct-input-stack">
          <div className="direct-source-row">
            <div className="field">
              <label htmlFor="paste-format">Format</label>
              <select
                id="paste-format"
                value={format}
                disabled={isBusy}
                onChange={(event) => {
                  setFormat(event.target.value as PasteDataFormat)
                  resetDerivedState()
                }}
              >
                {formatOptions.map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            </div>
            {analysis ? (
              <span className="status-pill">
                Detected: {formatLabel(analysis.format)}
              </span>
            ) : null}
          </div>
          <textarea
            className="direct-textarea"
            value={content}
            disabled={isBusy}
            placeholder='{"email":"ada@example.com","id":"123456"}'
            aria-label="Pasted data"
            onChange={(event) => {
              setContent(event.target.value)
              resetDerivedState()
            }}
          />
          <div className="direct-input-meta">
            <span className={`muted-text text-sm${isContentTooLarge ? ' danger-text' : ''}`}>
              {formatByteLimit(contentByteLength)} / {contentLimitLabel}
            </span>
          </div>
          {isContentTooLarge ? (
            <Alert icon={<AlertCircle aria-hidden="true" />}>
              Paste at most {contentLimitLabel} at a time, or use the CSV file workflow for larger inputs.
            </Alert>
          ) : null}
        </div>
      </Card>

      <Card title="2. Select Data to Transform" disabled={!analysis}>
        <div className="columns-stack">
          <div className="bulk-actions">
            <button
              type="button"
              className="button button-outline button-sm"
              disabled={isBusy || columns.length === 0 || selectedColumns.length === columns.length}
              onClick={() => setColumnSelection(columns.map((column) => column.index))}
            >
              Select All
            </button>
            <button
              type="button"
              className="button button-outline button-sm"
              disabled={isBusy || selectedColumns.length === 0}
              onClick={() => setColumnSelection([])}
            >
              Deselect All
            </button>
            <button
              type="button"
              className="button button-outline button-sm"
              disabled={isBusy || columns.length === 0}
              onClick={() => setColumnSelection(autoSelectedColumns(columns))}
            >
              Select Detected Risk
            </button>
          </div>

          <ColumnTable
            columns={visibleColumns}
            allColumnCount={columns.length}
            selectedSet={selectedSet}
            loading={busy === 'analyzing'}
            showAllColumns={showAllColumns}
            hiddenColumnCount={hiddenColumnCount}
            onToggleColumn={toggleColumn}
            controls={controls}
            onTypeChange={updateColumnType}
            onStrategyChange={updateColumnStrategy}
            onToggleShowAll={() => setShowAllColumns((current) => !current)}
            showRoles={false}
            availableStrategies={directInputStrategies}
          />

          {selectedUsesLocalAi && localAiBlocked ? (
            <Alert icon={<AlertCircle aria-hidden="true" />}>
              <div className="alert-line">
                <span>Set up Local AI before previewing or anonymizing Smart replacement fields.</span>
                <button type="button" className="button button-outline button-sm" onClick={onOpenLocalAiSettings}>
                  Open Local AI settings
                </button>
              </div>
            </Alert>
          ) : null}

          <p className="muted-text text-sm">
            {selectedColumns.length} of {columns.length} fields selected
            {analysis ? `, ${formatRowCount(analysis)} detected` : ''}
          </p>
        </div>
      </Card>

      <Card
        title="3. Preview (Optional)"
        disabled={!analysis || selectedColumns.length === 0}
        action={
          <button type="button" className="button button-outline button-sm" disabled={!canPreview} onClick={handlePreview}>
            {busy === 'previewing' ? <Loader2 className="spin" aria-hidden="true" /> : null}
            Show Preview
          </button>
        }
      >
        <PreviewTable preview={preview} loading={busy === 'previewing'} />
      </Card>

      <Card contentClassName="anonymize-card-content">
        <button type="button" className="button button-primary button-lg full-width" disabled={!canTransform} onClick={handleTransform}>
          {busy === 'transforming' ? <Loader2 className="spin" aria-hidden="true" /> : <Wand2 aria-hidden="true" />}
          Anonymize pasted data
        </button>
      </Card>

      {result ? (
        <Card
          title="Anonymized Output"
          action={
            <button type="button" className="button button-outline button-sm" disabled={isBusy} onClick={handleCopy}>
              {busy === 'copying' ? <Loader2 className="spin" aria-hidden="true" /> : <Clipboard aria-hidden="true" />}
              Copy
            </button>
          }
        >
          <div className="direct-output-stack">
            <textarea className="direct-output" value={result.output} readOnly aria-label="Anonymized pasted data" />
            <div className="direct-output-meta" aria-live="polite">
              <span className="muted-text text-sm">{formatPasteStats(result)}</span>
              {copyStatus ? (
                <span className="status-pill success">
                  <Check aria-hidden="true" />
                  {copyStatus}
                </span>
              ) : null}
            </div>
          </div>
        </Card>
      ) : null}

      {analysis && columns.length === 0 ? (
        <Alert icon={<AlertCircle aria-hidden="true" />}>No fields detected for this input.</Alert>
      ) : null}
    </div>
  )
}

function autoSelectedColumns(columns: ColumnMetadata[]) {
  return columns
    .filter((column) => column.piiRisk === 'high' || column.piiRisk === 'medium')
    .map((column) => column.index)
}

function formatPasteStats(result: PasteTransformData) {
  const rows = result.rowCount.toLocaleString()
  const columns = result.columnsAnonymized === 1 ? 'field' : 'fields'
  const duration = result.durationMs < 1000 ? `${result.durationMs}ms` : `${(result.durationMs / 1000).toFixed(2)}s`
  return `${rows} rows processed, ${result.columnsAnonymized} ${columns} transformed in ${duration}`
}

function formatLabel(format: PasteDataFormat) {
  return formatOptions.find((option) => option.value === format)?.label ?? format
}
