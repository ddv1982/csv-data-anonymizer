import {
  AlertCircle,
  AlertTriangle,
  FolderOpen,
  Loader2,
  Shield,
  X,
} from 'lucide-react'
import type { AnonymizerWorkflowState } from '../../hooks/useAnonymizerWorkflow'
import { formatRowCount } from '../../utils/format'
import { Alert } from '../Alert'
import { AppSettingsPanel } from '../AppSettingsPanel'
import { Card } from '../Card'
import { ColumnTable } from '../ColumnTable'
import { LocalAiSettingsBlock } from '../LocalAiSettingsBlock'
import { PreviewTable } from '../PreviewTable'
import { PrivacySettingsPanel } from '../PrivacySettingsPanel'
import { ProcessingStatus } from '../ProcessingStatus'
import { ResultDisplay } from '../ResultDisplay'
import { SectionHelp } from '../SectionHelp'
import { formatUnselectedRiskMessage } from './formatUnselectedRiskMessage'

export function WorkflowErrorToast({
  error,
  onDismiss,
}: {
  error: string | null
  onDismiss: () => void
}) {
  if (!error) return null

  return (
    <div className="toast-region" aria-live="assertive" aria-atomic="true">
      <Alert variant="destructive" icon={<AlertCircle aria-hidden="true" />}>
        <div className="alert-line">
          <span>{error}</span>
          <button
            type="button"
            className="button button-ghost button-sm"
            aria-label="Dismiss error message"
            onClick={onDismiss}
          >
            Dismiss
          </button>
        </div>
      </Alert>
    </div>
  )
}

export function AnonymizerWorkflowView({ workflow }: { workflow: AnonymizerWorkflowState }) {
  if (workflow.result) {
    return (
      <div className="workflow-stack">
        <ResultDisplay result={workflow.result} onReset={workflow.clearFile} onError={workflow.setError} />
      </div>
    )
  }

  return (
    <div className="workflow-stack">
      <FileStep workflow={workflow} />
      <ColumnSelectionStep workflow={workflow} />
      <ConfigurationStep workflow={workflow} />
      <PreviewStep workflow={workflow} />
      <RunStep workflow={workflow} />
    </div>
  )
}

function FileStep({ workflow }: { workflow: AnonymizerWorkflowState }) {
  return (
    <Card title="1. Select File">
      <div className="file-row">
        <button
          type="button"
          className="button button-outline"
          onClick={workflow.handlePickInput}
          disabled={workflow.isLoading}
          aria-label="Browse for CSV file"
        >
          {workflow.busy === 'picking' ? <Loader2 className="spin" aria-hidden="true" /> : <FolderOpen aria-hidden="true" />}
          Browse
        </button>
        <input
          type="text"
          value={workflow.inputPath}
          disabled={workflow.isLoading}
          placeholder="Select a CSV file..."
          aria-label="File path input"
          onChange={(event) => workflow.handleInputChange(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') workflow.maybeLoadManualPath()
          }}
        />
        {workflow.inputPath ? (
          <button
            type="button"
            className="button button-ghost button-icon"
            onClick={workflow.clearFile}
            disabled={workflow.isLoading}
            aria-label="Clear file selection"
          >
            <X aria-hidden="true" />
          </button>
        ) : null}
      </div>
    </Card>
  )
}

function ColumnSelectionStep({ workflow }: { workflow: AnonymizerWorkflowState }) {
  const unselectedRiskColumns = workflow.selectableColumns.filter(
    (column) =>
      (column.piiRisk === 'high' || column.piiRisk === 'medium') && !workflow.selectedSet.has(column.index),
  )
  const unselectedRiskMessage =
    unselectedRiskColumns.length > 0
      ? formatUnselectedRiskMessage(
          unselectedRiskColumns.map((column) => column.name),
          workflow.privacyConfig.releaseMode,
        )
      : null

  return (
    <Card title="2. Select Data to Transform" disabled={!workflow.hasFile}>
      <div className="columns-stack">
        <div className="bulk-actions">
          <button
            type="button"
            className="button button-outline button-sm"
            disabled={workflow.busy === 'loading' || workflow.allSelected || workflow.selectableColumns.length === 0}
            onClick={() => workflow.setColumnSelection(workflow.selectableColumns.map((column) => column.index))}
          >
            Select All
          </button>
          <button
            type="button"
            className="button button-outline button-sm"
            disabled={workflow.busy === 'loading' || workflow.selectedColumns.length === 0}
            onClick={() => workflow.setColumnSelection([])}
          >
            Deselect All
          </button>
          <button
            type="button"
            className="button button-outline button-sm"
            disabled={workflow.busy === 'loading' || workflow.highRiskColumns.length === 0}
            onClick={() => workflow.setColumnSelection(workflow.highRiskColumns)}
          >
            Select High Detector Risk
          </button>
          <button
            type="button"
            className="button button-outline button-sm"
            disabled={workflow.busy === 'loading' || workflow.selectableColumns.length === 0}
            onClick={() =>
              workflow.setColumnSelection(
                workflow.selectableColumns
                  .filter((column) => column.piiRisk === 'high' || column.piiRisk === 'medium')
                  .map((column) => column.index),
              )
            }
          >
            Select Detected Risk
          </button>
        </div>

        <div className="table-help-row">
          <SectionHelp topic="selectColumns" />
        </div>
        {unselectedRiskMessage ? (
          <Alert icon={<AlertTriangle aria-hidden="true" />}>
            <strong>Detector-flagged columns are unselected.</strong> {unselectedRiskMessage}
          </Alert>
        ) : null}

        <ColumnTable
          columns={workflow.visibleColumns}
          allColumnCount={workflow.columns.length}
          selectedSet={workflow.selectedSet}
          loading={workflow.isLoading}
          showAllColumns={workflow.showAllColumns}
          hiddenColumnCount={workflow.hiddenColumnCount}
          onToggleColumn={workflow.toggleColumn}
          controls={workflow.columnControls}
          roles={workflow.columnRoleControls}
          onTypeChange={workflow.updateColumnType}
          onStrategyChange={workflow.updateColumnStrategy}
          onRoleChange={workflow.updateColumnRole}
          onToggleShowAll={() => workflow.setShowAllColumns((current) => !current)}
        />

        <p className="muted-text text-sm">
          {workflow.selectedColumns.length} of {workflow.columns.length} columns selected
          {workflow.headers ? `, ${formatRowCount(workflow.headers)} loaded` : ''}
        </p>
      </div>
    </Card>
  )
}

