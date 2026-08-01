import { AlertCircle, Eraser, Loader2, Wand2 } from 'lucide-react'
import { type FocusEvent } from 'react'
import type { PasteDataWorkflowState } from '../hooks/usePasteDataWorkflow'
import { useStableViewportAction } from '../hooks/useStableViewportAction'
import { formatByteLimit, MAX_PASTE_CONTENT_BYTES } from '../limits'
import type { DetectionCoverageSummary, PasteDataFormat } from '../types'
import { formatRowCount, formatTransformStats } from '../utils/format'
import { Alert } from './Alert'
import { Card } from './Card'
import { ColumnSelectionPanel } from './ColumnSelectionPanel'
import { ColumnTable } from './ColumnTable'
import { CopyableOutputCard } from './CopyableOutputCard'
import { DetectionRunNotice } from './DetectionRunNotice'
import { LocalAiBlockedAlert } from './LocalAiBlockedAlert'
import { PreviewTable } from './PreviewTable'
import { PrivacyReportSummary } from './PrivacyReportSummary'
import { SectionHelp } from './SectionHelp'

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
  workflow,
  onOpenLocalAiSettings,
}: {
  workflow: PasteDataWorkflowState
  onOpenLocalAiSettings: () => void
}) {
  const { analysis, busy, content, format, preview, result, selection } = workflow
  const runWithStableViewport = useStableViewportAction()
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
            {
              label: 'Select Uncertain',
              disabled: workflow.isBusy || selection.uncertainColumns.length === 0,
              onClick: () => workflow.setColumnSelection(selection.uncertainColumns),
            },
          ]}
          footer={(
            <>
              <DetectionRunNotice
                summary={analysis?.detectionRunSummary}
                columns={selection.columns}
              />
              {analysis?.detectionCoverage.isPartial ? (
                <Alert icon={<AlertCircle aria-hidden="true" />}>
                  {formatDetectionCoverageWarning(analysis.detectionCoverage)}
                </Alert>
              ) : null}
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
        >
          <ColumnTable
            columns={selection.visibleColumns}
            allColumnCount={selection.columns.length}
            selectedSet={selection.selectedSet}
            loading={busy === 'analyzing'}
            disabled={workflow.isBusy}
            showAllColumns={selection.showAllColumns}
            hiddenColumnCount={selection.hiddenColumnCount}
            onToggleColumn={workflow.toggleColumn}
            controls={selection.columnControls}
            onStrategyChange={workflow.updateColumnStrategy}
            onToggleShowAll={() => selection.setShowAllColumns((current) => !current)}
          />
        </ColumnSelectionPanel>
      </Card>

      <Card
        title="3. Preview (Optional)"
        disabled={!analysis || selection.selectedColumns.length === 0}
        action={
          <button
            type="button"
            className="button button-outline button-sm"
            disabled={!workflow.canRun && busy !== 'previewing'}
            aria-busy={busy === 'previewing'}
            aria-disabled={busy === 'previewing' || undefined}
            onClick={() => {
              if (busy !== 'idle') return
              void runWithStableViewport(workflow.showPreview)
            }}
          >
            {busy === 'previewing' ? <Loader2 className="spin" aria-hidden="true" /> : null}
            Show Preview
          </button>
        }
      >
        <div className="table-help-row">
          <SectionHelp topic="preview" label="What Preview does not prove" />
        </div>
        <PreviewTable preview={preview} loading={busy === 'previewing'} />
      </Card>

      <Card contentClassName="anonymize-card-content">
        <button type="button" className="button button-primary button-lg full-width" disabled={!workflow.canRun} onClick={workflow.transform}>
          {busy === 'transforming' ? <Loader2 className="spin" aria-hidden="true" /> : <Wand2 aria-hidden="true" />}
          Transform pasted sample
        </button>
      </Card>

      {result ? (
        <CopyableOutputCard
          title="Anonymized Output"
          outputLabel="Anonymized pasted data"
          output={result.output}
          stats={formatTransformStats(result, { unit: 'field' })}
          copying={busy === 'copying'}
          disabled={workflow.isBusy}
          copyStatus={workflow.copyStatus}
          onCopy={workflow.copyOutput}
        />
      ) : null}

      {result ? <PrivacyReportSummary privacyReport={result.privacyReport} /> : null}

      {analysis && selection.columns.length === 0 ? (
        <Alert icon={<AlertCircle aria-hidden="true" />}>No fields detected for this input.</Alert>
      ) : null}
    </div>
  )
}

// Sits beside the column table, before the transform button, because that is the
// last moment the disclosure is actionable. The privacy report carries the same
// caveat, but it is only written once the output exists, so acting on it there means
// re-running the whole paste. The noun comes from the backend: a JSON or free-text
// paste is sampled in field values, not rows, and hard-coding "rows" would state a
// figure the row count beside it contradicts.
function formatDetectionCoverageWarning(coverage: DetectionCoverageSummary) {
  const noun = coverage.unit === 'rows' ? 'rows' : 'values'
  return (
    `Field types were detected from ${coverage.examined.toLocaleString()} of ` +
    `${coverage.total.toLocaleString()} ${noun}. A value that appears in only a few ${noun} may have ` +
    `been missed, so a field can show a low risk and stay unselected on evidence that never saw it. ` +
    `Raise "Sample rows" in settings and detect again, or select such fields yourself.`
  )
}

function formatLabel(format: PasteDataFormat) {
  return formatLabels[format] ?? format
}

function isPasteActionTarget(target: EventTarget | null) {
  return target instanceof HTMLElement && Boolean(target.closest('[data-paste-action]'))
}
