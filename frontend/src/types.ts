export type DataType =
  | 'email'
  | 'uuid'
  | 'timestamp'
  | 'numericId'
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

export interface PreviewData {
  previews: ColumnPreview[]
}

export interface AnonymizeData {
  outputPath: string
  rowCount: number
  columnsAnonymized: number
  durationMs: number
}
