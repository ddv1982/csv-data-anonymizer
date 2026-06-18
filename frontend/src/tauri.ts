import { invoke } from '@tauri-apps/api/core'
import type { AnalyzeResponse, AnonymizeData, AnonymizeJobStatus, AppSettings, PreviewData } from './types'

export function loadSettings(): Promise<AppSettings> {
  return invoke('load_settings')
}

export function saveSettings(settings: AppSettings): Promise<void> {
  return invoke('save_settings', { settings })
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
  deterministic: boolean,
  seed: string,
  sampleCount: number,
): Promise<PreviewData> {
  return invoke('preview_anonymization', {
    filePath,
    columns,
    deterministic,
    seed,
    sampleCount,
  })
}

export function anonymizeCsv(
  filePath: string,
  outputPath: string,
  columns: number[],
  deterministic: boolean,
  seed: string,
  force: boolean,
  sampleRowCount: number,
): Promise<AnonymizeData> {
  return invoke('anonymize_csv', {
    request: {
      filePath,
      outputPath,
      columns,
      deterministic,
      seed,
      force,
      sampleRowCount,
    },
  })
}

export function startAnonymizeJob(
  filePath: string,
  outputPath: string,
  columns: number[],
  deterministic: boolean,
  seed: string,
  force: boolean,
  sampleRowCount: number,
  totalRowCount: number | null,
): Promise<AnonymizeJobStatus> {
  return invoke('start_anonymize_job', {
    request: {
      filePath,
      outputPath,
      columns,
      deterministic,
      seed,
      force,
      sampleRowCount,
      totalRowCount,
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
