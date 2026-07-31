import type { Dispatch, SetStateAction } from 'react'
import { analyzeCsv, countCsvRows, pickInputCsv, pickOutputCsv } from '../tauri'
import type {
  AnalyzeResponse,
  AppSettings,
  ColumnControl,
  PreparedAnalysis,
} from '../types'
import { messageFrom } from '../utils/errors'
import { defaultOutputPathWithSuffix, directoryOf } from '../utils/paths'
import type { WorkflowShell } from './workflowTypes'

type HeadersState = AnalyzeResponse['headers'] | null

type CsvSelectionState = {
  headers: HeadersState
  setHeaders: Dispatch<SetStateAction<HeadersState>>
  setLoadedCsv: (headers: AnalyzeResponse['headers'], selectedColumns: number[]) => void
  setColumnControls: Dispatch<SetStateAction<Record<number, ColumnControl>>>
}

type CsvAnalysisArgs = {
  settingsLoaded: boolean
  clearArtifacts: () => void
  persistSettings: (settings: AppSettings) => Promise<void>
  onResetData: () => void
  inputPath: string
  setInputPath: Dispatch<SetStateAction<string>>
  outputPath: string
  setOutputPath: Dispatch<SetStateAction<string>>
  selection: CsvSelectionState
  setPreparedAnalysis: Dispatch<SetStateAction<PreparedAnalysis | null>>
}

export function useCsvAnalysis(
  shell: WorkflowShell,
  {
    settingsLoaded,
    clearArtifacts,
    persistSettings,
    onResetData,
    inputPath,
    setInputPath,
    outputPath,
    setOutputPath,
    selection,
    setPreparedAnalysis,
  }: CsvAnalysisArgs,
) {
  const { busy, setBusy, setError, setResult, settings } = shell

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

    try {
      const response = await analyzeCsv(
        normalized,
        settings.sampleRowCount,
        settings.defaultOutputSuffix,
      )
      setPreparedAnalysis(response.preparedAnalysis ?? null)
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
      // Intentional: keep the approximate count from analysis and only mark it incomplete.
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
    handlePickOutput,
    updateOutputPath,
    updateOutputPathSuffix,
    clearFile,
    handleInputChange,
    maybeLoadManualPath,
  }
}
