import { useEffect, useRef, useState } from 'react'
import { defaultSettings } from '../defaults'
import { loadSettings, saveSettings } from '../tauri'
import type { AppSettings } from '../types'
import { messageFrom } from '../utils/errors'

type PersistentSettingsOptions = {
  onError: (message: string) => void
  onAcceptedSettings?: (settings: AppSettings) => void
}

export function usePersistentSettings({ onError, onAcceptedSettings }: PersistentSettingsOptions) {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings)
  const latestSettingsRef = useRef(defaultSettings)
  const settingsSaveSequenceRef = useRef(0)
  const inFlightSettingsSavesRef = useRef(new Set<number>())
  const callbacksRef = useRef({ onError, onAcceptedSettings })

  useEffect(() => {
    callbacksRef.current = { onError, onAcceptedSettings }
  }, [onError, onAcceptedSettings])

  useEffect(() => {
    let isMounted = true
    loadSettings()
      .then((loaded) => {
        if (isMounted && settingsSaveSequenceRef.current === 0) {
          settingsSaveSequenceRef.current += 1
          latestSettingsRef.current = loaded
          setSettings(loaded)
          callbacksRef.current.onAcceptedSettings?.(loaded)
        }
      })
      .catch((caught: unknown) => {
        if (isMounted) callbacksRef.current.onError(messageFrom(caught))
      })

    return () => {
      isMounted = false
    }
  }, [])

  function applySettings(next: AppSettings) {
    latestSettingsRef.current = next
    setSettings(next)
  }

  function acceptSettings(next: AppSettings) {
    applySettings(next)
    callbacksRef.current.onAcceptedSettings?.(next)
  }

  function applyAuthoritativeSettings(next: AppSettings) {
    settingsSaveSequenceRef.current += 1
    acceptSettings(next)
  }

  async function persistSettings(next: AppSettings) {
    applySettings(next)
    const saveSequence = settingsSaveSequenceRef.current + 1
    settingsSaveSequenceRef.current = saveSequence
    inFlightSettingsSavesRef.current.add(saveSequence)
    let staleResponseNeedsReconcile = false

    try {
      const saved = await saveSettings(next)
      if (saveSequence === settingsSaveSequenceRef.current) {
        acceptSettings(saved)
      } else {
        staleResponseNeedsReconcile = true
      }
    } catch (caught) {
      if (saveSequence === settingsSaveSequenceRef.current) {
        callbacksRef.current.onError(messageFrom(caught))
      }
    } finally {
      inFlightSettingsSavesRef.current.delete(saveSequence)
      if (
        staleResponseNeedsReconcile &&
        !hasNewerSettingsSaveInFlight(saveSequence, inFlightSettingsSavesRef.current)
      ) {
        void persistSettings(latestSettingsRef.current)
      }
    }
  }

  async function refreshSettings() {
    try {
      const loaded = await loadSettings()
      applyAuthoritativeSettings(loaded)
    } catch (caught) {
      callbacksRef.current.onError(messageFrom(caught))
    }
  }

  return {
    settings,
    latestSettingsRef,
    applyAuthoritativeSettings,
    persistSettings,
    refreshSettings,
  }
}

function hasNewerSettingsSaveInFlight(saveSequence: number, inFlight: Set<number>) {
  for (const inFlightSequence of inFlight) {
    if (inFlightSequence > saveSequence) return true
  }
  return false
}
