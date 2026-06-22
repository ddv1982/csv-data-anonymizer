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
export type ReleaseMode = 'standard' | 'formalTabular' | 'differentialPrivacyAggregate' | 'syntheticData'
export type ColumnRole = 'auto' | 'directIdentifier' | 'quasiIdentifier' | 'sensitive' | 'attribute' | 'exclude'
export type DpAggregate = 'count' | 'sum' | 'mean'
export type PrivacyModel = 'kAnonymity' | 'lDiversity' | 'tCloseness' | 'differentialPrivacy' | 'syntheticData'

export interface ColumnControl {
  columnIndex: number
  typeOverride: DataType | null
  strategy: AnonymizationStrategy
}

export interface PrivacyColumnRole {
  columnIndex: number
  role: ColumnRole
  generalizationLevel: number
}

export interface FormalPrivacyConfig {
  k: number
  lDiversity: number | null
  tCloseness: number | null
  suppressSmallClasses: boolean
}

export interface DifferentialPrivacyConfig {
  epsilon: number
  aggregate: DpAggregate
  groupByColumn: number | null
  valueColumn: number | null
  lowerBound: number | null
  upperBound: number | null
}

export interface SyntheticDataConfig {
  rowCount: number | null
  epsilon: number | null
}

export interface PrivacyConfig {
  releaseMode: ReleaseMode
  columnRoles: PrivacyColumnRole[]
  formal: FormalPrivacyConfig
  differentialPrivacy: DifferentialPrivacyConfig
  synthetic: SyntheticDataConfig
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

export interface SmartReplacementEntry {
  columnIndex: number
  original: string
  replacement: string
}

export interface PreviewData {
  previews: ColumnPreview[]
  warnings: PreviewWarning[]
  smartReplacements: SmartReplacementEntry[]
}

export interface AnonymizeData {
  outputPath: string
  rowCount: number
  columnsAnonymized: number
  durationMs: number
  privacyReport: PrivacyReport
}

export interface PrivacyReport {
  releaseMode: ReleaseMode
  directIdentifiers: number
  quasiIdentifiers: number
  sensitiveColumns: number
  pseudonymizedColumns: number
  smartReplacementColumns: number
  opaqueTokenColumns: number
  maskedColumns: number
  generalizedColumns: number
  passThroughColumns: number
  suppressedRows: number
  syntheticRows: number
  dpEpsilon: string | null
  uniquePseudonymValues: number
  reusedPseudonymValues: number
  collisionsAvoided: number
  exhaustedPseudonymPools: number
  opaqueTokenValues: number
  smartReplacementValues: number
  smartReplacementFallbacks: number
  formalModels: PrivacyModelReport[]
  notes: string[]
}

export interface PrivacyModelReport {
  model: PrivacyModel
  satisfied: boolean
  actual: string
  threshold: string
  message: string
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
