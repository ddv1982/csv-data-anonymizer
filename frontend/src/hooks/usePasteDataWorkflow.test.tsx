import { act, render } from '@testing-library/react'
import { useEffect } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defaultSettings } from '../defaults'
import { columnMetadataFixture, completeDetectionCoverage } from '../test-utils/builders'
import type { LocalAiState } from './useLocalAi'
import { usePasteDataWorkflow } from './usePasteDataWorkflow'

type PasteDataWorkflowState = ReturnType<typeof usePasteDataWorkflow>

const tauriMocks = vi.hoisted(() => ({
  analyzePasteData: vi.fn(),
  previewPasteData: vi.fn(),
  transformPasteData: vi.fn(),
}))

vi.mock('../tauri', () => tauriMocks)

describe('usePasteDataWorkflow', () => {
  beforeEach(() => {
    vi.resetAllMocks()
    tauriMocks.analyzePasteData.mockResolvedValue({
      format: 'json',
      rowCount: 1,
      rowCountIsComplete: true,
      detectionCoverage: completeDetectionCoverage,
      columns: [columnMetadataFixture({ index: 0, name: '[].email', isSelected: true })],
    })
    tauriMocks.previewPasteData.mockResolvedValue({
      previews: [],
      warnings: [],
      smartReplacements: [],
    })
    tauriMocks.transformPasteData.mockResolvedValue({ output: '{}', privacyReport: null })
  })

  it('analyzes content and invalidates derived data when the content changes', async () => {
    const harness = renderWorkflow()

    act(() => harness.workflow.setContent('[{"email":"ada@example.com"}]'))
    await act(async () => harness.workflow.analyze())

    expect(tauriMocks.analyzePasteData).toHaveBeenCalledWith({
      content: '[{"email":"ada@example.com"}]',
      format: 'auto',
      sampleRowCount: defaultSettings.sampleRowCount,
    })
    expect(harness.workflow.analysis?.format).toBe('json')
    expect(harness.workflow.selectedUsesLocalAi).toBe(false)

    act(() => harness.workflow.setContent('[{"email":"grace@example.com"}]'))

    expect(harness.workflow.analysis).toBeNull()
    expect(harness.workflow.preview).toBeNull()
    expect(harness.workflow.result).toBeNull()
    expect(harness.workflow.selection.selectedColumns).toEqual([])
  })

  it('forwards preview failures and restores the idle state', async () => {
    const onError = vi.fn()
    tauriMocks.previewPasteData.mockRejectedValue(new Error('Preview failed'))
    const harness = renderWorkflow(onError)

    act(() => harness.workflow.setContent('[{"email":"ada@example.com"}]'))
    await act(async () => harness.workflow.analyze())
    await act(async () => harness.workflow.showPreview())

    expect(onError).toHaveBeenLastCalledWith('Preview failed')
    expect(harness.workflow.busy).toBe('idle')
    expect(harness.workflow.preview).toBeNull()
  })

  it('invalidates detection results when local NER changes but preserves source input', async () => {
    const harness = renderWorkflow()
    act(() => {
      harness.workflow.setFormat('json')
      harness.workflow.setContent('[{"email":"ada@example.com"}]')
    })
    await act(async () => harness.workflow.analyze())
    expect(harness.workflow.analysis).not.toBeNull()

    harness.rerender({ ...defaultSettings, localNerEnabled: true })

    expect(harness.workflow.content).toBe('[{"email":"ada@example.com"}]')
    expect(harness.workflow.format).toBe('json')
    expect(harness.workflow.analysis).toBeNull()
    expect(harness.workflow.selection.selectedColumns).toEqual([])
    expect(harness.workflow.preview).toBeNull()
    expect(harness.workflow.result).toBeNull()
    expect(harness.workflow.copyStatus).toBeNull()
  })

  it('ignores analysis that completes after the content changes', async () => {
    const pending = deferred<{
      format: 'json'
      rowCount: number
      rowCountIsComplete: boolean
      detectionCoverage: typeof completeDetectionCoverage
      columns: ReturnType<typeof columnMetadataFixture>[]
    }>()
    tauriMocks.analyzePasteData.mockReturnValue(pending.promise)
    const harness = renderWorkflow()

    act(() => harness.workflow.setContent('[{"email":"ada@example.com"}]'))
    let analyzePromise: Promise<void>
    act(() => {
      analyzePromise = harness.workflow.analyze()
    })
    act(() => harness.workflow.setContent('[{"email":"grace@example.com"}]'))
    await act(async () => {
      pending.resolve({
        format: 'json',
        rowCount: 1,
        rowCountIsComplete: true,
        detectionCoverage: completeDetectionCoverage,
        columns: [columnMetadataFixture({ index: 0, name: '[].email', isSelected: true })],
      })
      await analyzePromise!
    })

    expect(harness.workflow.analysis).toBeNull()
    expect(harness.workflow.selection.selectedColumns).toEqual([])
    expect(harness.workflow.busy).toBe('idle')
  })

  it('sends controls only for selected columns', async () => {
    const preparedAnalysis = {
      version: 1,
      sourceIdentity: 'paste',
      sourceFingerprint: 'sha256:paste',
      format: 'json',
      columns: [],
      detector: { status: 'completed' as const, detectorId: 'ollama:test' },
      detectionRunSummary: {
        deterministic: 'completed' as const,
        localNer: 'completed' as const,
        detectorId: 'ollama:test',
        examinedCells: 1,
        totalEligibleCells: 1,
        skippedOversizedCells: 0,
        acceptedCandidates: 0,
        rejectedCandidates: 0,
      },
      candidateEvidence: [],
      integrityChecksum: 'sha256:snapshot',
    }
    tauriMocks.analyzePasteData.mockResolvedValue({
      format: 'json',
      rowCount: 1,
      rowCountIsComplete: true,
      detectionCoverage: completeDetectionCoverage,
      columns: [
        columnMetadataFixture({ index: 0, name: '[].email', isSelected: true }),
        columnMetadataFixture({ index: 1, name: '[].city', isSelected: true }),
      ],
      preparedAnalysis,
    })
    const harness = renderWorkflow()

    act(() => harness.workflow.setContent('[{"email":"ada@example.com","city":"London"}]'))
    await act(async () => harness.workflow.analyze())
    act(() => {
      harness.workflow.updateColumnStrategy(harness.workflow.analysis!.columns[0], 'localAi')
      harness.workflow.setColumnSelection([1])
    })
    await act(async () => harness.workflow.showPreview())
    await act(async () => harness.workflow.transform())

    expect(tauriMocks.previewPasteData).toHaveBeenCalledWith(expect.objectContaining({
      content: expect.any(String),
      format: 'json',
      columns: [1],
      controls: [],
      sampleCount: expect.any(Number),
      sampleRowCount: defaultSettings.sampleRowCount,
      localAi: expect.any(Object),
      preparedAnalysis,
    }))
    expect(tauriMocks.transformPasteData).toHaveBeenCalledWith(expect.objectContaining({
      content: expect.any(String),
      format: 'json',
      columns: [1],
      controls: [],
      sampleRowCount: defaultSettings.sampleRowCount,
      previewSmartReplacements: expect.any(Array),
      localAi: expect.any(Object),
      preparedAnalysis,
    }))
  })

  it('keeps column controls locked until a preview operation finishes', async () => {
    const pending = deferred<{ previews: []; warnings: []; smartReplacements: [] }>()
    tauriMocks.previewPasteData.mockReturnValue(pending.promise)
    const harness = renderWorkflow()

    act(() => harness.workflow.setContent('[{"email":"ada@example.com"}]'))
    await act(async () => harness.workflow.analyze())
    let previewPromise: Promise<void>
    act(() => {
      previewPromise = harness.workflow.showPreview()
    })

    const selectedColumns = harness.workflow.selection.selectedColumns
    act(() => {
      harness.workflow.setColumnSelection([])
      harness.workflow.toggleColumn(harness.workflow.analysis!.columns[0])
      harness.workflow.updateColumnStrategy(harness.workflow.analysis!.columns[0], 'mask')
    })

    expect(harness.workflow.busy).toBe('previewing')
    expect(harness.workflow.selection.selectedColumns).toEqual(selectedColumns)
    expect(harness.workflow.selection.columnControls).toEqual({})

    await act(async () => {
      pending.resolve({ previews: [], warnings: [], smartReplacements: [] })
      await previewPromise!
    })

    expect(harness.workflow.busy).toBe('idle')
  })
})

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((nextResolve) => {
    resolve = nextResolve
  })
  return { promise, resolve }
}

