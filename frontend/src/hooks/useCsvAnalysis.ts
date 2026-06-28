import type { Dispatch, SetStateAction } from 'react'
import { privacyConfigFromSettings } from '../defaults'
import { analyzeCsv, countCsvRows, pickInputCsv, pickOutputCsv } from '../tauri'
import type {
  AnalyzeResponse,
  AnonymizeData,
  AppSettings,
  ColumnControl,
  PrivacyConfig,
} from '../types'
import { messageFrom } from '../utils/errors'
import { defaultOutputPathWithSuffix, directoryOf } from '../utils/paths'
import type { BusyState } from './workflowTypes'

type HeadersState = AnalyzeResponse['headers'] | null

type CsvSelectionState = {
  headers: HeadersState
  setHeaders: Dispatch<SetStateAction<HeadersState>>
  setLoadedCsv: (headers: AnalyzeResponse['headers'], selectedColumns: number[]) => void
  setColumnControls: Dispatch<SetStateAction<Record<number, ColumnControl>>>
}

type CsvAnalysisOptions = {
  settings: AppSettings
  settingsLoaded: boolean
  busy: BusyState
  setBusy: Dispatch<SetStateAction<BusyState>>
  setError: Dispatch<SetStateAction<string | null>>
  setResult: Dispatch<SetStateAction<AnonymizeData | null>>
  setPrivacyConfig: Dispatch<SetStateAction<PrivacyConfig>>
  clearArtifacts: () => void
  persistSettings: (settings: AppSettings) => Promise<void>
  onResetData: () => void
  inputPath: string
  setInputPath: Dispatch<SetStateAction<string>>
  outputPath: string
  setOutputPath: Dispatch<SetStateAction<string>>
  selection: CsvSelectionState
}

export function useCsvAnalysis({
  settings,
  settingsLoaded,
  busy,
  setBusy,
  setError,
  setResult,
  setPrivacyConfig,
  clearArtifacts,
  persistSettings,
  onResetData,
  inputPath,
  setInputPath,
  outputPath,
  setOutputPath,
  selection,
}: CsvAnalysisOptions) {
  async function handlePickInput() {
    if (busy !== 'idle' || !settingsLoaded) return

    setError(null)
    setBusy('picking')
    try {
      const picked = await pickInputCsv(settings.rememberLastPaths ? settings.lastInputDirectory : null)
      if (picked) {
        await loadCsv(picked)
      }
    } catch (caught) {
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function loadCsv(path = inputPath) {
    if (!settingsLoaded) return

    const normalized = path.trim()
    if (!normalized) {
      setError('Select or enter a CSV file path first.')
      return
    }

    setBusy('loading')
    setError(null)
    clearArtifacts()
    selection.setColumnControls({})
    setPrivacyConfig(privacyConfigFromSettings(settings))

    try {
      const response = await analyzeCsv(normalized, settings.sampleRowCount, settings.defaultOutputSuffix)
      setInputPath(response.headers.filePath)
      selection.setLoadedCsv(response.headers, response.selectedColumns)
      setOutputPath(response.suggestedOutputPath)

      if (settings.rememberLastPaths) {
        void persistSettings({
          ...settings,
          lastInputDirectory: directoryOf(response.headers.filePath),
          lastOutputDirectory: directoryOf(response.suggestedOutputPath),
        })
      }

      if (!response.headers.rowCountIsComplete) {
        void refreshExactRowCount(response.headers.filePath)
      }
    } catch (caught) {
      onResetData()
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handlePickOutput() {
    if (!selection.headers || busy !== 'idle' || !settingsLoaded) return

    setError(null)
    setBusy('picking')
    try {
      const picked = await pickOutputCsv(
        outputPath || (settings.rememberLastPaths ? settings.lastOutputDirectory : null),
      )
      if (picked) {
        setOutputPath(picked)
        setResult(null)
        if (settings.rememberLastPaths) {
          void persistSettings({ ...settings, lastOutputDirectory: directoryOf(picked) })
        }
      }
    } catch (caught) {
      setError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function refreshExactRowCount(path: string) {
    try {
      const rowCount = await countCsvRows(path)
      selection.setHeaders((current) =>
        current?.filePath === path ? { ...current, rowCount, rowCountIsComplete: true } : current,
      )
    } catch {
      selection.setHeaders((current) =>
        current?.filePath === path ? { ...current, rowCountIsComplete: false } : current,
      )
    }
  }

  function updateOutputPath(value: string) {
    setOutputPath(value)
    setResult(null)
  }

  function updateOutputPathSuffix(suffix: string) {
    if (!selection.headers) return
    setOutputPath(defaultOutputPathWithSuffix(selection.headers.filePath, suffix))
    setResult(null)
  }

  function clearFile() {
    setInputPath('')
    setOutputPath('')
    onResetData()
    setError(null)
  }

  function handleInputChange(value: string) {
    setInputPath(value)
    if (selection.headers && value.trim() !== selection.headers.filePath) {
      onResetData()
    }
  }

  function maybeLoadManualPath() {
    const normalized = inputPath.trim()
    if (settingsLoaded && busy === 'idle' && normalized && normalized !== selection.headers?.filePath) {
      void loadCsv(normalized)
    }
  }

  return {
    handlePickInput,
    loadCsv,
    handlePickOutput,
    refreshExactRowCount,
    updateOutputPath,
    updateOutputPathSuffix,
    clearFile,
    handleInputChange,
    maybeLoadManualPath,
  }
}
