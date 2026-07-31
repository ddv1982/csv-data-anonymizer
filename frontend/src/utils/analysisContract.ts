import type { AnalyzeResponse, ColumnEvidenceProfile, ColumnMetadata, PasteAnalyzeData } from '../types'

const dataTypes = new Set([
  'email', 'uuid', 'timestamp', 'numericId', 'numericValue', 'postalCode', 'address',
  'ipAddress', 'url', 'macAddress', 'taxId', 'boolean', 'currency', 'percentage',
  'countryCode', 'phone', 'firstName', 'lastName', 'fullName', 'enum', 'string', 'unknown',
])
const confidences = new Set(['high', 'medium', 'low'])
const findingKinds = new Set([
  'person', 'contact', 'privateAddress', 'addressRegion', 'privateDate',
  'accountOrFinancialId', 'recordIdentifier', 'governmentId', 'credentialOrSecret',
  'networkOrDeviceId', 'url', 'mixedSensitiveText', 'unknown',
])
const strategies = new Set([
  'auto', 'pseudonymize', 'tokenize', 'localAi', 'mask', 'label', 'redact', 'passThrough',
])

export function validateAnalyzeResponse(response: AnalyzeResponse): AnalyzeResponse {
  validateColumns(response?.headers?.columns, 'file')
  return response
}

export function validatePasteAnalyzeData(response: PasteAnalyzeData): PasteAnalyzeData {
  validateColumns(response?.columns, 'pasted data')
  return response
}

export function completeEvidenceProfile(value: unknown): ColumnEvidenceProfile | null {
  if (!isRecord(value)) return null
  const format = value.formatEvidence
  const semantic = value.semanticDecision
  const privacy = value.privacyDecision
  const redaction = value.redactionDecision
  if (!isRecord(format) || !isRecord(semantic) || !isRecord(privacy) || !isRecord(redaction)) {
    return null
  }

  const detectors = format.detectors === undefined ? [] : format.detectors
  const supportingEvidence = semantic.supportingEvidence === undefined
    ? []
    : semantic.supportingEvidence
  const conflictingEvidence = semantic.conflictingEvidence === undefined
    ? []
    : semantic.conflictingEvidence

  if (
    !isEnum(format.dataType, dataTypes) ||
    !isEnum(format.confidence, confidences) ||
    !isCount(format.matchCount) ||
    !isCount(format.sampleCount) ||
    !isEnum(format.basis, new Set(['detectionSample', 'userOverride', 'retainedPreviewValues'])) ||
    !isStringArray(detectors) ||
    !isEnum(semantic.kind, findingKinds) ||
    !isEnum(semantic.confidence, confidences) ||
    !isEnum(semantic.status, new Set(['resolved', 'uncertain', 'conflicting'])) ||
    !isEnum(semantic.specificity, new Set(['generic', 'specific'])) ||
    !isStringArray(supportingEvidence) ||
    !isStringArray(conflictingEvidence) ||
    typeof semantic.reason !== 'string' ||
    !isEnum(privacy.risk, new Set(['high', 'medium', 'low'])) ||
    !isEnum(privacy.recommendedStrategy, strategies) ||
    typeof privacy.autoSelected !== 'boolean' ||
    typeof privacy.reason !== 'string' ||
    typeof redaction.placeholder !== 'string' ||
    redaction.placeholder.length === 0 ||
    !isEnum(redaction.source, new Set(['typed', 'columnHeader', 'generic'])) ||
    typeof redaction.isTyped !== 'boolean' ||
    typeof redaction.preservesEquality !== 'boolean' ||
    typeof redaction.reason !== 'string'
  ) {
    return null
  }

  const profile = value as unknown as ColumnEvidenceProfile
  return {
    ...profile,
    formatEvidence: { ...profile.formatEvidence, detectors },
    semanticDecision: {
      ...profile.semanticDecision,
      supportingEvidence,
      conflictingEvidence,
    },
  }
}

function validateColumns(columns: ColumnMetadata[] | undefined, source: string) {
  if (!Array.isArray(columns)) throw incompatibleAnalysis(source)
  for (const column of columns) {
    const profile = completeEvidenceProfile(column?.evidenceProfile)
    if (!profile) throw incompatibleAnalysis(source)
    column.evidenceProfile = profile
  }
}

function incompatibleAnalysis(source: string) {
  return new Error(
    `The ${source} analysis returned incompatible decision data. Restart the application and try again.`,
  )
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every((item) => typeof item === 'string')
}

function isCount(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0
}

function isEnum(value: unknown, allowed: Set<string>): value is string {
  return typeof value === 'string' && allowed.has(value)
}
