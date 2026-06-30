import { setTheme as setTauriTheme } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import { defaultSettings } from './defaults'
import type {
  AnalyzeResponse,
  AnonymizeJobStatus,
  AppSettings,
  ColumnControl,
  DataType,
  LocalAiDownloadStatus,
  LocalAiRequest,
  LocalAiStatus,
  PasteAnalyzeData,
  PasteDataFormat,
  PasteTransformData,
  PreflightData,
  PreflightMode,
  PreflightParams,
  PreviewData,
  QuickTransformData,
  SmartReplacementEntry,
} from './types'

type TauriTheme = 'light' | 'dark'
type TestInvoke = (command: string, args?: Record<string, unknown>) => unknown
type PreflightCommandRequest = Omit<PreflightParams, 'localAiReady' | 'localAiMessage'> & {
  localAi: LocalAiRequest
}

declare global {
  interface Window {
    __CSV_ANONYMIZER_TEST_INVOKE__?: TestInvoke
    __TAURI_INTERNALS__?: unknown
  }
}

export function loadSettings(): Promise<AppSettings> {
  return invokeCommand('load_settings')
}

export function saveSettings(settings: AppSettings): Promise<AppSettings> {
  return invokeCommand('save_settings', { settings })
}

export function pickInputCsv(initialDirectory: string | null): Promise<string | null> {
  return invokeCommand('pick_input_csv', { initialDirectory })
}

export function pickOutputCsv(suggestedOutputPath: string | null): Promise<string | null> {
  return invokeCommand('pick_output_csv', { suggestedOutputPath })
}

export function analyzeCsv(
  filePath: string,
  sampleRowCount: number,
  outputSuffix: string,
): Promise<AnalyzeResponse> {
  return invokeCommand('analyze_csv', { filePath, sampleRowCount, outputSuffix })
}

export function countCsvRows(filePath: string): Promise<number> {
  return invokeCommand('count_csv_rows', { filePath })
}

export function analyzePasteData(
  content: string,
  format: PasteDataFormat,
  sampleRowCount: number,
): Promise<PasteAnalyzeData> {
  return invokeCommand('analyze_pasted_data', {
    request: {
      content,
      format,
      sampleRowCount,
    },
  })
}

export function previewPasteData(
  content: string,
  format: PasteDataFormat,
  columns: number[],
  controls: ColumnControl[],
  deterministic: boolean,
  seed: string,
  sampleCount: number,
  localAi: LocalAiRequest,
): Promise<PreviewData> {
  return invokeCommand('preview_pasted_data', {
    request: {
      content,
      format,
      columns,
      controls,
      deterministic,
      seed,
      sampleCount,
      localAi,
    },
  })
}

export function transformPasteData(
  content: string,
  format: PasteDataFormat,
  columns: number[],
  controls: ColumnControl[],
  deterministic: boolean,
  seed: string,
  previewSmartReplacements: SmartReplacementEntry[],
  localAi: LocalAiRequest,
): Promise<PasteTransformData> {
  return invokeCommand('anonymize_pasted_data', {
    request: {
      content,
      format,
      columns,
      controls,
      deterministic,
      seed,
      previewSmartReplacements,
      localAi,
    },
  })
}

export function generateQuickValues(
  dataType: DataType,
  strategy: ColumnControl['strategy'],
  count: number,
  deterministic: boolean,
  seed: string,
  localAi: LocalAiRequest,
): Promise<QuickTransformData> {
  return invokeCommand('generate_quick_values', {
    request: {
      dataType,
      strategy,
      count,
      deterministic,
      seed,
      localAi,
    },
  })
}

export function previewAnonymization(
  filePath: string,
  columns: number[],
  controls: ColumnControl[],
  deterministic: boolean,
  seed: string,
  sampleCount: number,
  localAi: LocalAiRequest,
): Promise<PreviewData> {
  return invokeCommand('preview_anonymization', {
    request: {
      filePath,
      columns,
      controls,
      deterministic,
      seed,
      sampleCount,
      localAi,
    },
  })
}

export function preflightAnonymization(
  mode: PreflightMode,
  filePath: string,
  outputPath: string | null,
  columns: number[],
  controls: ColumnControl[],
  deterministic: boolean,
  seed: string,
  force: boolean,
  sampleRowCount: number,
  previewSmartReplacements: SmartReplacementEntry[],
  localAi: LocalAiRequest,
): Promise<PreflightData> {
  const request: PreflightCommandRequest = {
    mode,
    filePath,
    outputPath,
    columns,
    controls,
    deterministic,
    seed,
    force,
    sampleRowCount,
    previewSmartReplacements,
    localAi,
  }

  return invokeCommand('preflight_anonymization', {
    request,
  })
}

