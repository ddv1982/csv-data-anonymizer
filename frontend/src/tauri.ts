import { setTheme as setTauriTheme } from '@tauri-apps/api/app'
import { Channel, invoke } from '@tauri-apps/api/core'
import { defaultSettings } from './defaults'
import { validateAnalyzeResponse, validatePasteAnalyzeData } from './utils/analysisContract'
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
  PastePreviewParams,
  PasteTransformData,
  PasteTransformParams,
  PreflightData,
  PreflightMode,
  PreflightParams,
  PreviewData,
  PreviewParams,
  PreparedAnalysis,
  QuickTransformData,
  SmartReplacementEntry,
} from './types'

type TauriTheme = 'light' | 'dark'
type TestInvoke = (command: string, args?: Record<string, unknown>) => unknown
// Every command request is its params struct plus the Local AI consent the shell
// resolves, so `invokeCommand`'s untyped payload cannot drift from the Rust struct
// unnoticed: the params interfaces are compared against it by
// `scripts/check-contracts.mjs`.
type WithLocalAi<Params> = Params & { localAi: LocalAiRequest }
type WithPreparedAnalysis<Params> = Params & { preparedAnalysis?: PreparedAnalysis | null }
type WithTokenizationKey<Params> = Params & { tokenizationKey?: string | null }
export type AnalyzeCsvRequest = {
  filePath: string
  sampleRowCount: number
  outputSuffix: string
}
export type AnalyzePasteDataRequest = {
  content: string
  format: PasteDataFormat
  sampleRowCount: number
}
export type PreflightCommandRequest = WithPreparedAnalysis<WithLocalAi<
  Omit<PreflightParams, 'localAiReady' | 'localAiMessage'>
>>
export type PreviewCommandRequest = WithTokenizationKey<WithPreparedAnalysis<WithLocalAi<PreviewParams>>>
export type PastePreviewCommandRequest = WithTokenizationKey<WithPreparedAnalysis<WithLocalAi<PastePreviewParams>>>
export type PasteTransformCommandRequest = WithTokenizationKey<WithPreparedAnalysis<WithLocalAi<PasteTransformParams>>>
export type QuickGenerateCommandRequest = WithTokenizationKey<{
  dataType: DataType
  strategy: ColumnControl['strategy']
  count: number
  localAi: LocalAiRequest
}>
export type StartAnonymizeJobRequest = WithTokenizationKey<WithPreparedAnalysis<WithLocalAi<{
  filePath: string
  outputPath: string
  columns: number[]
  controls: ColumnControl[]
  force: boolean
  sampleRowCount: number
  totalRowCount: number | null
  previewSmartReplacements: SmartReplacementEntry[]
}>>>
export type StartAnonymizeJobCall = {
  request: StartAnonymizeJobRequest
  onProgress: (status: AnonymizeJobStatus) => void
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

export function analyzeCsv(request: AnalyzeCsvRequest): Promise<AnalyzeResponse> {
  return invokeCommand<AnalyzeResponse>('analyze_csv', request)
    .then(validateAnalyzeResponse)
}

export function countCsvRows(filePath: string): Promise<number> {
  return invokeCommand('count_csv_rows', { filePath })
}

export function analyzePasteData(request: AnalyzePasteDataRequest): Promise<PasteAnalyzeData> {
  return invokeCommand<PasteAnalyzeData>('analyze_pasted_data', {
    request,
  }).then(validatePasteAnalyzeData)
}

export function previewPasteData(request: PastePreviewCommandRequest): Promise<PreviewData> {
  return invokeCommand('preview_pasted_data', { request })
}

export function transformPasteData(request: PasteTransformCommandRequest): Promise<PasteTransformData> {
  return invokeCommand('anonymize_pasted_data', { request })
}

export function generateQuickValues(request: QuickGenerateCommandRequest): Promise<QuickTransformData> {
  return invokeCommand('generate_quick_values', {
    request,
  })
}

export function previewAnonymization(request: PreviewCommandRequest): Promise<PreviewData> {
  return invokeCommand('preview_anonymization', { request })
}

export function preflightAnonymization(request: PreflightCommandRequest): Promise<PreflightData> {
  return invokeCommand('preflight_anonymization', {
    request,
  })
}

export function firstPreflightBlocker(preflight: PreflightData): string | null {
  return preflight.readiness.blockers[0] ?? null
}

export function startAnonymizeJob({ request, onProgress }: StartAnonymizeJobCall): Promise<AnonymizeJobStatus> {
  // Constructing a Tauri Channel itself requires the native callback registry.
  // Browser previews and E2E use the invoke seam below and have no such registry.
  const progressChannel = isTauriRuntime() ? new Channel<AnonymizeJobStatus>(onProgress) : null
  return invokeCommand('start_anonymize_job', {
    request,
    onProgress: progressChannel,
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