function ConfigurationStep({ workflow }: { workflow: AnonymizerWorkflowState }) {
  return (
    <Card title="3. Configuration">
      <div className="config-stack">
        <div className="field">
          <label htmlFor="output-path">Output Path</label>
          <div className="file-row">
            <input
              id="output-path"
              type="text"
              value={workflow.outputPath}
              disabled={!workflow.hasColumns || workflow.isLoading}
              placeholder="e.g., data_private_output.csv"
              aria-describedby="output-path-description"
              onChange={(event) => workflow.updateOutputPath(event.target.value)}
            />
            <button
              type="button"
              className="button button-outline"
              disabled={!workflow.hasColumns || workflow.isLoading}
              onClick={workflow.handlePickOutput}
              aria-label="Choose output CSV file"
            >
              <FolderOpen aria-hidden="true" />
              Browse
            </button>
          </div>
          <p id="output-path-description" className="muted-text text-sm">
            The path where the transformed or released file will be saved
          </p>
        </div>

        <LocalAiSettingsBlock
          settings={workflow.settings}
          localAi={workflow.localAi}
          disabled={!workflow.hasColumns || workflow.isLoading}
          onUpdateSetting={workflow.updateSetting}
        />

        <PrivacySettingsPanel
          config={workflow.privacyConfig}
          columns={workflow.columns}
          disabled={!workflow.hasColumns || workflow.isLoading}
          onResetBudget={() => void workflow.resetDpBudget()}
          onChange={workflow.updatePrivacyConfig}
        />
        {workflow.hasColumns && !workflow.privacyValidation.valid ? (
          <Alert variant="destructive" icon={<AlertCircle aria-hidden="true" />}>
            {workflow.privacyValidation.reason ?? 'Complete the privacy release settings before creating output.'}
          </Alert>
        ) : null}
        {workflow.privacyScaleWarning ? (
          <Alert icon={<AlertTriangle aria-hidden="true" />}>{workflow.privacyScaleWarning}</Alert>
        ) : null}

        <AppSettingsPanel
          settings={workflow.settings}
          open={workflow.settingsOpen}
          disabled={workflow.isLoading}
          onToggleOpen={() => workflow.setSettingsOpen((current) => !current)}
          onUpdateSetting={workflow.updateSetting}
        />
      </div>
    </Card>
  )
}

function PreviewStep({ workflow }: { workflow: AnonymizerWorkflowState }) {
  return (
    <Card
      title="4. Preview (Optional)"
      disabled={!workflow.hasSelectedColumns}
      action={
        <button
          type="button"
          className="button button-outline button-sm"
          disabled={!workflow.canPreview}
          onClick={() => void workflow.previewCsv()}
        >
          {workflow.busy === 'preview' ? <Loader2 className="spin" aria-hidden="true" /> : null}
          Show Preview
        </button>
      }
    >
      <PreviewTable preview={workflow.preview} loading={workflow.busy === 'preview'} />
    </Card>
  )
}

function RunStep({ workflow }: { workflow: AnonymizerWorkflowState }) {
  return (
    <Card contentClassName="anonymize-card-content">
      {workflow.busy === 'running' ? (
        <ProcessingStatus
          status={workflow.jobStatus}
          fallbackRowCount={
            workflow.privacyConfig.releaseMode === 'differentialPrivacyAggregate'
              ? 0
              : workflow.headers?.rowCountIsComplete
                ? workflow.headers.rowCount
                : 0
          }
          onCancel={() => void workflow.cancelCurrentJob()}
        />
      ) : (
        <button
          type="button"
          className="button button-primary button-lg full-width"
          disabled={!workflow.canAnonymize}
          onClick={workflow.runAnonymization}
        >
          <Shield aria-hidden="true" />
          Create anonymized CSV
        </button>
      )}
    </Card>
  )
}
