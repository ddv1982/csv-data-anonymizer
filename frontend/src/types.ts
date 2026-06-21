export type DataType =
  | 'email'
  | 'uuid'
  | 'timestamp'
  | 'numericId'
  | 'numericValue'
  | 'postalCode'
  | 'address'
  | 'ipAddress'
  | 'url'
  | 'macAddress'
  | 'taxId'
  | 'boolean'
  | 'currency'
  | 'percentage'
  | 'countryCode'
  | 'phone'
  | 'firstName'
  | 'lastName'
  | 'fullName'
  | 'enum'
  | 'string'
  | 'unknown'

export type Confidence = 'high' | 'medium' | 'low'
export type PiiRisk = 'high' | 'medium' | 'low'
export type EmptyFormat = 'emptyString' | 'null' | 'mixed'
export type AnonymizationStrategy = 'auto' | 'pseudonymize' | 'tokenize' | 'localAi' | 'mask' | 'passThrough'

export interface ColumnControl {
  columnIndex: number
  typeOverride: DataType | null
  strategy: AnonymizationStrategy
}

export interface AppSettings {
  schemaVersion: number
  deterministicDefault: boolean
  seed: string
  overwriteOutput: boolean
  sampleRowCount: number
  previewSampleCount: number
  defaultOutputSuffix: string
  rememberLastPaths: boolean
  lastInputDirectory: string | null
  lastOutputDirectory: string | null
  localAiEnabled: boolean
  localAiModel: string
}

export interface ColumnMetadata {
  name: string
  index: number
  detectedType: DataType
  confidence: Confidence
  piiRisk: PiiRisk
  sampleValues: string[]
  emptyFormat: EmptyFormat
  isSelected: boolean
  strategy: AnonymizationStrategy
}

export interface HeadersData {
  filePath: string
  rowCount: number
  rowCountIsComplete: boolean
  defaultOutputPath: string
  columns: ColumnMetadata[]
}

export interface AnalyzeResponse {
  headers: HeadersData
  selectedColumns: number[]
  suggestedOutputPath: string
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

export type WarningSeverity = 'info' | 'warning'

export interface PreviewWarning {
  columnIndex: number
  columnName: string
  message: string
  severity: WarningSeverity
}

export interface PreviewData {
  previews: ColumnPreview[]
  warnings: PreviewWarning[]
}

export interface AnonymizeData {
  outputPath: string
  rowCount: number
  columnsAnonymized: number
  durationMs: number
  privacyReport: PrivacyReport
}

export interface PrivacyReport {
  directIdentifiers: number
  quasiIdentifiers: number
  pseudonymizedColumns: number
  smartReplacementColumns: number
  opaqueTokenColumns: number
  maskedColumns: number
  generalizedColumns: number
  passThroughColumns: number
  uniquePseudonymValues: number
  reusedPseudonymValues: number
  collisionsAvoided: number
  exhaustedPseudonymPools: number
  opaqueTokenValues: number
  smartReplacementValues: number
  smartReplacementFallbacks: number
  notes: string[]
}

export type AnonymizeJobState = 'running' | 'succeeded' | 'failed' | 'canceled'

export interface AnonymizeJobStatus {
  jobId: string
  state: AnonymizeJobState
  rowsProcessed: number
  totalRows: number | null
  cancelRequested: boolean
  result: AnonymizeData | null
  error: string | null
}

export interface LocalAiRequest {
  enabled: boolean
  model: string
}

export interface LocalAiStatus {
  enabled: boolean
  provider: string
  model: string
  availableModels: string[]
  endpoint: string
  runtimeAvailable: boolean
  modelInstalled: boolean
  ready: boolean
  runtimeVersion: string | null
  message: string
}

export type LocalAiDownloadState = 'running' | 'succeeded' | 'failed' | 'canceled'

export interface LocalAiDownloadStatus {
  jobId: string
  state: LocalAiDownloadState
  model: string
  statusMessage: string
  completedBytes: number | null
  totalBytes: number | null
  cancelRequested: boolean
  error: string | null
}
