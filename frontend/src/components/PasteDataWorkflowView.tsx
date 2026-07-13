import { AlertCircle, Check, Clipboard, Eraser, Loader2, Wand2 } from 'lucide-react'
import type { FocusEvent } from 'react'
import { directInputStrategies } from '../dataOptions'
import { usePasteDataWorkflow } from '../hooks/usePasteDataWorkflow'
import { formatByteLimit, MAX_PASTE_CONTENT_BYTES } from '../limits'
import type { AppSettings, PasteDataFormat, PasteTransformData } from '../types'
import type { LocalAiState } from '../hooks/useLocalAi'
import { formatRowCount } from '../utils/format'
import { Alert } from './Alert'
import { Card } from './Card'
import { ColumnSelectionPanel } from './ColumnSelectionPanel'
import { LocalAiBlockedAlert } from './LocalAiBlockedAlert'
import { PreviewTable } from './PreviewTable'
import { PrivacyReportSummary } from './PrivacyReportSummary'

const formatLabels: Record<PasteDataFormat, string> = {
  auto: 'Auto detect',
  csv: 'CSV text',
  json: 'JSON',
  xml: 'XML',
  yaml: 'YAML',
  plainText: 'Plain text',
  logs: 'Log lines',
}

const formatOptions: Array<{ value: PasteDataFormat; label: string }> = [
  { value: 'auto', label: formatLabels.auto },
  { value: 'csv', label: formatLabels.csv },
  { value: 'json', label: formatLabels.json },
  { value: 'xml', label: formatLabels.xml },
  { value: 'yaml', label: formatLabels.yaml },
  { value: 'plainText', label: formatLabels.plainText },
  { value: 'logs', label: formatLabels.logs },
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
  const workflow = usePasteDataWorkflow({ settings, settingsLoaded, localAi, onError })
  const { analysis, busy, content, format, preview, result, selection } = workflow
  const contentLimitLabel = formatByteLimit(MAX_PASTE_CONTENT_BYTES)

  async function handlePasteInputBlur(event: FocusEvent<HTMLTextAreaElement>) {
    if (analysis || isPasteActionTarget(event.relatedTarget)) return
    await workflow.analyze()
  }

  return (
    <div className="workflow-stack">
      <Card
        title="1. Paste Sample"
        action={
          <div className="bulk-actions">
            <button
              type="button"
              className="button button-outline button-sm"
              data-paste-action
              disabled={!workflow.canClear}
              onClick={workflow.clear}
            >
              <Eraser aria-hidden="true" />
              Clear
            </button>
            <button
              type="button"
              className="button button-outline button-sm"
              data-paste-action
              disabled={!workflow.canAnalyze}
              onClick={workflow.analyze}
            >
              {busy === 'analyzing' ? <Loader2 className="spin" aria-hidden="true" /> : null}
              Detect Fields
            </button>
          </div>
        }
      >
        <div className="direct-input-stack">
          <div className="direct-source-row">
            <div className="field">
              <label htmlFor="paste-format">Format</label>
              <select
                id="paste-format"
                value={format}
                disabled={workflow.isBusy}
                onChange={(event) => {
                  workflow.setFormat(event.target.value as PasteDataFormat)
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
            disabled={workflow.isBusy}
            placeholder='{"email":"ada@example.com","id":"123456"}'
            aria-label="Pasted data"
            onChange={(event) => {
              workflow.setContent(event.target.value)
            }}
            onBlur={handlePasteInputBlur}
          />
          <div className="direct-input-meta">
            <span className={`muted-text text-sm${workflow.isContentTooLarge ? ' danger-text' : ''}`}>
              {formatByteLimit(workflow.contentByteLength)} / {contentLimitLabel}
            </span>
          </div>
          {workflow.isContentTooLarge ? (
            <Alert icon={<AlertCircle aria-hidden="true" />}>
              Paste at most {contentLimitLabel} at a time, or use the CSV file workflow for larger inputs.
            </Alert>
          ) : null}
        </div>
      </Card>

      <Card title="2. Select Data to Transform" disabled={!analysis}>
        <ColumnSelectionPanel
          actions={[
            {
              label: 'Select All',
              disabled: workflow.isBusy || selection.columns.length === 0 || selection.allSelected,
              onClick: () => workflow.setColumnSelection(selection.columns.map((column) => column.index)),
            },
            {
              label: 'Deselect All',
              disabled: workflow.isBusy || selection.selectedColumns.length === 0,
              onClick: () => workflow.setColumnSelection([]),
            },
            {
              label: 'Select Detected Risk',
              disabled: workflow.isBusy || selection.detectedRiskColumns.length === 0,
              onClick: () => workflow.setColumnSelection(selection.detectedRiskColumns),
            },
          ]}
          columns={selection.visibleColumns}
          allColumnCount={selection.columns.length}
          selectedSet={selection.selectedSet}
          loading={busy === 'analyzing'}
          showAllColumns={selection.showAllColumns}
          hiddenColumnCount={selection.hiddenColumnCount}
          onToggleColumn={workflow.toggleColumn}
          controls={selection.columnControls}
          onStrategyChange={workflow.updateColumnStrategy}
          onToggleShowAll={() => selection.setShowAllColumns((current) => !current)}
          availableStrategies={directInputStrategies}
          footer={(
            <>
              {workflow.selectedUsesLocalAi && workflow.localAiBlocked ? (
                <LocalAiBlockedAlert
                  message="Set up Local AI before previewing or anonymizing Smart replacement fields."
                  onOpenSettings={onOpenLocalAiSettings}
                />
              ) : null}
              <p className="muted-text text-sm">
                {selection.selectedColumns.length} of {selection.columns.length} fields selected
                {analysis ? `, ${formatRowCount(analysis)} detected` : ''}
              </p>
            </>
          )}
        />
      </Card>

      <Card
        title="3. Preview (Optional)"
        disabled={!analysis || selection.selectedColumns.length === 0}
        action={
          <button type="button" className="button button-outline button-sm" disabled={!workflow.canPreview} onClick={workflow.showPreview}>
            {busy === 'previewing' ? <Loader2 className="spin" aria-hidden="true" /> : null}
            Show Preview
          </button>
        }
      >
        <PreviewTable preview={preview} loading={busy === 'previewing'} />
      </Card>

      <Card contentClassName="anonymize-card-content">
        <button type="button" className="button button-primary button-lg full-width" disabled={!workflow.canTransform} onClick={workflow.transform}>
          {busy === 'transforming' ? <Loader2 className="spin" aria-hidden="true" /> : <Wand2 aria-hidden="true" />}
          Transform pasted sample
        </button>
      </Card>

      {result ? (
        <Card
          title="Anonymized Output"
          action={
            <button type="button" className="button button-outline button-sm" disabled={workflow.isBusy} onClick={workflow.copyOutput}>
              {busy === 'copying' ? <Loader2 className="spin" aria-hidden="true" /> : <Clipboard aria-hidden="true" />}
              Copy
            </button>
          }
        >
          <div className="direct-output-stack">
            <textarea className="direct-output" value={result.output} readOnly aria-label="Anonymized pasted data" />
            <div className="direct-output-meta" aria-live="polite">
              <span className="muted-text text-sm">{formatPasteStats(result)}</span>
              {workflow.copyStatus ? (
                <span className="status-pill success">
                  <Check aria-hidden="true" />
                  {workflow.copyStatus}
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
  return formatLabels[format] ?? format
}

function isPasteActionTarget(target: EventTarget | null) {
  return target instanceof HTMLElement && Boolean(target.closest('[data-paste-action]'))
}
