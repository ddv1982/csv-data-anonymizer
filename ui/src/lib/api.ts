// API Client for CSV Anonymizer backend

// Types matching API contracts

export type PiiRisk = 'high' | 'medium' | 'low'
export type Confidence = 'high' | 'medium' | 'low'
export type DataType =
  | 'email'
  | 'phone'
  | 'uuid'
  | 'numeric_id'
  | 'date'
  | 'timestamp'
  | 'string'
  | 'unknown'

export interface ColumnInfo {
  index: number
  name: string
  detectedType: DataType
  confidence: Confidence
  piiRisk: PiiRisk
  sampleValues: string[]
}

export interface HeadersResponse {
  success: true
  data: {
    filePath: string
    rowCount: number
    columns: ColumnInfo[]
  }
}

export interface SampleTransform {
  original: string
  anonymized: string
}

export interface ColumnPreview {
  columnIndex: number
  columnName: string
  samples: SampleTransform[]
}

export interface PreviewResponse {
  success: true
  data: {
    previews: ColumnPreview[]
  }
}

export interface AnonymizeResponse {
  success: true
  data: {
    outputPath: string
    rowCount: number
    columnsAnonymized: number
    duration: number
  }
}

export interface HealthResponse {
  status: 'ok'
  version: string
  timestamp: string
}

export interface ApiError {
  success: false
  error: {
    code: string
    message: string
    suggestion?: string
  }
}

export type ApiResult<T> = T | ApiError

// Check if response is an error
export function isApiError(response: ApiResult<unknown>): response is ApiError {
  return (response as ApiError).success === false
}

/**
 * Error code to user-friendly message mapping
 */
const ERROR_MESSAGES = {
  FILE_NOT_FOUND: 'File not found. Please select a valid CSV file.',
  INVALID_CSV: 'Unable to parse CSV. Check file format and encoding.',
  CONFIG_INVALID: 'Invalid configuration. Please check your settings.',
  PATH_TRAVERSAL: 'Invalid file path. Path traversal is not allowed.',
  OUTPUT_EXISTS: 'Output file already exists. Enable "force" to overwrite.',
  NETWORK_ERROR: 'Network error. Please check your connection and try again.',
  SERVER_ERROR: 'Server error. Please try again or use the CLI.',
  UNKNOWN: 'An unexpected error occurred. Please try again.',
} as const

const DEFAULT_ERROR_MESSAGE = 'An unexpected error occurred. Please try again.'

/**
 * Get a user-friendly error message from an API error response
 */
export function getErrorMessage(error: ApiError): string {
  const code = error.error.code as keyof typeof ERROR_MESSAGES

  // Check if we have a mapped message
  if (code in ERROR_MESSAGES) {
    return ERROR_MESSAGES[code]
  }

  // Use the server-provided message if available
  if (error.error.message) {
    return error.error.message
  }

  // Add suggestion if available
  const suggestion = error.error.suggestion
  if (suggestion) {
    return `${DEFAULT_ERROR_MESSAGE} ${suggestion}`
  }

  return DEFAULT_ERROR_MESSAGE
}

// Base fetch function with error handling
async function apiFetch<T>(
  endpoint: string,
  options?: RequestInit
): Promise<ApiResult<T>> {
  try {
    const response = await fetch(endpoint, {
      ...options,
      headers: {
        'Content-Type': 'application/json',
        ...options?.headers,
      },
    })

    const data = await response.json() as T | ApiError

    return data
  } catch (error) {
    return {
      success: false,
      error: {
        code: 'NETWORK_ERROR',
        message: error instanceof Error ? error.message : 'Network request failed',
        suggestion: 'Check your network connection and try again.',
      },
    }
  }
}

// API Functions

export async function getHealth(): Promise<ApiResult<HealthResponse>> {
  return apiFetch<HealthResponse>('/api/health')
}

export interface GetHeadersParams {
  filePath: string
}

export async function getHeaders(
  params: GetHeadersParams
): Promise<ApiResult<HeadersResponse>> {
  return apiFetch<HeadersResponse>('/api/headers', {
    method: 'POST',
    body: JSON.stringify(params),
  })
}

export interface GetPreviewParams {
  filePath: string
  columns: number[]
  deterministic?: boolean
  seed?: string | null
  sampleCount?: number
}

export async function getPreview(
  params: GetPreviewParams
): Promise<ApiResult<PreviewResponse>> {
  return apiFetch<PreviewResponse>('/api/preview', {
    method: 'POST',
    body: JSON.stringify(params),
  })
}

export interface AnonymizeParams {
  filePath: string
  outputPath: string
  columns: number[]
  deterministic?: boolean
  seed?: string
  force?: boolean
}

export async function anonymizeFile(
  params: AnonymizeParams
): Promise<ApiResult<AnonymizeResponse>> {
  return apiFetch<AnonymizeResponse>('/api/anonymize', {
    method: 'POST',
    body: JSON.stringify(params),
  })
}
