import {
  AlertCircle,
  AlertTriangle,
  FolderOpen,
  Loader2,
  Shield,
  X,
} from 'lucide-react'
import { Alert } from './components/Alert'
import { AppSettingsPanel } from './components/AppSettingsPanel'
import { Card } from './components/Card'
import { ColumnTable } from './components/ColumnTable'
import { LocalAiPanel } from './components/LocalAiPanel'
import { PreviewTable } from './components/PreviewTable'
import { PrivacySettingsPanel } from './components/PrivacySettingsPanel'
import { ProcessingStatus } from './components/ProcessingStatus'
import { ResultDisplay } from './components/ResultDisplay'
import { SectionHelp } from './components/SectionHelp'
import { ThemeModeToggle } from './components/ThemeModeToggle'
import { useAnonymizerWorkflow } from './hooks/useAnonymizerWorkflow'
import { normalizeThemeMode, useTheme } from './hooks/useTheme'
import { formatRowCount } from './utils/format'

function App() {
  const {
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
  } = useAnonymizerWorkflow()
  const themeMode = normalizeThemeMode(settings.themeMode)
  useTheme(themeMode)

  const unselectedRiskColumns = selectableColumns.filter(
    (column) => (column.piiRisk === 'high' || column.piiRisk === 'medium') && !selectedSet.has(column.index),
  )
  const unselectedRiskMessage =
    unselectedRiskColumns.length > 0
      ? formatUnselectedRiskMessage(
          unselectedRiskColumns.map((column) => column.name),
          privacyConfig.releaseMode,
        )
      : null

  return (
    <div className="app-root">
      <header className="app-topbar">
        <div className="container app-topbar-inner">
          <Shield className="brand-icon" aria-hidden="true" />
          <h1>CSV Anonymizer</h1>
          <ThemeModeToggle themeMode={themeMode} onChange={(mode) => updateSetting('themeMode', mode)} />
        </div>
      </header>

      {error ? (
        <div className="toast-region" aria-live="assertive" aria-atomic="true">
          <Alert variant="destructive" icon={<AlertCircle aria-hidden="true" />}>
            <div className="alert-line">
              <span>{error}</span>
              <button
                type="button"
                className="button button-ghost button-sm"
                aria-label="Dismiss error message"
                onClick={() => setError(null)}
              >
                Dismiss
              </button>
            </div>
          </Alert>
        </div>
      ) : null}

      <main className="container app-main">
        <div className="workflow-stack">
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

              <Card title="2. Select Data to Transform" disabled={!hasFile}>
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
                      Select High Detector Risk
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
                    columns={visibleColumns}
                    allColumnCount={columns.length}
                    selectedSet={selectedSet}
                    loading={isLoading}
                    showAllColumns={showAllColumns}
                    hiddenColumnCount={hiddenColumnCount}
                    onToggleColumn={toggleColumn}
                    controls={columnControls}
                    roles={columnRoleControls}
                    onTypeChange={updateColumnType}
                    onStrategyChange={updateColumnStrategy}
                    onRoleChange={updateColumnRole}
                    onToggleShowAll={() => setShowAllColumns((current) => !current)}
                  />

                  <p className="muted-text text-sm">
                    {selectedColumns.length} of {columns.length} columns selected
                    {headers ? `, ${formatRowCount(headers)} loaded` : ''}
                  </p>
                </div>
              </Card>

              <Card title="3. Configuration">
                <div className="config-stack">
                  <div className="field">
                    <label htmlFor="output-path">Output Path</label>
                    <div className="file-row">
                      <input
                        id="output-path"
                        type="text"
                        value={outputPath}
                        disabled={!hasColumns || isLoading}
                        placeholder="e.g., data_private_output.csv"
                        aria-describedby="output-path-description"
                        onChange={(event) => updateOutputPath(event.target.value)}
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
                      The path where the transformed or released file will be saved
                    </p>
                  </div>

                  <LocalAiPanel
                    enabled={settings.localAiEnabled}
                    model={settings.localAiModel}
                    status={localAi.status}
                    downloadStatus={localAi.downloadStatus}
                    disabled={!hasColumns || isLoading}
                    onToggle={(checked) => updateSetting('localAiEnabled', checked)}
                    onModelChange={(model) => updateSetting('localAiModel', model)}
                    onRefresh={() => void localAi.refresh()}
                    onDownload={() => void localAi.startDownload()}
                    onCancelDownload={() => void localAi.cancelDownload()}
                    onOpenSetup={() => void localAi.openSetup()}
                  />

                  <PrivacySettingsPanel
                    config={privacyConfig}
                    columns={columns}
                    disabled={!hasColumns || isLoading}
                    onResetBudget={() => void resetDpBudget()}
                    onChange={updatePrivacyConfig}
                  />
                  {hasColumns && !privacyValidation.valid ? (
                    <Alert variant="destructive" icon={<AlertCircle aria-hidden="true" />}>
                      {privacyValidation.reason ?? 'Complete the privacy release settings before creating output.'}
                    </Alert>
                  ) : null}

                  <AppSettingsPanel
                    settings={settings}
                    open={settingsOpen}
                    disabled={isLoading}
                    onToggleOpen={() => setSettingsOpen((current) => !current)}
                    onUpdateSetting={updateSetting}
                  />
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
                    fallbackRowCount={
                      privacyConfig.releaseMode === 'differentialPrivacyAggregate'
                        ? 0
                        : headers?.rowCountIsComplete
                          ? headers.rowCount
                          : 0
                    }
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
                    Create anonymized CSV
                  </button>
                )}
              </Card>
            </>
          )}
        </div>
      </main>

      <footer className="app-footer">
        <div className="container">
          <p>CSV Anonymizer - Transform sensitive fields in CSV files</p>
        </div>
      </footer>
    </div>
  )
}

function formatUnselectedRiskMessage(columnNames: string[], releaseMode: string) {
  const count = columnNames.length
  const shownNames = columnNames.slice(0, 3).join(', ')
  const extraCount = Math.max(count - 3, 0)
  const suffix = extraCount > 0 ? `, and ${extraCount} more` : ''
  const columnText = count === 1 ? 'column is' : 'columns are'
  const prefix = `${count} ${columnText} flagged high or medium risk by detection (${shownNames}${suffix}).`
  if (releaseMode === 'differentialPrivacyAggregate') {
    return `${prefix} DP aggregate output does not include row-level source rows, but configured group/value columns must still be selected.`
  }
  if (releaseMode === 'syntheticData') {
    return `${prefix} Synthetic data requires every CSV column to be selected so source columns are not copied through.`
  }
  return `${prefix} Unselected row-level columns are written unchanged.`
}

export default App