export function firstPreflightBlocker(preflight: PreflightData): string | null {
  return preflight.readiness.blockers[0] ?? null
}

export function startAnonymizeJob(
  filePath: string,
  outputPath: string,
  columns: number[],
  controls: ColumnControl[],
  deterministic: boolean,
  seed: string,
  force: boolean,
  sampleRowCount: number,
  totalRowCount: number | null,
  previewSmartReplacements: SmartReplacementEntry[],
  localAi: LocalAiRequest,
): Promise<AnonymizeJobStatus> {
  return invokeCommand('start_anonymize_job', {
    request: {
      filePath,
      outputPath,
      columns,
      controls,
      deterministic,
      seed,
      force,
      sampleRowCount,
      totalRowCount,
      previewSmartReplacements,
      localAi,
    },
  })
}

export function getAnonymizeJobStatus(jobId: string): Promise<AnonymizeJobStatus> {
  return invokeCommand('get_anonymize_job_status', { jobId })
}

export function cancelAnonymizeJob(jobId: string): Promise<AnonymizeJobStatus> {
  return invokeCommand('cancel_anonymize_job', { jobId })
}

export function openOutputLocation(outputPath: string): Promise<void> {
  return invokeCommand('open_output_location', { outputPath })
}

export function getLocalAiStatus(request: LocalAiRequest): Promise<LocalAiStatus> {
  return invokeCommand('get_local_ai_status', { request })
}

export function startLocalAiModelDownload(request: LocalAiRequest): Promise<LocalAiDownloadStatus> {
  return invokeCommand('start_local_ai_model_download', { request })
}

export function getLocalAiModelDownloadStatus(jobId: string): Promise<LocalAiDownloadStatus> {
  return invokeCommand('get_local_ai_model_download_status', { jobId })
}

export function cancelLocalAiModelDownload(jobId: string): Promise<LocalAiDownloadStatus> {
  return invokeCommand('cancel_local_ai_model_download', { jobId })
}

export function openLocalAiSetupUrl(): Promise<void> {
  return invokeCommand('open_local_ai_setup_url')
}

export async function setAppTheme(theme: TauriTheme | null): Promise<void> {
  try {
    await setTauriTheme(theme)
  } catch {
    // Browser/Vite contexts do not provide the Tauri app plugin.
  }
}

function invokeCommand<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (typeof window !== 'undefined' && window.__CSV_ANONYMIZER_TEST_INVOKE__) {
    return Promise.resolve(window.__CSV_ANONYMIZER_TEST_INVOKE__(command, args) as T)
  }
  if (!isTauriRuntime()) {
    const fallback = browserPreviewFallback(command, args)
    if (fallback.handled) return Promise.resolve(fallback.value as T)
    return Promise.reject(new Error('This action requires the Tauri desktop app.'))
  }
  return invoke<T>(command, args)
}

function isTauriRuntime() {
  return typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__)
}

function browserPreviewFallback(command: string, args?: Record<string, unknown>) {
  if (command === 'load_settings') {
    return { handled: true, value: defaultSettings }
  }
  if (command === 'save_settings') {
    return { handled: true, value: args?.settings ?? defaultSettings }
  }
  if (command === 'get_local_ai_status') {
    const request = args?.request as LocalAiRequest | undefined
    return {
      handled: true,
      value: {
        enabled: Boolean(request?.enabled),
        provider: 'ollama',
        model: request?.model ?? defaultSettings.localAiModel,
        availableModels: [],
        endpoint: 'http://127.0.0.1:11434',
        runtimeAvailable: false,
        modelInstalled: false,
        ready: false,
        runtimeVersion: null,
        message: 'Local AI is available in the desktop app.',
      } satisfies LocalAiStatus,
    }
  }
  if (command === 'preflight_anonymization') {
    const request = args?.request as { mode?: PreflightMode; columns?: unknown[] } | undefined
    return {
      handled: true,
      value: {
        mode: request?.mode ?? 'anonymize',
        readiness: {
          status: 'verified',
          blockers: [],
          reviewItems: [],
          verifiedItems: [`${request?.columns?.length ?? 0} column(s) selected.`],
        },
        evidence: [],
        columnReports: [],
      } satisfies PreflightData,
    }
  }

  return { handled: false, value: null }
}
