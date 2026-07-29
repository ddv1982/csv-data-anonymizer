import { act, render } from '@testing-library/react'
import { useEffect } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { defaultSettings } from '../defaults'
import { columnMetadataFixture } from '../test-utils/builders'
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

    expect(tauriMocks.analyzePasteData).toHaveBeenCalledWith(
      '[{"email":"ada@example.com"}]',
      'auto',
      defaultSettings.sampleRowCount,
    )
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

  it('ignores analysis that completes after the content changes', async () => {
    const pending = deferred<{
      format: 'json'
      rowCount: number
      rowCountIsComplete: boolean
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
        columns: [columnMetadataFixture({ index: 0, name: '[].email', isSelected: true })],
      })
      await analyzePromise!
    })

    expect(harness.workflow.analysis).toBeNull()
    expect(harness.workflow.selection.selectedColumns).toEqual([])
    expect(harness.workflow.busy).toBe('idle')
  })

  it('sends controls only for selected columns', async () => {
    tauriMocks.analyzePasteData.mockResolvedValue({
      format: 'json',
      rowCount: 1,
      rowCountIsComplete: true,
      columns: [
        columnMetadataFixture({ index: 0, name: '[].email', isSelected: true }),
        columnMetadataFixture({ index: 1, name: '[].city', isSelected: true }),
      ],
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

    expect(tauriMocks.previewPasteData).toHaveBeenCalledWith(
      expect.any(String),
      'json',
      [1],
      [],
      expect.any(Number),
      defaultSettings.sampleRowCount,
      expect.any(Object),
    )
    expect(tauriMocks.transformPasteData).toHaveBeenCalledWith(
      expect.any(String),
      'json',
      [1],
      [],
      defaultSettings.sampleRowCount,
      expect.any(Array),
      expect.any(Object),
    )
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
}: {
  onError: (message: string | null) => void
  onUpdate: (workflow: PasteDataWorkflowState) => void
}) {
  const workflow = usePasteDataWorkflow({
    settings: defaultSettings,
    settingsLoaded: true,
    localAi: localAiFixture(),
    onError,
  })

  useEffect(() => onUpdate(workflow), [onUpdate, workflow])
  return null
}

function renderWorkflow(onError = vi.fn()) {
  let workflow: PasteDataWorkflowState | null = null
  render(<WorkflowHarness onError={onError} onUpdate={(nextWorkflow) => { workflow = nextWorkflow }} />)

  return {
    get workflow() {
      if (!workflow) throw new Error('workflow did not render')
      return workflow
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
