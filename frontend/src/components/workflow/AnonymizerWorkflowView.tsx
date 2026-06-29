import {
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  FolderOpen,
  Info,
  Loader2,
  Shield,
  X,
} from 'lucide-react'
import type { ReactNode } from 'react'
import type { AnonymizerWorkflowState } from '../../hooks/useAnonymizerWorkflow'
import { formatRowCount } from '../../utils/format'
import { Alert } from '../Alert'
import { AppSettingsPanel } from '../AppSettingsPanel'
import { Card } from '../Card'
import { ColumnTable } from '../ColumnTable'
import { PreviewTable } from '../PreviewTable'
import { PrivacyReleaseModeSelector, PrivacySettingsPanel } from '../PrivacySettingsPanel'
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

export function AnonymizerWorkflowView({
  workflow,
  onOpenLocalAiSettings,
}: {
  workflow: AnonymizerWorkflowState
  onOpenLocalAiSettings: () => void
}) {
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
      <ConfigurationStep workflow={workflow} onOpenLocalAiSettings={onOpenLocalAiSettings} />
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
          disabled={workflow.settingsDisabled}
          aria-label="Browse for CSV file"
        >
          {workflow.busy === 'picking' ? <Loader2 className="spin" aria-hidden="true" /> : <FolderOpen aria-hidden="true" />}
          Browse
        </button>
        <input
          type="text"
          value={workflow.inputPath}
          disabled={workflow.settingsDisabled}
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
            disabled={workflow.settingsDisabled}
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
  const syntheticSelectionMessage =
    'Synthetic data creates a complete replacement dataset. Every CSV column is included; Type Override and Role control the generated values. Strategy is ignored.'
  const unselectedRiskColumns = workflow.syntheticSelectionLocked
    ? []
    : workflow.selectableColumns.filter(
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
    <Card
      title="2. Release Mode and Columns"
      disabled={!workflow.hasFile}
    >
      <div className="columns-stack">
        <PrivacyReleaseModeSelector
          config={workflow.privacyConfig}
          disabled={!workflow.hasColumns || workflow.isLoading}
          onChange={workflow.updatePrivacyConfig}
        />

        {workflow.syntheticSelectionLocked ? (
          <Alert icon={<Info aria-hidden="true" />}>{syntheticSelectionMessage}</Alert>
        ) : (
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
        )}

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
          selectionLocked={workflow.syntheticSelectionLocked}
          selectionLockedReason="Synthetic data includes every CSV column."
          strategyControlsDisabled={workflow.syntheticSelectionLocked}
          strategyControlsDisabledReason="Synthetic data is selected as a global release mode, not as a per-column strategy."
        />

        <p className="muted-text text-sm">
          {workflow.selectedColumns.length} of {workflow.columns.length} columns selected
          {workflow.headers ? `, ${formatRowCount(workflow.headers)} loaded` : ''}
        </p>
      </div>
    </Card>
  )
}

function ConfigurationStep({
  workflow,
  onOpenLocalAiSettings,
}: {
  workflow: AnonymizerWorkflowState
  onOpenLocalAiSettings: () => void
}) {
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

        {workflow.localAiBlocked ? (
          <Alert icon={<AlertCircle aria-hidden="true" />}>
            <div className="alert-line">
              <span>Set up Local AI before previewing or creating output with Smart replacement columns.</span>
              <button type="button" className="button button-outline button-sm" onClick={onOpenLocalAiSettings}>
                Open Local AI settings
              </button>
            </div>
          </Alert>
        ) : null}

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
          disabled={workflow.settingsDisabled}
          onToggleOpen={() => workflow.setSettingsOpen((current) => !current)}
          onUpdateSetting={workflow.updateSetting}
        />
      </div>
    </Card>
  )
}

function PreviewStep({ workflow }: { workflow: AnonymizerWorkflowState }) {
  const syntheticPreviewDisabled = workflow.privacyConfig.releaseMode === 'syntheticData'

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
      {syntheticPreviewDisabled ? (
        <Alert icon={<Info aria-hidden="true" />}>
          Preview is disabled for Synthetic data because the current preview shows row-level transformations, not the
          final generated dataset.
        </Alert>
      ) : (
        <PreviewTable preview={workflow.preview} loading={workflow.busy === 'preview'} />
      )}
    </Card>
  )
}

function RunStep({ workflow }: { workflow: AnonymizerWorkflowState }) {
  return (
    <Card contentClassName="anonymize-card-content">
      <ReleaseReadinessPanel readiness={workflow.releaseReadiness} />
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

function ReleaseReadinessPanel({ readiness }: { readiness: AnonymizerWorkflowState['releaseReadiness'] }) {
  const statusLabel =
    readiness.status === 'verified' ? 'Ready' : readiness.status === 'blocked' ? 'Blocked' : 'Review'
  const statusClass =
    readiness.status === 'verified'
      ? 'status-pill success'
      : readiness.status === 'blocked'
        ? 'status-pill blocked'
        : 'status-pill warning'

  return (
    <div className="release-readiness-panel">
      <div className="release-readiness-header">
        <span className="privacy-config-title">
          <Shield aria-hidden="true" />
          Release readiness
        </span>
        <span className={statusClass}>{statusLabel}</span>
      </div>
      {readiness.blockers.length > 0 ? (
        <ReadinessList title="Blocked" icon={<AlertCircle aria-hidden="true" />} items={readiness.blockers} />
      ) : null}
      {readiness.reviewItems.length > 0 ? (
        <ReadinessList title="Review" icon={<AlertTriangle aria-hidden="true" />} items={readiness.reviewItems} />
      ) : null}
      {readiness.verifiedItems.length > 0 ? (
        <ReadinessList
          title="Verified"
          icon={readiness.status === 'verified' ? <CheckCircle2 aria-hidden="true" /> : <Info aria-hidden="true" />}
          items={readiness.verifiedItems.slice(0, 5)}
        />
      ) : null}
    </div>
  )
}

function ReadinessList({ title, icon, items }: { title: string; icon: ReactNode; items: string[] }) {
  return (
    <div className="release-readiness-list">
      <span className="release-readiness-list-title">
        {icon}
        {title}
      </span>
      <ul>
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  )
}
