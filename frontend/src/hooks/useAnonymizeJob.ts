import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from 'react'
import {
  cancelAnonymizeJob,
  firstPreflightBlocker,
  getAnonymizeJobStatus,
  preflightAnonymization,
  startAnonymizeJob,
} from '../tauri'
import type {
  AnonymizeData,
  AnonymizeJobStatus,
  AppSettings,
  ColumnControl,
  HeadersData,
  LocalAiRequest,
  SmartReplacementEntry,
} from '../types'
import { messageFrom } from '../utils/errors'
import { directoryOf } from '../utils/paths'
import type { BusyState } from './workflowTypes'

type AnonymizeJobOptions = {
  inputPath: string
  outputPath: string
  selectedColumns: number[]
  selectedControls: ColumnControl[]
  hasColumns: boolean
  hasSelectedColumns: boolean
  headers: HeadersData | null
  settings: AppSettings
  previewSmartReplacements: SmartReplacementEntry[]
  localAiRequest: LocalAiRequest
  localAiBlocked: boolean
  busy: BusyState
  setBusy: Dispatch<SetStateAction<BusyState>>
  setError: Dispatch<SetStateAction<string | null>>
  setResult: Dispatch<SetStateAction<AnonymizeData | null>>
  persistSettings: (settings: AppSettings) => Promise<void>
  refreshSettings: () => Promise<void>
}

export function useAnonymizeJob({
  inputPath,
  outputPath,
  selectedColumns,
  selectedControls,
  hasColumns,
  hasSelectedColumns,
  headers,
  settings,
  previewSmartReplacements,
  localAiRequest,
  localAiBlocked,
  busy,
  setBusy,
  setError,
  setResult,
  persistSettings,
  refreshSettings,
}: AnonymizeJobOptions) {
  const [activeJobId, setActiveJobId] = useState<string | null>(null)
  const [jobStatus, setJobStatus] = useState<AnonymizeJobStatus | null>(null)
  const handleJobStatusRef = useRef(handleJobStatus)
  const consecutivePollFailuresRef = useRef(0)
  const canAnonymize = Boolean(
    hasColumns &&
      hasSelectedColumns &&
      inputPath &&
      outputPath &&
      busy === 'idle' &&
      !localAiBlocked,
  )

  useEffect(() => {
    handleJobStatusRef.current = handleJobStatus
  })

  useEffect(() => {
    if (busy !== 'running' || !activeJobId) return

    const jobId = activeJobId
    let isMounted = true
    let timeoutId: number | undefined
    consecutivePollFailuresRef.current = 0

    async function pollJob() {
      try {
        const status = await getAnonymizeJobStatus(jobId)
        if (!isMounted) return
        consecutivePollFailuresRef.current = 0
        const finished = handleJobStatusRef.current(status)
        if (!finished) {
          timeoutId = window.setTimeout(pollJob, 300)
        }
      } catch (caught) {
        if (!isMounted) return
        consecutivePollFailuresRef.current += 1
        if (consecutivePollFailuresRef.current < 2) {
          timeoutId = window.setTimeout(pollJob, 300)
          return
        }
        void cancelAnonymizeJob(jobId).catch(() => undefined)
        setActiveJobId(null)
        setJobStatus(null)
        setBusy('idle')
        setError(messageFrom(caught))
      }
    }

    timeoutId = window.setTimeout(pollJob, 300)

    return () => {
      isMounted = false
      if (timeoutId) window.clearTimeout(timeoutId)
    }
  }, [activeJobId, busy, setBusy, setError])

  function handleJobStatus(status: AnonymizeJobStatus) {
    setJobStatus(status)

    if (status.state === 'running') {
      return false
    }

    setActiveJobId(null)
    setBusy('idle')

    if (status.state === 'succeeded' && status.result) {
      setResult(status.result)
      setJobStatus(null)
      const nextSettings = settingsAfterSuccessfulRun(settings, status.result)
      if (nextSettings !== settings) {
        void persistSettings(nextSettings)
      } else {
        void refreshSettings()
      }
      return true
    }

    setJobStatus(null)
    if (status.state === 'canceled') {
      setError('Output creation canceled.')
    } else {
      setError(status.error ? messageFrom(status.error) : 'Output creation failed.')
    }
    return true
  }

  function anonymizeBlockedMessage() {
    if (localAiBlocked) return 'Set up Local AI before creating output with Smart replacement columns.'
    if (!inputPath || !hasColumns) return 'Load a CSV file first.'
    if (!hasSelectedColumns) return 'Select at least one column to anonymize.'
    if (!outputPath) return 'Choose an output path.'
    return 'Wait for the current operation to finish.'
  }

  async function runAnonymization() {
    if (!canAnonymize) {
      setError(anonymizeBlockedMessage())
      return
    }

    setBusy('running')
    setError(null)
    setResult(null)
    setJobStatus(null)

    try {
      const preflight = await preflightAnonymization(
        'anonymize',
        inputPath,
        outputPath,
        selectedColumns,
        selectedControls,
        settings.overwriteOutput,
        settings.sampleRowCount,
        previewSmartReplacements,
        localAiRequest,
      )
      const blocker = firstPreflightBlocker(preflight)
      if (blocker) {
        setBusy('idle')
        setError(blocker)
        return
      }
      const status = await startAnonymizeJob(
        inputPath,
        outputPath,
        selectedColumns,
        selectedControls,
        settings.overwriteOutput,
        settings.sampleRowCount,
        headers?.rowCountIsComplete ? headers.rowCount : null,
        previewSmartReplacements,
        localAiRequest,
      )
      setActiveJobId(status.jobId)
      handleJobStatus(status)
    } catch (caught) {
      setActiveJobId(null)
      setJobStatus(null)
      setBusy('idle')
      setError(messageFrom(caught))
    }
  }

  async function cancelCurrentJob() {
    if (!activeJobId || busy !== 'running') return

    try {
      const status = await cancelAnonymizeJob(activeJobId)
      handleJobStatus(status)
    } catch (caught) {
      setError(messageFrom(caught))
    }
  }

  function clearJobState() {
    setActiveJobId(null)
    setJobStatus(null)
  }

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
