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
export type PrivacyFindingKind =
  | 'person'
  | 'contact'
  | 'privateAddress'
  | 'privateDate'
  | 'accountOrFinancialId'
  | 'governmentId'
  | 'credentialOrSecret'
  | 'networkOrDeviceId'
  | 'url'
  | 'mixedSensitiveText'
export type EmptyFormat = 'emptyString' | 'null' | 'mixed'
export type AnonymizationStrategy =
  | 'auto'
  | 'pseudonymize'
  | 'tokenize'
  | 'localAi'
  | 'mask'
  | 'redact'
  | 'passThrough'
export type ThemeMode = 'system' | 'light' | 'dark'
export type ReleaseReadinessStatus = 'verified' | 'review' | 'blocked'
export type ReleaseEvidenceStatus = 'verified' | 'review' | 'blocked' | 'info'
export type SmartReplacementRejectionReason =
  | 'unexpectedOriginal'
  | 'missingOutput'
  | 'emptyOutput'
  | 'sameAsOriginal'
  | 'containsOriginal'
  | 'controlCharacter'
  | 'duplicateOriginal'
  | 'duplicateOutput'
export type PreflightMode = 'preview' | 'anonymize'

export interface ColumnControl {
  columnIndex: number
  typeOverride: DataType | null
  strategy: AnonymizationStrategy
}

export interface AppSettings {
  schemaVersion: number
  themeMode: ThemeMode
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
  sourcePath?: string | null
  index: number
  detectedType: DataType
  confidence: Confidence
  detectionTrace?: DetectionTrace | null
  privacyFindings?: PrivacyFinding[]
  privacyEvidence?: PrivacyEvidenceSummary[]
  piiRisk: PiiRisk
  sampleValues: string[]
  emptyFormat: EmptyFormat
  isSelected: boolean
  strategy: AnonymizationStrategy
}

export interface PrivacyFinding {
  kind: PrivacyFindingKind
  dataType: DataType
  rowIndex: number
  start: number
  end: number
  matchValue: string
  sampleValue: string
  confidence: Confidence
  score: number
  detector: string
  reason: string
}

export interface PrivacyEvidenceSummary {
  kind: PrivacyFindingKind
  dataType: DataType
  confidence: Confidence
  matchCount: number
  sampleCount: number
  score: number
  detector: string
  reason: string
  detectors?: string[]
}

export interface DetectionTrace {
  summary: string
  selectedReason: string
  totalNonEmpty: number
  candidates: DetectionTraceItem[]
}

export interface DetectionTraceItem {
  dataType: DataType
  reason: string
  matchCount: number
  totalConsidered: number
  confidence: Confidence
  accepted: boolean
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

export type PasteDataFormat = 'auto' | 'csv' | 'json' | 'xml' | 'yaml' | 'plainText' | 'logs'

export interface PasteAnalyzeData {
  format: PasteDataFormat
  rowCount: number
  rowCountIsComplete: boolean
  columns: ColumnMetadata[]
}

export interface PasteTransformData {
  output: string
  rowCount: number
  columnsAnonymized: number
  durationMs: number
  privacyReport: PrivacyReport
}

export interface QuickTransformData {
  output: string
  rowCount: number
  values: SampleTransform[]
  privacyReport: PrivacyReport
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

export interface SmartReplacementRejectionCount {
  reason: SmartReplacementRejectionReason
  count: number
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

export interface PreflightParams {
  mode: PreflightMode
  filePath: string
  outputPath?: string | null
  columns: number[]
  controls: ColumnControl[]
  force: boolean
  sampleRowCount: number
  previewSmartReplacements: SmartReplacementEntry[]
  localAiReady: boolean
  localAiMessage?: string | null
}

export interface PreflightData {
  mode: PreflightMode
  readiness: ReleaseReadiness
  evidence: ReleaseEvidenceItem[]
  columnReports: ColumnReleaseReport[]
}

export interface PrivacyReport {
  directIdentifiers: number
  quasiIdentifiers: number
  sensitiveColumns: number
  pseudonymizedColumns: number
  smartReplacementColumns: number
  opaqueTokenColumns: number
  maskedColumns: number
  redactedColumns: number
  passThroughColumns: number
  uniquePseudonymValues: number
  reusedPseudonymValues: number
  collisionsAvoided: number
  exhaustedPseudonymPools: number
  opaqueTokenValues: number
  smartReplacementValues: number
  smartReplacementRejections: number
  smartReplacementRejectionReasons: SmartReplacementRejectionCount[]
  smartReplacementFallbacks: number
  shapeFallbackValues: number
  readiness: ReleaseReadiness
  evidence: ReleaseEvidenceItem[]
  columnReports: ColumnReleaseReport[]
  utilityMetrics: UtilityMetric[]
  notes: string[]
}

export interface ReleaseReadiness {
  status: ReleaseReadinessStatus
  blockers: string[]
  reviewItems: string[]
  verifiedItems: string[]
}

export interface ReleaseEvidenceItem {
  id: string
  label: string
  status: ReleaseEvidenceStatus
  detail: string
}

export interface ColumnReleaseReport {
  columnIndex: number
  columnName: string
  selected: boolean
  detectedType: DataType
  piiRisk: PiiRisk
  strategy: AnonymizationStrategy
  action: string
  status: ReleaseEvidenceStatus
  detail: string
}

export interface UtilityMetric {
  label: string
  value: string
  status: ReleaseEvidenceStatus
  detail?: string | null
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
