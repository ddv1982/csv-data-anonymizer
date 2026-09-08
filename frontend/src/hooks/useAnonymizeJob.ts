import {
  firstPreflightBlocker,
  preflightAnonymization,
  startAnonymizeJob,
} from '../tauri'
import type {
  AnonymizeData,
  AnonymizeJobStatus,
  AppSettings,
  ColumnControl,
  HeadersData,
  PreparedAnalysis,
  SmartReplacementEntry,
} from '../types'
import { messageFrom } from '../utils/errors'
import { confirmEphemeralTokenizationKey } from '../utils/tokenizationKey'
import { directoryOf } from '../utils/paths'
import { getProtectionReadiness } from './workflowReadiness'
import { useAnonymizeJobTracker } from './useAnonymizeJobTracker'
import type { WorkflowShell } from './workflowTypes'

type AnonymizeJobArgs = {
  inputPath: string
  outputPath: string
  selectedColumns: number[]
  selectedControls: ColumnControl[]
  hasColumns: boolean
  hasSelectedColumns: boolean
  headers: HeadersData | null
  preparedAnalysis: PreparedAnalysis | null
  previewSmartReplacements: SmartReplacementEntry[]
  tokenizationKey: string | null
  selectedUsesLocalAi: boolean
  selectedUsesTokenization: boolean
  persistSettings: (settings: AppSettings) => Promise<void>
  refreshSettings: () => Promise<void>
}

export function useAnonymizeJob(
  shell: WorkflowShell,
  {
    inputPath,
    outputPath,
    selectedColumns,
    selectedControls,
    hasColumns,
    hasSelectedColumns,
    headers,
    previewSmartReplacements,
    preparedAnalysis,
    tokenizationKey,
    selectedUsesTokenization,
    selectedUsesLocalAi,
    persistSettings,
    refreshSettings,
  }: AnonymizeJobArgs,
) {
  const { busy, setBusy, setError, setResult, settings, localAi } = shell
  const localAiRequest = localAi.request

  const trackerReadiness = getProtectionReadiness({
    usesLocalAi: selectedUsesLocalAi,
    localAi,
    requiresPreparedAnalysis: settings.localNerEnabled,
    hasPreparedAnalysis: Boolean(preparedAnalysis),
    usesTokenization: selectedUsesTokenization,
    tokenizationKey,
  })
  const tracker = useAnonymizeJobTracker({ busy, setBusy, setError, onTerminal: handleTerminal })
  const jobStatus = tracker.jobStatus

  function handleTerminal(status: AnonymizeJobStatus) {
    if (status.state === 'succeeded' && status.result) {
      setResult(status.result)
      const nextSettings = settingsAfterSuccessfulRun(settings, status.result)
      if (nextSettings !== settings) void persistSettings(nextSettings)
      else void refreshSettings()
      return
    }
    if (status.state === 'canceled') setError('Output creation canceled.')
    else setError(status.error ? messageFrom(status.error) : 'Output creation failed.')
  }

  const canAnonymize = Boolean(
    hasColumns && hasSelectedColumns && inputPath && outputPath && busy === 'idle' && !trackerReadiness.blocker,
  )

  async function runAnonymization() {
    if (!canAnonymize) {
      if (trackerReadiness.blocker === 'localAi') setError('Set up Local AI before creating output with Smart replacement columns.')
      else if (trackerReadiness.blocker === 'preparedAnalysis') setError('Analyze the source again before creating output.')
      else if (trackerReadiness.blocker === 'tokenizationKey') setError('Enter a valid 64-character hexadecimal tokenization key before creating output.')
      else if (!inputPath || !hasColumns) setError('Load a CSV file first.')
      else if (!hasSelectedColumns) setError('Select at least one column to anonymize.')
      else if (!outputPath) setError('Choose an output path.')
      else setError('Wait for the current operation to finish.')
      return
    }
    const activeTokenizationKey = trackerReadiness.tokenizationKey
    if (!confirmEphemeralTokenizationKey(activeTokenizationKey)) return
    setBusy('running')
    setError(null)
    setResult(null)
    tracker.clearJobState()

    try {
      const preflight = await preflightAnonymization({
        mode: 'anonymize',
        filePath: inputPath,
        outputPath,
        columns: selectedColumns,
        controls: selectedControls,
        force: settings.overwriteOutput,
        sampleRowCount: settings.sampleRowCount,
        previewSmartReplacements,
        localAi: localAiRequest,
        preparedAnalysis,
      })
      const blocker = firstPreflightBlocker(preflight)
      if (blocker) {
        setBusy('idle')
        setError(blocker)
        return
      }
      const status = await startAnonymizeJob({
        request: {
          filePath: inputPath,
          outputPath,
          columns: selectedColumns,
          controls: selectedControls,
          force: settings.overwriteOutput,
          sampleRowCount: settings.sampleRowCount,
          totalRowCount: headers?.rowCountIsComplete ? headers.rowCount : null,
          previewSmartReplacements,
          localAi: localAiRequest,
          preparedAnalysis,
          tokenizationKey: activeTokenizationKey,
        },
        onProgress: tracker.onProgress,
      })
      tracker.acceptStatus(status)
    } catch (caught) {
      tracker.clearJobState()
      setBusy('idle')
      setError(messageFrom(caught))
    }
  }

  const cancelCurrentJob = tracker.cancelCurrentJob
  const clearJobState = tracker.clearJobState

  return {
    jobStatus,
    canAnonymize,
    runAnonymization,
    cancelCurrentJob,
    clearJobState,
  }
}

function settingsAfterSuccessfulRun(settings: AppSettings, result: AnonymizeData): AppSettings {
  let nextSettings = settings
  if (settings.rememberLastPaths) {
    nextSettings = { ...nextSettings, lastOutputDirectory: directoryOf(result.outputPath) }
  }

  return nextSettings
}
