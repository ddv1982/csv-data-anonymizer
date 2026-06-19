import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  cancelLocalAiModelDownload,
  getLocalAiModelDownloadStatus,
  getLocalAiStatus,
  openLocalAiSetupUrl,
  startLocalAiModelDownload,
} from '../tauri'
import type { AppSettings, LocalAiDownloadStatus, LocalAiRequest, LocalAiStatus } from '../types'
import { messageFrom } from '../utils/errors'

export function useLocalAi(settings: AppSettings, onError: (message: string) => void) {
  const [status, setStatus] = useState<LocalAiStatus | null>(null)
  const [downloadJobId, setDownloadJobId] = useState<string | null>(null)
  const [downloadStatus, setDownloadStatus] = useState<LocalAiDownloadStatus | null>(null)
  const request = useMemo(() => localAiRequest(settings), [settings.localAiEnabled, settings.localAiModel])

  const refresh = useCallback(async () => {
    try {
      setStatus(await getLocalAiStatus(request))
    } catch (caught) {
      setStatus(null)
      onError(messageFrom(caught))
    }
  }, [onError, request])

  const startDownload = useCallback(async () => {
    try {
      const nextStatus = await startLocalAiModelDownload(request)
      setDownloadStatus(nextStatus)
      setDownloadJobId(nextStatus.jobId)
    } catch (caught) {
      onError(messageFrom(caught))
    }
  }, [onError, request])

  const cancelDownload = useCallback(async () => {
    if (!downloadJobId) return

    try {
      const nextStatus = await cancelLocalAiModelDownload(downloadJobId)
      setDownloadStatus(nextStatus)
      if (nextStatus.state !== 'running') {
        setDownloadJobId(null)
      }
    } catch (caught) {
      onError(messageFrom(caught))
    }
  }, [downloadJobId, onError])

  const openSetup = useCallback(async () => {
    try {
      await openLocalAiSetupUrl()
    } catch (caught) {
      onError(messageFrom(caught))
    }
  }, [onError])

  useEffect(() => {
    let isMounted = true
    getLocalAiStatus(request)
      .then((nextStatus) => {
        if (isMounted) setStatus(nextStatus)
      })
      .catch((caught: unknown) => {
        if (isMounted) {
          setStatus(null)
          onError(messageFrom(caught))
        }
      })

    return () => {
      isMounted = false
    }
  }, [onError, request])

  useEffect(() => {
    if (!downloadJobId) return

    const jobId = downloadJobId
    let isMounted = true
    let timeoutId: number | undefined

    async function pollDownload() {
      try {
        const nextStatus = await getLocalAiModelDownloadStatus(jobId)
        if (!isMounted) return
        setDownloadStatus(nextStatus)
        if (nextStatus.state === 'running') {
          timeoutId = window.setTimeout(pollDownload, 500)
        } else {
          setDownloadJobId(null)
          void refresh()
        }
      } catch (caught) {
        if (!isMounted) return
        setDownloadJobId(null)
        onError(messageFrom(caught))
      }
    }

    timeoutId = window.setTimeout(pollDownload, 500)

    return () => {
      isMounted = false
      if (timeoutId) window.clearTimeout(timeoutId)
    }
  }, [downloadJobId, onError, refresh])

  return {
    request,
    status,
    downloadStatus,
    ready: Boolean(settings.localAiEnabled && status?.ready),
    downloadRunning: downloadStatus?.state === 'running',
    refresh,
    startDownload,
    cancelDownload,
    openSetup,
  }
}

function localAiRequest(settings: AppSettings): LocalAiRequest {
  return {
    enabled: settings.localAiEnabled,
    model: settings.localAiModel,
  }
}
