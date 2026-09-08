import { useEffect, useReducer, useRef } from 'react'
import { cancelAnonymizeJob, getAnonymizeJobStatus } from '../tauri'
import type { AnonymizeJobStatus } from '../types'
import { messageFrom } from '../utils/errors'
import type { WorkflowShell } from './workflowTypes'

const POLL_INTERVAL_MS = 300
const CHANNEL_SILENCE_BEFORE_POLL_MS = 5_000
const POLL_FAILURES_BEFORE_REPORTING = 3
const MAX_POLL_RETRY_MS = 5_000
const LOST_CONTACT_DEADLINE_MS = 120_000
function formatMinutes(milliseconds: number) { const minutes = milliseconds / 60_000; return `${minutes} ${minutes === 1 ? 'minute' : 'minutes'}` }
const LOST_CONTACT_GIVE_UP_MESSAGE = `Lost contact with the running job for over ${formatMinutes(LOST_CONTACT_DEADLINE_MS)}, so this app stopped tracking it. The job may still be running and writing to the output file: check that file before starting another run over the same path.`
function pollRetryDelay(failures: number) { return Math.min(POLL_INTERVAL_MS * 2 ** (failures - 1), MAX_POLL_RETRY_MS) }
function lostContactMessage(caught: unknown) { return `Lost contact with the running job, still retrying. ${messageFrom(caught)}` }

type State = { tag: 'idle'; status: null } | { tag: 'running'; jobId: string; status: AnonymizeJobStatus }
type Event = { type: 'started' | 'progress'; status: AnonymizeJobStatus } | { type: 'cleared' }
const idle: State = { tag: 'idle', status: null }
function reducer(state: State, event: Event): State {
  if (event.type === 'cleared') return idle
  if (event.type === 'started') return event.status.state === 'running' ? { tag: 'running', jobId: event.status.jobId, status: event.status } : idle
  if (state.tag !== 'running' || event.status.jobId !== state.jobId) return state
  return event.status.state === 'running' ? { ...state, status: event.status } : idle
}

type AnonymizeJobTrackerArgs = Pick<WorkflowShell, 'busy' | 'setBusy' | 'setError'> & { onTerminal: (status: AnonymizeJobStatus) => void }
export function useAnonymizeJobTracker({ busy, setBusy, setError, onTerminal }: AnonymizeJobTrackerArgs) {
  const [state, dispatch] = useReducer(reducer, idle)
  const activeJobId = state.tag === 'running' ? state.jobId : null
  const onTerminalRef = useRef(onTerminal)
  const handlerRef = useRef<(status: AnonymizeJobStatus) => boolean>(() => false)
  const terminalIds = useRef(new Set<string>())
  const lastChannelUpdate = useRef<number | null>(null)
  const failures = useRef(0)
  const lostSince = useRef<number | null>(null)
  const lostMessage = useRef<string | null>(null)
  useEffect(() => { onTerminalRef.current = onTerminal })
  useEffect(() => { handlerRef.current = acceptStatus })

  function acceptStatus(status: AnonymizeJobStatus) {
    if (status.state === 'running') {
      if (terminalIds.current.has(status.jobId)) return true
      dispatch({ type: activeJobId ? 'progress' : 'started', status }); return false
    }
    terminalIds.current.add(status.jobId)
    dispatch({ type: 'progress', status }); setBusy('idle'); onTerminalRef.current(status); return true
  }
  function onProgress(status: AnonymizeJobStatus) { lastChannelUpdate.current = Date.now(); handlerRef.current(status) }

  useEffect(() => {
    if (busy !== 'running' || !activeJobId) return
    const jobId = activeJobId; let mounted = true; let timeout: number | undefined
    failures.current = 0; lostSince.current = null; lostMessage.current = null
    const report = (message: string) => { lostMessage.current = message; setError(message) }
    const clearReport = () => { const reported = lostMessage.current; if (!reported) return; lostMessage.current = null; setError(current => current === reported ? null : current) }
    async function poll() {
      const channel = lastChannelUpdate.current
      if (channel !== null && Date.now() - channel < CHANNEL_SILENCE_BEFORE_POLL_MS) { timeout = window.setTimeout(poll, CHANNEL_SILENCE_BEFORE_POLL_MS - (Date.now() - channel)); return }
      try {
        const status = await getAnonymizeJobStatus(jobId); if (!mounted) return
        clearReport(); failures.current = 0; lostSince.current = null
        if (!handlerRef.current(status)) timeout = window.setTimeout(poll, POLL_INTERVAL_MS)
      } catch (caught) {
        if (!mounted) return
        failures.current += 1; lostSince.current ??= Date.now()
        if (Date.now() - lostSince.current >= LOST_CONTACT_DEADLINE_MS) { lostMessage.current = null; dispatch({ type: 'cleared' }); setBusy('idle'); setError(LOST_CONTACT_GIVE_UP_MESSAGE); return }
        if (failures.current >= POLL_FAILURES_BEFORE_REPORTING) report(lostContactMessage(caught))
        timeout = window.setTimeout(poll, pollRetryDelay(failures.current))
      }
    }
    timeout = window.setTimeout(poll, POLL_INTERVAL_MS)
    return () => { mounted = false; if (timeout) window.clearTimeout(timeout) }
  }, [activeJobId, busy, setBusy, setError])

  async function cancelCurrentJob() {
    if (!activeJobId || busy !== 'running') return
    try { const status = await cancelAnonymizeJob(activeJobId); acceptStatus(status) } catch (caught) { setError(messageFrom(caught)) }
  }
  function clearJobState() { dispatch({ type: 'cleared' }) }
  return { jobStatus: state.status, acceptStatus, onProgress, cancelCurrentJob, clearJobState }
}