function WorkflowHarness({
  onError,
  onUpdate,
  settings,
}: {
  onError: (message: string | null) => void
  onUpdate: (workflow: PasteDataWorkflowState) => void
  settings: typeof defaultSettings
}) {
  const workflow = usePasteDataWorkflow({
    settings,
    settingsLoaded: true,
    localAi: localAiFixture(),
    onError,
  })

  useEffect(() => onUpdate(workflow), [onUpdate, workflow])
  return null
}

function renderWorkflow(onError = vi.fn()) {
  let workflow: PasteDataWorkflowState | null = null
  const rendered = render(
    <WorkflowHarness
      settings={defaultSettings}
      onError={onError}
      onUpdate={(nextWorkflow) => { workflow = nextWorkflow }}
    />,
  )

  return {
    get workflow() {
      if (!workflow) throw new Error('workflow did not render')
      return workflow
    },
    rerender(settings: typeof defaultSettings) {
      rendered.rerender(
        <WorkflowHarness
          settings={settings}
          onError={onError}
          onUpdate={(nextWorkflow) => { workflow = nextWorkflow }}
        />,
      )
    },
  }
}

function localAiFixture(): LocalAiState {
  return {
    request: { enabled: false, model: 'gemma3:4b' },
    status: null,
    downloadStatus: null,
    selectedModel: 'gemma3:4b',
    statusMatchesModel: false,
    ready: false,
    downloadRunning: false,
    refresh: vi.fn(),
    startDownload: vi.fn(),
    cancelDownload: vi.fn(),
    openSetup: vi.fn(),
  }
}
