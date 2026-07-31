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
  | 'addressRegion'
  | 'privateDate'
  | 'accountOrFinancialId'
  | 'recordIdentifier'
  | 'governmentId'
  | 'credentialOrSecret'
  | 'networkOrDeviceId'
  | 'url'
  | 'mixedSensitiveText'
export type SemanticDecisionStatus = 'resolved' | 'uncertain' | 'conflicting'
export type SemanticSpecificity = 'generic' | 'specific'
export type RedactionPlaceholderSource = 'typed' | 'columnHeader' | 'generic'
export type FormatEvidenceBasis = 'detectionSample' | 'userOverride' | 'retainedPreviewValues'
export type EmptyFormat = 'emptyString' | 'null' | 'mixed'
export type AnonymizationStrategy =
  | 'auto'
  | 'pseudonymize'
  | 'tokenize'
  | 'localAi'
  | 'mask'
  | 'label'
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
  | 'matchesOtherOriginal'
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
  /** Optional on-device NER supplements, but never replaces, deterministic detection. */
  localNerEnabled: boolean
}

export type ColumnReviewReason =
  | 'detectorsDisagree'
  | 'localNerLowConfidence'
  | 'ambiguousContext'
  | 'insufficientSample'

export type DetectionReviewReason = 'detectorFailed' | 'candidateRejected'

export type LocalNerRunStatus =
  | 'disabled'
  | 'completed'
  | 'unavailable'
  | 'failed'
  | 'incomplete'

export interface DetectionRunSummary {
  deterministic: 'completed'
  localNer: LocalNerRunStatus
  detectorId?: string | null
  modelVersion?: string | null
  examinedCells: number
  totalEligibleCells: number
  skippedOversizedCells: number
  acceptedCandidates: number
  rejectedCandidates: number
  reviewReasons?: DetectionReviewReason[]
  message?: string | null
}

/**
 * An opaque, backend-issued description of the exact detection run.
 *
 * The frontend must retain and return this value unchanged. It deliberately does
 * not try to interpret detector evidence or recreate it from mutable UI state.
 */
export interface PreparedDetectorIdentity {
  status: LocalNerRunStatus
  detectorId?: string | null
  modelVersion?: string | null
}

export interface PreparedCandidateEvidence {
  id: string
  columnIndex: number
  rowIndex: number
  start: number
  end: number
  kind: PrivacyFindingKind
  dataType: DataType
  matchValue: string
  sampleValue: string
  detector: string
}

export interface PreparedAnalysisSnapshot {
  version: number
  sourceIdentity: string
  sourceFingerprint: string
  format: string
  sampleRowCount: number
  columns: ColumnMetadata[]
  detector: PreparedDetectorIdentity
  detectionRunSummary: DetectionRunSummary
  candidateEvidence: PreparedCandidateEvidence[]
  integrityChecksum: string
}

export type PreparedAnalysis = PreparedAnalysisSnapshot

/**
 * The backend-authoritative explanation of one column's detection and privacy
 * decision. The UI presents this profile; it must not derive a new decision
 * from individual evidence items.
 */
export interface ColumnEvidenceProfile {
  formatEvidence: {
    dataType: DataType
    confidence: Confidence
    matchCount: number
    sampleCount: number
    basis: FormatEvidenceBasis
    detectors: string[]
  }
  semanticDecision: {
    kind: PrivacyFindingKind | 'unknown'
    confidence: Confidence
    status: SemanticDecisionStatus
    specificity: SemanticSpecificity
    supportingEvidence: string[]
    conflictingEvidence: string[]
    reason: string
  }
  privacyDecision: {
    risk: PiiRisk
    recommendedStrategy: AnonymizationStrategy
    autoSelected: boolean
    reason: string
  }
  redactionDecision: {
    placeholder: string
    source: RedactionPlaceholderSource
    isTyped: boolean
    preservesEquality: boolean
    reason: string
  }
}

export interface ColumnMetadata {
  name: string
  /** Another column's header reduces to the same label, so labels carry the index. */
  headerLabelIsAmbiguous: boolean
  sourcePath?: string | null
  index: number
  detectedType: DataType
  confidence: Confidence
  detectionTrace?: DetectionTrace | null
  privacyFindings?: PrivacyFinding[]
  privacyEvidence?: PrivacyEvidenceSummary[]
  /** Backend-authoritative profile present on every supported analysis schema. */
  evidenceProfile: ColumnEvidenceProfile
  piiRisk: PiiRisk
  sampleValues: string[]
  /** Distribution of the detection sample, not of the whole input. */
  sampleValueDistribution: ColumnValueDistribution
  emptyFormat: EmptyFormat
  isSelected: boolean
  strategy: AnonymizationStrategy
  /** Present only when the combined detectors need a human decision. */
  reviewReasons?: ColumnReviewReason[]
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
  detectionRunSummary: DetectionRunSummary
}

export interface AnalyzeResponse {
  headers: HeadersData
  selectedColumns: number[]
  suggestedOutputPath: string
  preparedAnalysis?: PreparedAnalysis | null
}

export type PasteDataFormat = 'auto' | 'csv' | 'json' | 'xml' | 'yaml' | 'plainText' | 'logs'

export type DetectionCoverageUnit = 'rows' | 'values'

export interface DetectionCoverageSummary {
  examined: number
  total: number
  unit: DetectionCoverageUnit
  isPartial: boolean
}

