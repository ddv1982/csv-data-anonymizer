import { AlertCircle, Check, Clipboard, Loader2, Wand2 } from 'lucide-react'
import { useMemo, useState } from 'react'
import { directInputStrategies } from '../dataOptions'
import { useColumnSelection } from '../hooks/useColumnSelection'
import { byteLength, formatByteLimit, MAX_PASTE_CONTENT_BYTES } from '../limits'
import { analyzePasteData, previewPasteData, transformPasteData } from '../tauri'
import { useCopyOutput } from '../hooks/useCopyOutput'
import type {
  AppSettings,
  PasteAnalyzeData,
  PasteDataFormat,
  PasteTransformData,
  PreviewData,
} from '../types'
import type { LocalAiState } from '../hooks/useLocalAi'
import { messageFrom } from '../utils/errors'
import { formatRowCount } from '../utils/format'
import { Alert } from './Alert'
import { Card } from './Card'
import { ColumnTable } from './ColumnTable'
import { PreviewTable } from './PreviewTable'
import { PrivacyReportSummary } from './ResultDisplay'

type PasteBusyState = 'idle' | 'analyzing' | 'previewing' | 'transforming' | 'copying'

const formatOptions: Array<{ value: PasteDataFormat; label: string }> = [
  { value: 'auto', label: 'Auto detect' },
  { value: 'csv', label: 'CSV text' },
  { value: 'json', label: 'JSON' },
]

export function PasteDataWorkflowView({
  settings,
  settingsLoaded,
  localAi,
  onOpenLocalAiSettings,
  onError,
}: {
  settings: AppSettings
  settingsLoaded: boolean
  localAi: LocalAiState
  onOpenLocalAiSettings: () => void
  onError: (message: string | null) => void
}) {
  const [format, setFormat] = useState<PasteDataFormat>('auto')
  const [content, setContent] = useState('')
  const [analysis, setAnalysis] = useState<PasteAnalyzeData | null>(null)
  const [preview, setPreview] = useState<PreviewData | null>(null)
  const [result, setResult] = useState<PasteTransformData | null>(null)
  const [busy, setBusy] = useState<PasteBusyState>('idle')
  const selection = useColumnSelection(analysis?.columns, { pruneDefaultControls: true })

  const isBusy = busy !== 'idle'
  const { copyOutput, copyStatus, setCopyStatus } = useCopyOutput({ isBusy, onError, setBusy })
  const contentByteLength = useMemo(() => byteLength(content), [content])
  const contentLimitLabel = formatByteLimit(MAX_PASTE_CONTENT_BYTES)
  const isContentTooLarge = contentByteLength > MAX_PASTE_CONTENT_BYTES
  const selectedUsesLocalAi = selection.selectionUsesLocalAi(selection.selectedColumns)
  const localAiBlocked = selectedUsesLocalAi && (!localAi.ready || localAi.downloadRunning)
  const canAnalyze = settingsLoaded && content.trim().length > 0 && !isBusy && !isContentTooLarge
  const canPreview = settingsLoaded && Boolean(analysis) && selection.selectedColumns.length > 0 && !isBusy && !localAiBlocked
  const canTransform = settingsLoaded && Boolean(analysis) && selection.selectedColumns.length > 0 && !isBusy && !localAiBlocked

  function resetDerivedState() {
    setAnalysis(null)
    selection.resetColumnSelection()
    setPreview(null)
    setResult(null)
    setCopyStatus(null)
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
      selection.setSelectedColumns(
        nextAnalysis.columns
          .filter((column) => column.piiRisk === 'high' || column.piiRisk === 'medium')
          .map((column) => column.index),
      )
      selection.resetColumnControls()
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handlePreview() {
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
      const nextPreview = await previewPasteData(
        content,
        analysis.format,
        selection.selectedColumns,
        selection.columnControlList,
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
    if (!settingsLoaded || !analysis || selection.selectedColumns.length === 0 || isBusy) return
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
        selection.selectedColumns,
        selection.columnControlList,
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

  function setColumnSelection(nextColumns: number[]) {
    selection.setSelectedColumns(nextColumns)
    clearPreviewAndResult()
  }

  function toggleColumn(column: Parameters<typeof selection.toggleColumn>[0]) {
    selection.toggleColumn(column)
    clearPreviewAndResult()
  }

  function updateColumnStrategy(column: Parameters<typeof selection.updateColumnStrategy>[0], strategy: Parameters<typeof selection.updateColumnStrategy>[1]) {
    selection.updateColumnStrategy(column, strategy)
    clearPreviewAndResult()
  }

  function clearPreviewAndResult() {
    setResult(null)
    setPreview(null)
  }

  return (
    <div className="workflow-stack">
      <Card
        title="1. Paste Sample"
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
              disabled={isBusy || selection.columns.length === 0 || selection.allSelected}
              onClick={() => setColumnSelection(selection.selectableColumns.map((column) => column.index))}
            >
              Select All
            </button>
            <button
              type="button"
              className="button button-outline button-sm"
              disabled={isBusy || selection.selectedColumns.length === 0}
              onClick={() => setColumnSelection([])}
            >
              Deselect All
            </button>
            <button
              type="button"
              className="button button-outline button-sm"
              disabled={isBusy || selection.detectedRiskColumns.length === 0}
              onClick={() => setColumnSelection(selection.detectedRiskColumns)}
            >
              Select Detected Risk
            </button>
          </div>

          <ColumnTable
            columns={selection.visibleColumns}
            allColumnCount={selection.columns.length}
            selectedSet={selection.selectedSet}
            loading={busy === 'analyzing'}
            showAllColumns={selection.showAllColumns}
            hiddenColumnCount={selection.hiddenColumnCount}
            onToggleColumn={toggleColumn}
            controls={selection.columnControls}
            onStrategyChange={updateColumnStrategy}
            onToggleShowAll={() => selection.setShowAllColumns((current) => !current)}
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
            {selection.selectedColumns.length} of {selection.columns.length} fields selected
            {analysis ? `, ${formatRowCount(analysis)} detected` : ''}
          </p>
        </div>
      </Card>

      <Card
        title="3. Preview (Optional)"
        disabled={!analysis || selection.selectedColumns.length === 0}
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
          Transform pasted sample
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

      {result ? <PrivacyReportSummary privacyReport={result.privacyReport} /> : null}

      {analysis && selection.columns.length === 0 ? (
        <Alert icon={<AlertCircle aria-hidden="true" />}>No fields detected for this input.</Alert>
      ) : null}
    </div>
  )
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
