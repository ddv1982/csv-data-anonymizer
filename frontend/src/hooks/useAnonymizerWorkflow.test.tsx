import { act, render } from '@testing-library/react'
import { useEffect } from 'react'
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { defaultSettings, privacyConfigFromSettings } from '../defaults'
import type {
  AnonymizeData,
  AnonymizeJobStatus,
  AppSettings,
  ColumnMetadata,
  LocalAiStatus,
  PrivacyReport,
} from '../types'
import { useAnonymizerWorkflow, type AnonymizerWorkflowState } from './useAnonymizerWorkflow'

type PreflightLike = { readiness: { blockers: string[] } }

const tauriMocks = vi.hoisted(() => ({
  loadSettings: vi.fn(),
  saveSettings: vi.fn(),
  resetDpBudgetLedger: vi.fn(),
  pickInputCsv: vi.fn(),
  pickOutputCsv: vi.fn(),
  analyzeCsv: vi.fn(),
  countCsvRows: vi.fn(),
  preflightAnonymization: vi.fn(),
  firstPreflightBlocker: vi.fn((preflight: PreflightLike) => preflight.readiness.blockers[0] ?? null),
  previewAnonymization: vi.fn(),
  startAnonymizeJob: vi.fn(),
  getAnonymizeJobStatus: vi.fn(),
  cancelAnonymizeJob: vi.fn(),
  openOutputLocation: vi.fn(),
  getLocalAiStatus: vi.fn(),
  startLocalAiModelDownload: vi.fn(),
  getLocalAiModelDownloadStatus: vi.fn(),
  cancelLocalAiModelDownload: vi.fn(),
  openLocalAiSetupUrl: vi.fn(),
  setAppTheme: vi.fn(),
}))

vi.mock('../tauri', () => tauriMocks)

describe('useAnonymizerWorkflow', () => {
  beforeEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
    tauriMocks.loadSettings.mockResolvedValue(settingsFixture())
    tauriMocks.saveSettings.mockImplementation(async (settings: AppSettings) => settings)
    tauriMocks.getLocalAiStatus.mockResolvedValue(localAiStatusFixture())
    tauriMocks.pickInputCsv.mockResolvedValue('/data/input.csv')
    tauriMocks.pickOutputCsv.mockResolvedValue('/data/custom-output.csv')
    tauriMocks.countCsvRows.mockResolvedValue(2)
    tauriMocks.preflightAnonymization.mockResolvedValue(verifiedPreflightFixture())
    tauriMocks.firstPreflightBlocker.mockImplementation(
      (preflight: PreflightLike) => preflight.readiness.blockers[0] ?? null,
    )
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
      false,
      '',
      5,
      { enabled: false, model: 'gemma3:4b' },
    )
  })

  it('blocks DP aggregate output when repeatable replacements are enabled', async () => {
    tauriMocks.analyzeCsv.mockResolvedValue(analyzeResponseFixture())
    const harness = renderWorkflow()
    await flushPromises()

    await act(async () => {
      await harness.workflow.handlePickInput()
    })
    await act(async () => {
      harness.workflow.updateSetting('deterministicDefault', true)
      harness.workflow.updatePrivacyConfig({
        ...privacyConfigFromSettings(defaultSettings),
        releaseMode: 'differentialPrivacyAggregate',
      })
    })
    await act(async () => {
      await harness.workflow.runAnonymization()
    })

    expect(tauriMocks.startAnonymizeJob).not.toHaveBeenCalled()
    expect(harness.workflow.error).toBe('Turn off Repeatable replacements before creating DP aggregate output.')
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
      '',
      false,
      100,
      2,
      [],
      expect.objectContaining({ releaseMode: 'standard' }),
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

function settingsFixture(overrides: Partial<AppSettings> = {}): AppSettings {
  return {
    ...defaultSettings,
    ...overrides,
  }
}

function localAiStatusFixture(): LocalAiStatus {
  return {
    enabled: false,
    provider: 'ollama',
    model: 'gemma3:4b',
    availableModels: [],
    endpoint: 'http://127.0.0.1:11434',
    runtimeAvailable: false,
    modelInstalled: false,
    ready: false,
    runtimeVersion: null,
    message: 'Local AI is off.',
  }
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
}> = {}) {
  return {
    filePath: '/data/input.csv',
    rowCount: overrides.rowCount ?? 2,
    rowCountIsComplete: overrides.rowCountIsComplete ?? true,
    defaultOutputPath: '/data/input_private_output.csv',
    columns: [columnFixture(0, 'email', 'email', 'high'), columnFixture(1, 'country', 'countryCode', 'medium')],
  }
}

function columnFixture(
  index: number,
  name: string,
  detectedType: ColumnMetadata['detectedType'],
  piiRisk: ColumnMetadata['piiRisk'],
): ColumnMetadata {
  return {
    name,
    index,
    detectedType,
    confidence: 'high',
    piiRisk,
    sampleValues: ['sample'],
    emptyFormat: 'emptyString',
    isSelected: true,
    strategy: 'auto',
  }
}

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
    privacyReport: privacyReportFixture(),
  }
}

function privacyReportFixture(overrides: Partial<PrivacyReport> = {}): PrivacyReport {
  return {
    releaseMode: 'standard',
    directIdentifiers: 1,
    quasiIdentifiers: 1,
    sensitiveColumns: 0,
    pseudonymizedColumns: 1,
    smartReplacementColumns: 0,
    opaqueTokenColumns: 0,
    maskedColumns: 0,
    generalizedColumns: 0,
    passThroughColumns: 0,
    suppressedRows: 0,
    syntheticRows: 0,
    dpEpsilon: null,
    dpBudget: null,
    uniquePseudonymValues: 2,
    reusedPseudonymValues: 0,
    collisionsAvoided: 0,
    exhaustedPseudonymPools: 0,
    opaqueTokenValues: 0,
    smartReplacementValues: 0,
    smartReplacementRejections: 0,
    smartReplacementRejectionReasons: [],
    smartReplacementFallbacks: 0,
    formalModels: [],
    readiness: {
      status: 'verified',
      blockers: [],
      reviewItems: [],
      verifiedItems: [],
    },
    evidence: [],
    columnReports: [],
    utilityMetrics: [],
    notes: [],
    ...overrides,
  }
}

function verifiedPreflightFixture() {
  return {
    mode: 'anonymize',
    readiness: {
      status: 'verified',
      blockers: [],
      reviewItems: [],
      verifiedItems: [],
    },
    evidence: [],
    columnReports: [],
  }
}