export interface PasteAnalyzeData {
  format: PasteDataFormat
  rowCount: number
  rowCountIsComplete: boolean
  detectionCoverage: DetectionCoverageSummary
  columns: ColumnMetadata[]
  detectionRunSummary: DetectionRunSummary
  preparedAnalysis?: PreparedAnalysis | null
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

export interface PreviewParams {
  filePath: string
  columns: number[]
  controls: ColumnControl[]
  sampleCount: number
  sampleRowCount: number
}

export interface PastePreviewParams {
  content: string
  format: PasteDataFormat
  columns: number[]
  controls: ColumnControl[]
  sampleCount: number
  sampleRowCount: number
}

export interface PasteTransformParams {
  content: string
  format: PasteDataFormat
  columns: number[]
  controls: ColumnControl[]
  sampleRowCount: number
  previewSmartReplacements: SmartReplacementEntry[]
}

export interface PreflightData {
  mode: PreflightMode
  readiness: ReleaseReadiness
  evidence: ReleaseEvidenceItem[]
  columnReports: ColumnReleaseReport[]
}

export interface PrivacyReport {
  detectionRunSummary?: DetectionRunSummary | null
  directIdentifiers: number
  quasiIdentifiers: number
  pseudonymizedColumns: number
  smartReplacementColumns: number
  opaqueTokenColumns: number
  maskedColumns: number
  labelledColumns: number
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
  columnValueDistributions: ColumnValueDistribution[]
  rowUniqueness: RowUniquenessSummary | null
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

/**
 * What one column's consistent pseudonyms reveal about the values behind them.
 *
 * Few `distinctValues` over many `totalValues` means the mapping can be relabelled
 * by frequency; a `singletonValues` entry is a pseudonym covering exactly one row,
 * which singles that record out however opaque the token looks.
 */
export interface ColumnValueDistribution {
  columnIndex: number
  distinctValues: number
  totalValues: number
  singletonValues: number
  /** With `singletonValues`, what lets a sampled distribution estimate the whole column. */
  doubletonValues: number
  maxValueOccurrences: number
}

/** What survived a column that an outsider holding the original could match against. */
export type MatchedPart =
  | 'wholeValue'
  | 'emailDomain'
  | 'dateDecadeAndTime'
  | 'survivingFormat'
  | 'blankPattern'

/**
 * One column the joint measure read, and what it was matched on.
 *
 * The pairing is the point. Two flat lists of indices — value-carrying and format-only —
 * cannot express the middle case, and the middle case is the common one on a pseudonymized
 * file: a column whose domain or whose decade survived, but not its value.
 */
export interface MatchedColumn {
  columnIndex: number
  matchedOn: MatchedPart
  /**
   * Whether every measured row actually carried `matchedOn`, or only some of them did.
   *
   * `matchedOn` is fixed per column by its strategy and detected type; no cell value can
   * change it. Cells can still fail to carry it — one that does not fit its column's detected
   * shape is pseudonymized generically and projects to nothing — so a timestamp column where
   * one value in a hundred parses was still described as the decade and time of all hundred.
   * The counts already treat those rows as sharing nothing on that column, so this qualifies
   * the wording and not the arithmetic.
   */
  matchedEveryRow: boolean
}

/**
 * How exposed the released rows are once every column is read together.
 *
 * Every other privacy figure is per column, so a file can report no unselected high or
 * medium risk column while postcode, birth date and job title jointly single out a third
 * of its rows. This is the figure that says so.
 *
 * Counted over every column in `matchedColumns` — including the format-only ones, which
 * contribute to the same classes — with the single exception of `distinctRowsAllColumns`.
 * Absent, never zeroed, on the paths with no rows to measure: a summary claiming zero unique
 * rows would read as a clean result from a check that never ran.
 */
export interface RowUniquenessSummary {
  rowsMeasured: number
  /**
   * The columns the measure read, each with what an outsider could match it on, in column
   * order. Empty means nothing released is matchable — not that the data is anonymous.
   *
   * Only columns that actually yielded something are listed, so no figure here rests on a
   * column whose projection came back empty on every row.
   */
  matchedColumns: MatchedColumn[]
  distinctClasses: number
  /** Rows alone in their class: holding those columns for a person finds their row. */
  uniqueRows: number
  /** The k-anonymity floor. One freak record sets it, hence the percentile beside it. */
  smallestClass: number
  /** The class size at or below which the most exposed 5% of rows sit. */
  fifthPercentileClassSize: number
  /**
   * Distinct rows over every released column, subset rule not applied. `null` when this
   * histogram alone outgrew what the check keeps, which does not make the joint figures
   * incomplete.
   */
  distinctRowsAllColumns: number | null
  /** The *joint* measurement stopped early; every count above is then a lower bound. */
  measurementIncomplete: boolean
  /**
   * What `uniqueRows` would have been with each matched column dropped, best first.
   *
   * The only figure here anyone can act on. Empty both when nothing was measured and when
   * there is no matched column at all, which `dropAttributionIncomplete` tells apart.
   */
  dropColumnEffects: DropColumnEffect[]
  /**
   * `dropColumnEffects` is empty for a reason other than there being nothing to say: the
   * joint measurement stopped, the file was wider than the attribution tracks, or the
   * leave-one-out histograms outgrew their budget.
   */
  dropAttributionIncomplete: boolean
}

/**
 * What dropping one column would do to the count of rows that stand alone.
 *
 * Measured, not estimated, and the two disagree often enough to be worth the pass: a column
 * that looks revealing can carry almost nothing once its strategy has flattened it, and one
 * of two correlated columns can be dropped without moving the count at all.
 */
export interface DropColumnEffect {
  columnIndex: number
  /**
   * Rows still alone in their class with this column dropped and every other unchanged.
   * Never larger than `uniqueRows` — removing a column only merges classes.
   */
  uniqueRowsWithout: number
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
