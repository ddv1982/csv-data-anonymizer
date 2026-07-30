import { act, render } from '@testing-library/react'
import { useEffect } from 'react'
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { verifiedPreflightFixture } from '../test-utils/builders'
import {
  columnFixture as riskColumnFixture,
  resetTauriMocks,
  settingsFixture,
  tauriMocks,
  transformedPrivacyReportFixture,
} from '../test-utils/mocks'
import type { AnonymizeData, AnonymizeJobStatus, ColumnMetadata } from '../types'
import { useAnonymizerWorkflow, type AnonymizerWorkflowState } from './useAnonymizerWorkflow'

// Dynamic, because the factory runs while the hook under test pulls in `../tauri` — ahead of
// this file's own imports, so a plain reference to `tauriMocks` would still be in its dead zone.
vi.mock('../tauri', async () => (await import('../test-utils/mocks')).tauriMocks)

describe('useAnonymizerWorkflow', () => {
  beforeEach(() => {
    vi.useRealTimers()
    resetTauriMocks()
    tauriMocks.pickInputCsv.mockResolvedValue('/data/input.csv')
    tauriMocks.pickOutputCsv.mockResolvedValue('/data/custom-output.csv')
    tauriMocks.countCsvRows.mockResolvedValue(2)
    tauriMocks.previewAnonymization.mockResolvedValue({
      previews: [],
      warnings: [],
      smartReplacements: [],
    })
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('loads a picked CSV, refreshes exact row count, and persists remembered directories', async () => {
    const settings = settingsFixture({
      rememberLastPaths: true,
      lastInputDirectory: '/last/input',
      lastOutputDirectory: '/last/output',
    })
    tauriMocks.loadSettings.mockResolvedValue(settings)
    tauriMocks.pickInputCsv.mockResolvedValue('/data/input.csv')
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture({ rowCountIsComplete: false }))
    tauriMocks.countCsvRows.mockResolvedValue(42)

    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await flushPromises()

    expect(tauriMocks.pickInputCsv).toHaveBeenCalledWith('/last/input')
    expect(tauriMocks.analyzeCsv).toHaveBeenCalledWith('/data/input.csv', 100, '_private_output')
    expect(tauriMocks.countCsvRows).toHaveBeenCalledWith('/data/input.csv')
    expect(harness.workflow.inputPath).toBe('/data/input.csv')
    expect(harness.workflow.outputPath).toBe('/data/input_private_output.csv')
    expect(harness.workflow.selectedColumns).toEqual([0, 1])
    expect(harness.workflow.headers?.rowCount).toBe(42)
    expect(tauriMocks.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        lastInputDirectory: '/data',
        lastOutputDirectory: '/data',
      }),
    )
  })

  it('updates column selection and sends controlled preview payloads', async () => {
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      harness.workflow.handleInputChange('/data/input.csv')
      await harness.workflow.previewCsv('/data/input.csv', [])
    })

    expect(tauriMocks.previewAnonymization).not.toHaveBeenCalled()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await flushPromises()

    act(() => {
      harness.workflow.updateColumnStrategy(harness.workflow.columns[1], 'mask')
      harness.workflow.setColumnSelection([1, 0, 1])
    })
    await act(async () => {
      await harness.workflow.previewCsv()
    })

    expect(harness.workflow.selectedColumns).toEqual([0, 1])
    expect(tauriMocks.previewAnonymization).toHaveBeenCalledWith(
      '/data/input.csv',
      [0, 1],
      [{ columnIndex: 1, typeOverride: null, strategy: 'mask' }],
      5,
      100,
      { enabled: false, model: 'gemma3:4b' },
    )
  })

  it('polls a started job to success and persists the output directory', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.getAnonymizeJobStatus.mockResolvedValue(succeededJobStatus())
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await act(async () => {
      await harness.workflow.runAnonymization()
    })

    expect(tauriMocks.startAnonymizeJob).toHaveBeenCalledWith(
      '/data/input.csv',
      '/data/input_private_output.csv',
      [0, 1],
      [],
      false,
      100,
      2,
      [],
      { enabled: false, model: 'gemma3:4b' },
    )

    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })

    expect(tauriMocks.getAnonymizeJobStatus).toHaveBeenCalledWith('job-1')
    expect(harness.workflow.result?.outputPath).toBe('/out/final.csv')
    expect(harness.workflow.busy).toBe('idle')
    expect(tauriMocks.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({ lastOutputDirectory: '/out' }),
    )
  })

  it('cancels an active job and reports cancellation', async () => {
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.cancelAnonymizeJob.mockResolvedValue({
      ...runningJobStatus(),
      state: 'canceled',
      cancelRequested: true,
    })
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await act(async () => {
      await harness.workflow.runAnonymization()
    })
    await act(async () => {
      await harness.workflow.cancelCurrentJob()
    })

    expect(tauriMocks.cancelAnonymizeJob).toHaveBeenCalledWith('job-1')
    expect(harness.workflow.error).toBe('Output creation canceled.')
    expect(harness.workflow.busy).toBe('idle')
  })

  it('surfaces the failure message when a job reaches the failed state', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.getAnonymizeJobStatus.mockResolvedValue({
      ...runningJobStatus(),
      state: 'failed',
      error: 'Simulated backend failure',
    })
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await act(async () => {
      await harness.workflow.runAnonymization()
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })

    expect(harness.workflow.error).toBe('Simulated backend failure')
    expect(harness.workflow.busy).toBe('idle')
    expect(harness.workflow.result).toBeNull()
    expect(harness.workflow.jobStatus).toBeNull()
  })

  it('aborts the run and surfaces the blocker when preflight reports blockers', async () => {
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.preflightAnonymization.mockResolvedValue(
      verifiedPreflightFixture({
        readiness: {
          status: 'blocked',
          blockers: ['Output path matches the input file.'],
          reviewItems: [],
          verifiedItems: [],
        },
      }),
    )
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await act(async () => {
      await harness.workflow.runAnonymization()
    })

    expect(tauriMocks.startAnonymizeJob).not.toHaveBeenCalled()
    expect(harness.workflow.error).toBe('Output path matches the input file.')
    expect(harness.workflow.busy).toBe('idle')
  })

  it('retries a failed job poll once before completing on the next tick', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.getAnonymizeJobStatus
      .mockRejectedValueOnce(new Error('Transient poll failure'))
      .mockResolvedValueOnce(succeededJobStatus())
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await act(async () => {
      await harness.workflow.runAnonymization()
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })

    expect(harness.workflow.busy).toBe('running')
    expect(harness.workflow.error).toBeNull()

    await act(async () => {
      await vi.advanceTimersByTimeAsync(300)
    })

    expect(tauriMocks.getAnonymizeJobStatus).toHaveBeenCalledTimes(2)
    expect(harness.workflow.result?.outputPath).toBe('/out/final.csv')
    expect(harness.workflow.busy).toBe('idle')
    expect(tauriMocks.cancelAnonymizeJob).not.toHaveBeenCalled()
  })

  it('keeps polling through transient failures and never cancels the job itself', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    // Four failures, past the reporting threshold, then contact returns. A run that
    // recovers has to be allowed to finish: the old behaviour cancelled it after two.
    tauriMocks.getAnonymizeJobStatus
      .mockRejectedValueOnce(new Error('Simulated poll failure'))
      .mockRejectedValueOnce(new Error('Simulated poll failure'))
      .mockRejectedValueOnce(new Error('Simulated poll failure'))
      .mockRejectedValueOnce(new Error('Simulated poll failure'))
      .mockResolvedValue(succeededJobStatus())
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => harness.workflow.handlePickInput())
    await act(async () => harness.workflow.runAnonymization())
    // Long enough to cover the backoff between the failed polls and the retry that
    // succeeds, rather than a fixed multiple of the poll interval.
    await act(async () => vi.advanceTimersByTimeAsync(20_000))

    expect(tauriMocks.cancelAnonymizeJob).not.toHaveBeenCalled()
    expect(harness.workflow.result?.outputPath).toBe('/out/final.csv')
    expect(harness.workflow.busy).toBe('idle')
    // The lost-contact message is cleared once contact returns.
    expect(harness.workflow.error).toBeNull()
  })

  it('reports lost contact while retrying without ending the run', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.getAnonymizeJobStatus.mockRejectedValue(new Error('Simulated poll failure'))
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => harness.workflow.handlePickInput())
    await act(async () => harness.workflow.runAnonymization())
    await act(async () => vi.advanceTimersByTimeAsync(20_000))

    expect(harness.workflow.error).toContain('Lost contact with the running job')
    // Still running, so the user keeps a working Cancel button and the job is not
    // reported as failed while it may still be streaming rows.
    expect(harness.workflow.busy).toBe('running')
    expect(tauriMocks.cancelAnonymizeJob).not.toHaveBeenCalled()
  })

  it('backs off 300/600/1200 and caps at 5000ms between failed polls', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.getAnonymizeJobStatus.mockRejectedValue(new Error('Simulated poll failure'))
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => harness.workflow.handlePickInput())
    await act(async () => harness.workflow.runAnonymization())

    // Each pair advances to one tick before the expected poll and then over it, so a
    // constant delay or a wrong exponent cannot pass: a blanket advance would.
    const advance = async (ms: number) => {
      await act(async () => vi.advanceTimersByTimeAsync(ms))
    }
    const expectPollCount = (count: number) => {
      expect(tauriMocks.getAnonymizeJobStatus).toHaveBeenCalledTimes(count)
    }

    await advance(299)
    expectPollCount(0)
    await advance(1)
    expectPollCount(1) // first poll at the 300ms interval

    await advance(299)
    expectPollCount(1)
    await advance(1)
    expectPollCount(2) // retry 1: 300ms

    await advance(599)
    expectPollCount(2)
    await advance(1)
    expectPollCount(3) // retry 2: 600ms

    await advance(1_199)
    expectPollCount(3)
    await advance(1)
    expectPollCount(4) // retry 3: 1200ms

    await advance(2_399)
    expectPollCount(4)
    await advance(1)
    expectPollCount(5) // retry 4: 2400ms

    await advance(4_799)
    expectPollCount(5)
    await advance(1)
    expectPollCount(6) // retry 5: 4800ms

    await advance(4_999)
    expectPollCount(6)
    await advance(1)
    expectPollCount(7) // retry 6 would be 9600ms, capped to 5000ms

    await advance(4_999)
    expectPollCount(7)
    await advance(1)
    expectPollCount(8) // and stays capped

    expect(harness.workflow.busy).toBe('running')
  })

  it('stops polling and releases the UI after two minutes of lost contact', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    // A permanently failing poll — an unknown job id, or a poisoned registry mutex —
    // can never succeed on retry, so without a deadline `busy` would stay 'running'
    // forever and the only recovery would be killing the app.
    tauriMocks.getAnonymizeJobStatus.mockRejectedValue(new Error('Simulated poll failure'))
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => harness.workflow.handlePickInput())
    await act(async () => harness.workflow.runAnonymization())

    await act(async () => vi.advanceTimersByTimeAsync(119_000))
    expect(harness.workflow.busy).toBe('running')

    await act(async () => vi.advanceTimersByTimeAsync(11_000))

    expect(harness.workflow.busy).toBe('idle')
    expect(harness.workflow.jobStatus).toBeNull()
    // Not reported as a failure: the job may still be writing, so the message must warn
    // against reusing the output path rather than invite an immediate retry.
    expect(harness.workflow.error).toContain('stopped tracking it')
    expect(harness.workflow.error).toContain('may still be running')
    expect(tauriMocks.cancelAnonymizeJob).not.toHaveBeenCalled()

    // Polling really stopped, rather than continuing behind an idle UI.
    const pollsAtDeadline = tauriMocks.getAnonymizeJobStatus.mock.calls.length
    await act(async () => vi.advanceTimersByTimeAsync(60_000))
    expect(tauriMocks.getAnonymizeJobStatus).toHaveBeenCalledTimes(pollsAtDeadline)
  })

  it('requests cancel during an outage and recovers through the lost-contact deadline', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.getAnonymizeJobStatus.mockRejectedValue(new Error('Simulated poll failure'))
    // What the backend actually returns for cancelling a *running* job: the state stays
    // Running with cancelRequested set, and the terminal 'canceled' state is only ever
    // published to a later status poll.
    tauriMocks.cancelAnonymizeJob.mockResolvedValue({
      ...runningJobStatus(),
      cancelRequested: true,
    })
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => harness.workflow.handlePickInput())
    await act(async () => harness.workflow.runAnonymization())
    await act(async () => vi.advanceTimersByTimeAsync(2_000))
    await act(async () => harness.workflow.cancelCurrentJob())

    expect(tauriMocks.cancelAnonymizeJob).toHaveBeenCalledWith('job-1')
    // Cancel alone cannot end the wait: the response is non-terminal, and the UI now
    // disables the Cancel button because cancelRequested is set.
    expect(harness.workflow.busy).toBe('running')
    expect(harness.workflow.jobStatus?.cancelRequested).toBe(true)

    await act(async () => vi.advanceTimersByTimeAsync(130_000))

    expect(harness.workflow.busy).toBe('idle')
    expect(harness.workflow.error).toContain('stopped tracking it')
  })

  it('keeps an unrelated error when a poll recovers', async () => {
    vi.useFakeTimers()
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    tauriMocks.startAnonymizeJob.mockResolvedValue(runningJobStatus())
    tauriMocks.getAnonymizeJobStatus
      .mockRejectedValueOnce(new Error('Simulated poll failure'))
      .mockRejectedValueOnce(new Error('Simulated poll failure'))
      .mockRejectedValueOnce(new Error('Simulated poll failure'))
      .mockResolvedValue(succeededJobStatus())
    tauriMocks.cancelAnonymizeJob.mockRejectedValue(new Error('Cancel request was rejected.'))
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => harness.workflow.handlePickInput())
    await act(async () => harness.workflow.runAnonymization())
    await act(async () => vi.advanceTimersByTimeAsync(1_200))

    expect(harness.workflow.error).toContain('Lost contact with the running job')

    // The user hits Cancel while out of contact and the request itself fails.
    await act(async () => harness.workflow.cancelCurrentJob())
    expect(harness.workflow.error).toBe('Cancel request was rejected.')

    await act(async () => vi.advanceTimersByTimeAsync(1_200))

    // Contact returned, but the message on screen is no longer ours to retract: the
    // user must still learn that the cancel never took.
    expect(tauriMocks.getAnonymizeJobStatus).toHaveBeenCalledTimes(4)
    expect(harness.workflow.error).toBe('Cancel request was rejected.')
    expect(harness.workflow.busy).toBe('idle')
  })

  it('recomputes the suggested output path when the suffix setting changes', async () => {
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    act(() => {
      harness.workflow.updateSetting('defaultOutputSuffix', '_safe')
    })

    expect(harness.workflow.outputPath).toBe('/data/input_safe.csv')
    expect(tauriMocks.saveSettings).toHaveBeenCalledWith(
      expect.objectContaining({ defaultOutputSuffix: '_safe' }),
    )
  })
})

