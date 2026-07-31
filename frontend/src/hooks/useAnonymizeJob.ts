import { useEffect, useRef, useState } from 'react'
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
  PreparedAnalysis,
  SmartReplacementEntry,
} from '../types'
import { messageFrom } from '../utils/errors'
import { directoryOf } from '../utils/paths'
import type { WorkflowShell } from './workflowTypes'

/** Gap between successful status polls. */
const POLL_INTERVAL_MS = 300
/**
 * Failed polls tolerated silently before the user is told contact was lost.
 *
 * Above one so a single dropped response — common enough while the backend is
 * busy writing — never surfaces as an error the run then recovers from anyway.
 */
const POLL_FAILURES_BEFORE_REPORTING = 3
/** Ceiling on the backoff, so a long outage still notices recovery promptly. */
const MAX_POLL_RETRY_MS = 5_000
/**
 * How long contact may stay lost before the client stops polling and hands the UI back.
 *
 * Without a deadline, polling has no terminal condition at all: `snapshot_job` returns a
 * permanent error for an id that is no longer in the registry, and a poisoned registry
 * mutex fails forever too — neither can ever succeed on a retry. Retrying those every
 * five seconds keeps `busy === 'running'` for good, and that state has no exit: the view
 * gates every other control on it, and Cancel cannot rescue it either — pressing it only
 * sets a flag the worker reads, and the terminal `canceled` state that would release the
 * view can only be learned by the polling that is broken. Only killing the app recovers.
 *
 * Two minutes is measured against continuous unreachability, not against run length: a
 * healthy hour-long run answers every poll in microseconds (a status snapshot is an
 * in-memory lock and clone), so a long run never approaches this. Two minutes of silence
 * is roughly thirty consecutive failed attempts at the capped 5s backoff — far past any
 * plausible transient hiccup — while still being short enough that a wedged user is not
 * held hostage.
 *
 * Deliberately a wall clock and not a failure count: the backoff makes "how many failures"
 * a poor proxy for "how long the user has been stuck", which is the thing that matters.
 *
 * An unknown-job error is not treated as immediately terminal. The only signal Tauri hands
 * back is the Rust error prose ("Unknown anonymization job: …"); matching on it would stop
 * working silently the day someone rewords that string, leaving exactly the freeze this
 * constant exists to prevent. The deadline costs an unknown-job outage two minutes of
 * waiting and works no matter how the backend words its errors.
 */
const LOST_CONTACT_DEADLINE_MS = 120_000

/**
 * Says tracking stopped without claiming the run failed or finished.
 *
 * The backend job is unaffected by the client giving up on it — it may still be streaming
 * rows into the output file — so this must not read as a failure that invites a retry over
 * the same output path.
 */
function formatMinutes(milliseconds: number) {
  const minutes = milliseconds / 60_000
  return `${minutes} ${minutes === 1 ? 'minute' : 'minutes'}`
}

const LOST_CONTACT_GIVE_UP_MESSAGE =
  // The duration is read off the deadline rather than written out, because the two had
  // already been written out twice and would part company the first time the deadline moved:
  // the user would be told "two minutes" by a timer that waited five. Pluralised for the same
  // reason — a one-minute deadline would otherwise render "over 1 minutes".
  `Lost contact with the running job for over ${formatMinutes(LOST_CONTACT_DEADLINE_MS)}, so ` +
  'this app stopped tracking it. The job may still be running and writing to the output ' +
  'file: check that file before starting another run over the same path.'

/**
 * How long to wait before retrying after `failures` consecutive failed polls.
 *
 * Backs off rather than retrying flat at the poll interval, so a backend that is
 * unresponsive because it is overloaded is not asked 200 more times a minute while
 * it recovers. Capped, because the job may finish at any point and the client has
 * no other way to find out.
 */
function pollRetryDelay(failures: number) {
  return Math.min(POLL_INTERVAL_MS * 2 ** (failures - 1), MAX_POLL_RETRY_MS)
}

/**
 * Says contact was lost without claiming the run failed, and keeps the underlying
 * reason where it is useful.
 *
 * The distinction matters: the job may well still be streaming rows, and telling
 * someone their run failed when it has not is what would make them start a second
 * one over the same output.
 */
function lostContactMessage(caught: unknown) {
  return `Lost contact with the running job, still retrying. ${messageFrom(caught)}`
}

