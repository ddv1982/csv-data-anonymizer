import { useEffect, useRef, useState } from 'react'
import { defaultSettings } from '../defaults'
import { loadSettings, saveSettings } from '../tauri'
import type { AppSettings } from '../types'
import { messageFrom } from '../utils/errors'

type PersistentSettingsOptions = {
  onError: (message: string) => void
}

export function usePersistentSettings({ onError }: PersistentSettingsOptions) {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings)
  const [settingsLoaded, setSettingsLoaded] = useState(false)
  const latestSettingsRef = useRef(defaultSettings)
  const settingsLoadedRef = useRef(false)
  const settingsSaveSequenceRef = useRef(0)
  const inFlightSettingsSavesRef = useRef(new Set<number>())
  // Held in a ref so the load effect below can stay dependency-free: it must run once
  // per mount, not again every time the caller passes a new inline callback.
  const onErrorRef = useRef(onError)

  useEffect(() => {
    onErrorRef.current = onError
  }, [onError])

  useEffect(() => {
    let isMounted = true
    loadSettings()
      .then((loaded) => {
        if (isMounted) {
          if (settingsSaveSequenceRef.current === 0) {
            settingsSaveSequenceRef.current += 1
            latestSettingsRef.current = loaded
            setSettings(loaded)
          }
          settingsLoadedRef.current = true
          setSettingsLoaded(true)
        }
      })
      .catch((caught: unknown) => {
        if (isMounted) {
          onErrorRef.current(messageFrom(caught))
          settingsLoadedRef.current = true
          setSettingsLoaded(true)
        }
      })

    return () => {
      isMounted = false
    }
  }, [])

  function applySettings(next: AppSettings) {
    latestSettingsRef.current = next
    setSettings(next)
  }

  function applyAuthoritativeSettings(next: AppSettings) {
    settingsSaveSequenceRef.current += 1
    applySettings(next)
  }

  async function persistSettings(next: AppSettings) {
    if (!settingsLoadedRef.current) return

    applySettings(next)
    const saveSequence = settingsSaveSequenceRef.current + 1
    settingsSaveSequenceRef.current = saveSequence
    inFlightSettingsSavesRef.current.add(saveSequence)
    let staleResponseNeedsReconcile = false

    try {
      const saved = await saveSettings(next)
      if (saveSequence === settingsSaveSequenceRef.current) {
        applySettings(saved)
      } else {
        staleResponseNeedsReconcile = true
      }
    } catch (caught) {
      if (saveSequence === settingsSaveSequenceRef.current) {
        onErrorRef.current(messageFrom(caught))
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
      onErrorRef.current(messageFrom(caught))
    }
  }

  return {
    settings,
    settingsLoaded,
    latestSettingsRef,
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
