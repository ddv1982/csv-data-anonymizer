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
    vi.clearAllMocks()
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
})

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