function WorkflowHarness({ onUpdate }: { onUpdate: (workflow: AnonymizerWorkflowState) => void }) {
  const workflow = useAnonymizerWorkflow()

  useEffect(() => {
    onUpdate(workflow)
  }, [onUpdate, workflow])

  return null
}

function renderWorkflow() {
  let workflow: AnonymizerWorkflowState | null = null
  render(<WorkflowHarness onUpdate={(nextWorkflow) => { workflow = nextWorkflow }} />)

  return {
    get workflow() {
      if (!workflow) throw new Error('workflow did not render')
      return workflow
    },
  }
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
  })
}

function analyzeResponseFixture(overrides: Partial<ReturnType<typeof headersFixture>> = {}) {
  const headers = headersFixture(overrides)
  return {
    headers,
    selectedColumns: [0, 1],
    suggestedOutputPath: '/data/input_private_output.csv',
  }
}

function headersFixture(overrides: Partial<{
  rowCount: number
  rowCountIsComplete: boolean
  columns: ColumnMetadata[]
}> = {}) {
  return {
    filePath: '/data/input.csv',
    rowCount: overrides.rowCount ?? 2,
    rowCountIsComplete: overrides.rowCountIsComplete ?? true,
    defaultOutputPath: '/data/input_private_output.csv',
    columns: overrides.columns ?? [
      columnFixture(0, 'email', 'email', 'high'),
      columnFixture(1, 'country', 'countryCode', 'medium'),
    ],
  }
}

// Both columns arrive selected regardless of their risk: every assertion below is about what
// the workflow does with a selection, not about how one is arrived at.
const columnFixture = (
  index: number,
  name: string,
  detectedType: ColumnMetadata['detectedType'],
  piiRisk: ColumnMetadata['piiRisk'],
): ColumnMetadata => riskColumnFixture(index, name, detectedType, piiRisk, { isSelected: true })

function runningJobStatus(): AnonymizeJobStatus {
  return {
    jobId: 'job-1',
    state: 'running',
    rowsProcessed: 0,
    totalRows: 2,
    cancelRequested: false,
    result: null,
    error: null,
  }
}

function succeededJobStatus(): AnonymizeJobStatus {
  return {
    ...runningJobStatus(),
    state: 'succeeded',
    rowsProcessed: 2,
    result: resultFixture(),
  }
}

function resultFixture(): AnonymizeData {
  return {
    outputPath: '/out/final.csv',
    rowCount: 2,
    columnsAnonymized: 2,
    durationMs: 10,
    privacyReport: transformedPrivacyReportFixture({
      quasiIdentifiers: 1,
      uniquePseudonymValues: 2,
    }),
  }
}
