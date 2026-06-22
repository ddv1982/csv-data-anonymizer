import { setTheme as setTauriTheme } from '@tauri-apps/api/app'
import { invoke } from '@tauri-apps/api/core'
import type {
  AnalyzeResponse,
  AnonymizeData,
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

export function loadSettings(): Promise<AppSettings> {
  return invoke('load_settings')
}

export function saveSettings(settings: AppSettings): Promise<AppSettings> {
  return invoke('save_settings', { settings })
}

export function resetDpBudgetLedger(): Promise<AppSettings> {
  return invoke('reset_dp_budget_ledger')
}

export function pickInputCsv(initialDirectory: string | null): Promise<string | null> {
  return invoke('pick_input_csv', { initialDirectory })
}

export function pickOutputCsv(suggestedOutputPath: string | null): Promise<string | null> {
  return invoke('pick_output_csv', { suggestedOutputPath })
}

export function analyzeCsv(
  filePath: string,
  sampleRowCount: number,
  outputSuffix: string,
): Promise<AnalyzeResponse> {
  return invoke('analyze_csv', { filePath, sampleRowCount, outputSuffix })
}

export function countCsvRows(filePath: string): Promise<number> {
  return invoke('count_csv_rows', { filePath })
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
  return invoke('preview_anonymization', {
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

export function anonymizeCsv(
  filePath: string,
  outputPath: string,
  columns: number[],
  controls: ColumnControl[],
  deterministic: boolean,
  seed: string,
  force: boolean,
  sampleRowCount: number,
  previewSmartReplacements: SmartReplacementEntry[],
  privacyConfig: PrivacyConfig | null,
  localAi: LocalAiRequest,
): Promise<AnonymizeData> {
  return invoke('anonymize_csv', {
    request: {
      filePath,
      outputPath,
      columns,
      controls,
      deterministic,
      seed,
      force,
      sampleRowCount,
      previewSmartReplacements,
      privacyConfig,
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
  return invoke('start_anonymize_job', {
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
  return invoke('get_anonymize_job_status', { jobId })
}

export function cancelAnonymizeJob(jobId: string): Promise<AnonymizeJobStatus> {
  return invoke('cancel_anonymize_job', { jobId })
}

export function openOutputLocation(outputPath: string): Promise<void> {
  return invoke('open_output_location', { outputPath })
}

export function getLocalAiStatus(request: LocalAiRequest): Promise<LocalAiStatus> {
  return invoke('get_local_ai_status', { request })
}

export function startLocalAiModelDownload(request: LocalAiRequest): Promise<LocalAiDownloadStatus> {
  return invoke('start_local_ai_model_download', { request })
}

export function getLocalAiModelDownloadStatus(jobId: string): Promise<LocalAiDownloadStatus> {
  return invoke('get_local_ai_model_download_status', { jobId })
}

export function cancelLocalAiModelDownload(jobId: string): Promise<LocalAiDownloadStatus> {
  return invoke('cancel_local_ai_model_download', { jobId })
}

export function openLocalAiSetupUrl(): Promise<void> {
  return invoke('open_local_ai_setup_url')
}

export async function setAppTheme(theme: TauriTheme | null): Promise<void> {
  try {
    await setTauriTheme(theme)
  } catch {
    // Browser/Vite contexts do not provide the Tauri app plugin.
  }
}
