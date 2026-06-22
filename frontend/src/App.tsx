import {
  AlertCircle,
  ChevronDown,
  FolderOpen,
  Loader2,
  Shield,
  X,
} from 'lucide-react'
import { Alert } from './components/Alert'
import { Card } from './components/Card'
import { ColumnTable } from './components/ColumnTable'
import { LocalAiPanel } from './components/LocalAiPanel'
import { PreviewTable } from './components/PreviewTable'
import { PrivacySettingsPanel } from './components/PrivacySettingsPanel'
import { ProcessingStatus } from './components/ProcessingStatus'
import { ResultDisplay } from './components/ResultDisplay'
import { SwitchRow } from './components/SwitchRow'
import { useAnonymizerWorkflow } from './hooks/useAnonymizerWorkflow'
import { formatRowCount } from './utils/format'
import { clampNumber } from './utils/numbers'

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
    updateColumnRole,
    toggleColumn,
    clearFile,
    handleInputChange,
    maybeLoadManualPath,
  } = useAnonymizerWorkflow()

  return (
    <div className="app-root">
      <header className="app-topbar">
        <div className="container app-topbar-inner">
          <Shield className="brand-icon" aria-hidden="true" />
          <h1>CSV Anonymizer</h1>
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
                      The path where the anonymized file will be saved
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
                    onChange={updatePrivacyConfig}
                  />

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
                          label="Repeatable replacements"
                          description="Use the same private seed to get the same replacements again."
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
                            placeholder="Enter a private seed"
                            aria-describedby="seed-description"
                            onChange={(event) => updateSetting('seed', event.target.value)}
                          />
                          <p id="seed-description" className="muted-text text-sm">
                            Useful when multiple files need matching replacements. Keep the seed private.
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