type AnonymizeJobArgs = {
  inputPath: string
  outputPath: string
  selectedColumns: number[]
  selectedControls: ColumnControl[]
  hasColumns: boolean
  hasSelectedColumns: boolean
  headers: HeadersData | null
  previewSmartReplacements: SmartReplacementEntry[]
  preparedAnalysis: PreparedAnalysis | null
  localAiBlocked: boolean
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
    localAiBlocked,
    persistSettings,
    refreshSettings,
  }: AnonymizeJobArgs,
) {
  const { busy, setBusy, setError, setResult, settings, localAi } = shell
  const localAiRequest = localAi.request

  const [activeJobId, setActiveJobId] = useState<string | null>(null)
  const [jobStatus, setJobStatus] = useState<AnonymizeJobStatus | null>(null)
  const handleJobStatusRef = useRef(handleJobStatus)
  const consecutivePollFailuresRef = useRef(0)
  /** When the current run of consecutive poll failures started, for the lost-contact deadline. */
  const lostContactSinceRef = useRef<number | null>(null)
  /**
   * The exact message this hook last wrote, so recovery clears only that message.
   *
   * Clearing on "polling failed recently" instead would erase whatever error is on screen,
   * including ones written after it by other code paths — a rejected Cancel, or a Local AI
   * download failure during a run — leaving the user believing an action succeeded.
   */
  const lostContactMessageRef = useRef<string | null>(null)
  const canAnonymize = Boolean(
    hasColumns &&
      hasSelectedColumns &&
      inputPath &&
      outputPath &&
      busy === 'idle' &&
      (!settings.localNerEnabled || Boolean(preparedAnalysis)) &&
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
    lostContactSinceRef.current = null
    lostContactMessageRef.current = null

    function reportLostContact(message: string) {
      lostContactMessageRef.current = message
      setError(message)
    }

    function clearLostContactReport() {
      const reported = lostContactMessageRef.current
      if (!reported) return
      lostContactMessageRef.current = null
      // Only retract our own message: anything written after it belongs to another
      // code path and is still news to the user.
      setError((current) => (current === reported ? null : current))
    }

    async function pollJob() {
      try {
        const status = await getAnonymizeJobStatus(jobId)
        if (!isMounted) return
        // Clear a lost-contact message the moment contact returns, so a recovered
        // run does not finish while still showing the reason it looked stuck.
        clearLostContactReport()
        consecutivePollFailuresRef.current = 0
        lostContactSinceRef.current = null
        const finished = handleJobStatusRef.current(status)
        if (!finished) {
          timeoutId = window.setTimeout(pollJob, POLL_INTERVAL_MS)
        }
      } catch (caught) {
        if (!isMounted) return
        // Losing contact with the job is not the job failing. Cancelling here used
        // to abort the run after two missed polls — under a second — discarding work
        // that can legitimately stream for an hour. The run is cancel-safe and leaves
        // no partial output either way, so there is nothing for the client to protect
        // by ending it; it only has to keep asking and say that it is out of touch.
        consecutivePollFailuresRef.current += 1
        lostContactSinceRef.current ??= Date.now()

        if (Date.now() - lostContactSinceRef.current >= LOST_CONTACT_DEADLINE_MS) {
          // Give the UI back rather than poll a job we can provably never hear from
          // again. Not rescheduling is the point: this is the only exit from `running`
          // when the failure is permanent.
          lostContactMessageRef.current = null
          setActiveJobId(null)
          setJobStatus(null)
          setBusy('idle')
          setError(LOST_CONTACT_GIVE_UP_MESSAGE)
          return
        }

        if (consecutivePollFailuresRef.current >= POLL_FAILURES_BEFORE_REPORTING) {
          reportLostContact(lostContactMessage(caught))
        }
        timeoutId = window.setTimeout(pollJob, pollRetryDelay(consecutivePollFailuresRef.current))
      }
    }

    timeoutId = window.setTimeout(pollJob, POLL_INTERVAL_MS)

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
    if (settings.localNerEnabled && !preparedAnalysis) return 'Analyze the source again before creating output.'
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
        ...(preparedAnalysis ? [preparedAnalysis] : []),
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
        ...(preparedAnalysis ? [preparedAnalysis] : []),
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
