import { act, render } from '@testing-library/react'
import { useEffect } from 'react'
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { defaultSettings } from '../defaults'
import { localAiStatusFixture } from '../test-utils/builders'
import type { AppSettings, LocalAiDownloadStatus } from '../types'
import { useLocalAi, type LocalAiState } from './useLocalAi'

const tauriMocks = vi.hoisted(() => ({
  getLocalAiStatus: vi.fn(),
  startLocalAiModelDownload: vi.fn(),
  getLocalAiModelDownloadStatus: vi.fn(),
  cancelLocalAiModelDownload: vi.fn(),
  openLocalAiSetupUrl: vi.fn(),
}))

vi.mock('../tauri', () => tauriMocks)

describe('useLocalAi', () => {
  beforeEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
    tauriMocks.getLocalAiStatus.mockResolvedValue(localAiStatusFixture())
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('releases the running download state when a status poll fails', async () => {
    vi.useFakeTimers()
    const onError = vi.fn()
    tauriMocks.startLocalAiModelDownload.mockResolvedValue(downloadStatusFixture())
    tauriMocks.getLocalAiModelDownloadStatus.mockRejectedValue(
      new Error('Download status unavailable'),
    )
    const harness = renderLocalAi(onError)
    await flushPromises()

    await act(async () => {
      await harness.localAi.startDownload()
    })

    expect(harness.localAi.downloadRunning).toBe(true)

    await act(async () => {
      await vi.advanceTimersByTimeAsync(500)
    })

    expect(tauriMocks.getLocalAiModelDownloadStatus).toHaveBeenCalledWith('download-1')
    expect(harness.localAi.downloadRunning).toBe(false)
    expect(harness.localAi.downloadStatus).toBeNull()
    expect(onError).toHaveBeenCalledWith('Download status unavailable')
  })

  it('completes a download poll and refreshes the Local AI status', async () => {
    vi.useFakeTimers()
    const onError = vi.fn()
    tauriMocks.startLocalAiModelDownload.mockResolvedValue(downloadStatusFixture())
    tauriMocks.getLocalAiModelDownloadStatus.mockResolvedValue({
      ...downloadStatusFixture(),
      state: 'succeeded',
      completedBytes: 100,
    })
    const harness = renderLocalAi(onError)
    await flushPromises()

    await act(async () => {
      await harness.localAi.startDownload()
    })

    expect(harness.localAi.downloadRunning).toBe(true)

    await act(async () => {
      await vi.advanceTimersByTimeAsync(500)
    })

    expect(harness.localAi.downloadRunning).toBe(false)
    expect(harness.localAi.downloadStatus?.state).toBe('succeeded')
    expect(tauriMocks.getLocalAiStatus).toHaveBeenCalledTimes(2)
    expect(onError).not.toHaveBeenCalled()
  })
})

function LocalAiHarness({
  onError,
  onUpdate,
}: {
  onError: (message: string) => void
  onUpdate: (localAi: LocalAiState) => void
}) {
  const localAi = useLocalAi(settingsFixture(), onError)

  useEffect(() => {
    onUpdate(localAi)
  }, [onUpdate, localAi])

  return null
}

function renderLocalAi(onError: (message: string) => void) {
  let localAi: LocalAiState | null = null
  render(
    <LocalAiHarness
      onError={onError}
      onUpdate={(nextLocalAi) => {
        localAi = nextLocalAi
      }}
    />,
  )

  return {
    get localAi() {
      if (!localAi) throw new Error('localAi did not render')
      return localAi
    },
  }
}

async function flushPromises() {
  await act(async () => {
    await Promise.resolve()
    await Promise.resolve()
  })
}

function settingsFixture(): AppSettings {
  return { ...defaultSettings }
}

function downloadStatusFixture(overrides: Partial<LocalAiDownloadStatus> = {}): LocalAiDownloadStatus {
  return {
    jobId: 'download-1',
    state: 'running',
    model: 'gemma3:4b',
    statusMessage: 'Downloading model...',
    completedBytes: 10,
    totalBytes: 100,
    cancelRequested: false,
    error: null,
    ...overrides,
  }
}
