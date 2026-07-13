import { act, render } from '@testing-library/react'
import { useEffect } from 'react'
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import { defaultSettings } from '../defaults'
import { localAiStatusFixture } from '../test-utils/builders'
import type { AppSettings, LocalAiDownloadStatus, LocalAiStatus } from '../types'
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

  it('keeps the active download cancellable when the selected model changes', async () => {
    const onError = vi.fn()
    tauriMocks.startLocalAiModelDownload.mockResolvedValue(downloadStatusFixture())
    const harness = renderLocalAi(onError)
    await flushPromises()

    await act(async () => {
      await harness.localAi.startDownload()
    })

    expect(harness.localAi.downloadRunning).toBe(true)

    harness.rerender(settingsFixture({ localAiModel: 'llama3:8b' }))

    expect(harness.localAi.selectedModel).toBe('llama3:8b')
    expect(harness.localAi.downloadRunning).toBe(true)
  })

  it('keeps the newest status when an older refresh resolves last', async () => {
    const onError = vi.fn()
    const first = deferred<LocalAiStatus>()
    const second = deferred<LocalAiStatus>()
    tauriMocks.getLocalAiStatus
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const harness = renderLocalAi(onError)
    await flushPromises()

    harness.rerender(settingsFixture({ localAiEnabled: true, localAiModel: 'llama3:8b' }))
    await flushPromises()

    await act(async () => {
      second.resolve(localAiStatusFixture({ model: 'llama3:8b', ready: true }))
      await second.promise
    })
    expect(harness.localAi.status?.model).toBe('llama3:8b')
    expect(harness.localAi.ready).toBe(true)

    await act(async () => {
      first.resolve(localAiStatusFixture({ model: 'gemma3:4b', ready: true }))
      await first.promise
    })
    expect(harness.localAi.status?.model).toBe('llama3:8b')
    expect(harness.localAi.ready).toBe(true)
    expect(onError).not.toHaveBeenCalled()
  })

  it('ignores an older refresh failure after the newest status succeeds', async () => {
    const onError = vi.fn()
    const first = deferred<LocalAiStatus>()
    const second = deferred<LocalAiStatus>()
    tauriMocks.getLocalAiStatus
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const harness = renderLocalAi(onError)
    await flushPromises()

    harness.rerender(settingsFixture({ localAiEnabled: true, localAiModel: 'llama3:8b' }))
    await flushPromises()

    await act(async () => {
      second.resolve(localAiStatusFixture({ model: 'llama3:8b', ready: true }))
      await second.promise
    })
    await act(async () => {
      first.reject(new Error('Stale status failure'))
      await first.promise.catch(() => undefined)
    })

    expect(harness.localAi.status?.model).toBe('llama3:8b')
    expect(harness.localAi.ready).toBe(true)
    expect(onError).not.toHaveBeenCalled()
  })
})

function LocalAiHarness({
  settings,
  onError,
  onUpdate,
}: {
  settings: AppSettings
  onError: (message: string) => void
  onUpdate: (localAi: LocalAiState) => void
}) {
  const localAi = useLocalAi(settings, onError)

  useEffect(() => {
    onUpdate(localAi)
  }, [onUpdate, localAi])

  return null
}

function renderLocalAi(onError: (message: string) => void, settings = settingsFixture()) {
  let localAi: LocalAiState | null = null
  const result = render(
    <LocalAiHarness
      settings={settings}
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
    rerender(nextSettings: AppSettings) {
      result.rerender(
        <LocalAiHarness
          settings={nextSettings}
          onError={onError}
          onUpdate={(nextLocalAi) => {
            localAi = nextLocalAi
          }}
        />,
      )
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
  return { ...defaultSettings, ...overrides }
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

function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise
    reject = rejectPromise
  })
  return { promise, resolve, reject }
}
