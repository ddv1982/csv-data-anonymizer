import { setTheme as setTauriTheme } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import type {
  AnalyzeResponse,
  AnonymizeJobStatus,
  AppSettings,
  ColumnControl,
  LocalAiDownloadStatus,
  LocalAiRequest,
  LocalAiStatus,
  PreviewData,
  PrivacyConfig,
  SmartReplacementEntry,
} from './types'

type TauriTheme = 'light' | 'dark'
type TestInvoke = (command: string, args?: Record<string, unknown>) => unknown

declare global {
  interface Window {
    __CSV_ANONYMIZER_TEST_INVOKE__?: TestInvoke
  }
}

export function loadSettings(): Promise<AppSettings> {
  return invokeCommand('load_settings')
}

export function saveSettings(settings: AppSettings): Promise<AppSettings> {
  return invokeCommand('save_settings', { settings })
}

export const DP_BUDGET_RESET_CONFIRMATION_PHRASE = 'RESET DP BUDGET'

export function resetDpBudgetLedger(confirmationPhrase: string): Promise<AppSettings> {
  return invokeCommand('reset_dp_budget_ledger', { confirmationPhrase })
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
  privacyConfig: PrivacyConfig | null,
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
      privacyConfig,
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
  return invoke<T>(command, args)
}
