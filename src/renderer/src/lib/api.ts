import type {
  ActionData,
  AnonymizeData,
  AnonymizeParams,
  ApiErrorDetails,
  ApiFailure,
  ApiResult,
  AppSettings,
  AppSettingsPatch,
  ColumnInfo,
  ColumnPreview,
  FileDialogData,
  GetHeadersParams,
  GetPreviewParams,
  HeadersData,
  HealthData,
  OutputPathDialogParams,
  PreviewData,
  ShowItemParams
} from '@shared/contracts'

export type {
  ActionData,
  AnonymizeData,
  AnonymizeParams,
  ApiFailure as ApiError,
  ApiResult,
  AppSettings,
  AppSettingsPatch,
  ColumnInfo,
  ColumnPreview,
  FileDialogData,
  GetHeadersParams,
  GetPreviewParams,
  HeadersData,
  HealthData,
  PreviewData
}

export type PiiRisk = ColumnInfo['piiRisk']
export type Confidence = ColumnInfo['confidence']
export type DataType = ColumnInfo['detectedType']

export function isApiError(response: ApiResult<unknown>): response is ApiFailure {
  return response.success === false
}

const ERROR_MESSAGES = {
  FILE_NOT_FOUND: 'File not found. Please select a valid CSV file.',
  CSV_PARSE_ERROR: 'Unable to parse CSV. Check file format and encoding.',
  CONFIG_INVALID: 'Invalid settings. Please check your configuration.',
  COLUMN_NOT_FOUND: 'Column selection is out of range for this CSV file.',
  OUTPUT_EXISTS: 'Output file already exists. Disable overwrite or choose a different output path.',
  INVALID_SELECTION: 'Column selection is invalid.',
  BRIDGE_UNAVAILABLE: 'Desktop bridge is unavailable. Restart the application.',
  UNKNOWN: 'An unexpected error occurred. Please try again.'
} as const

const DEFAULT_ERROR_MESSAGE = 'An unexpected error occurred. Please try again.'

export function getErrorMessage(error: ApiFailure): string {
  const code = error.error.code as keyof typeof ERROR_MESSAGES

  if (code in ERROR_MESSAGES) {
    return ERROR_MESSAGES[code]
  }

  if (error.error.message) {
    return error.error.message
  }

  return DEFAULT_ERROR_MESSAGE
}

function bridge() {
  if (!window.csvAnonymizer) {
    throw new Error('Desktop bridge is unavailable')
  }
  return window.csvAnonymizer
}

async function invoke<T>(operation: () => Promise<ApiResult<T>>): Promise<ApiResult<T>> {
  try {
    return await operation()
  } catch (error) {
    return {
      success: false,
      error: toApiError(error)
    }
  }
}

function toApiError(error: unknown): ApiErrorDetails {
  return {
    code: error instanceof Error && error.message.includes('Desktop bridge') ? 'BRIDGE_UNAVAILABLE' : 'UNKNOWN',
    message: error instanceof Error ? error.message : DEFAULT_ERROR_MESSAGE
  }
}

export async function getHealth(): Promise<ApiResult<HealthData>> {
  return invoke(() => bridge().getHealth())
}

export async function getSettings(): Promise<ApiResult<AppSettings>> {
  return invoke(() => bridge().getSettings())
}

export async function updateSettings(input: AppSettingsPatch): Promise<ApiResult<AppSettings>> {
  return invoke(() => bridge().updateSettings(input))
}

export async function selectCsvFile(): Promise<ApiResult<FileDialogData>> {
  return invoke(() => bridge().selectCsvFile())
}

export async function selectOutputFile(input?: OutputPathDialogParams): Promise<ApiResult<FileDialogData>> {
  return invoke(() => bridge().selectOutputFile(input))
}

export async function showOutputInFolder(input: ShowItemParams): Promise<ApiResult<ActionData>> {
  return invoke(() => bridge().showOutputInFolder(input))
}

export async function getHeaders(params: GetHeadersParams): Promise<ApiResult<HeadersData>> {
  return invoke(() => bridge().getHeaders(params))
}

export async function getPreview(params: GetPreviewParams): Promise<ApiResult<PreviewData>> {
  return invoke(() => bridge().getPreview(params))
}

export async function anonymizeFile(params: AnonymizeParams): Promise<ApiResult<AnonymizeData>> {
  return invoke(() => bridge().anonymizeFile(params))
}
