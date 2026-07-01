import type { ColumnMetadata, LocalAiStatus, PreflightData, PrivacyReport } from '../types'

export function columnMetadataFixture(overrides: Partial<ColumnMetadata> = {}): ColumnMetadata {
  const piiRisk = overrides.piiRisk ?? 'high'
  return {
    name: 'email',
    index: 0,
    detectedType: 'email',
    confidence: 'high',
    piiRisk,
    sampleValues: ['sample'],
    emptyFormat: 'emptyString',
    isSelected: false,
    strategy: piiRisk === 'high' || piiRisk === 'medium' ? 'redact' : 'auto',
    ...overrides,
  }
}

export function privacyReportFixture(overrides: Partial<PrivacyReport> = {}): PrivacyReport {
  return {
    directIdentifiers: 0,
    quasiIdentifiers: 0,
    pseudonymizedColumns: 0,
    smartReplacementColumns: 0,
    opaqueTokenColumns: 0,
    maskedColumns: 0,
    redactedColumns: 0,
    passThroughColumns: 0,
    uniquePseudonymValues: 0,
    reusedPseudonymValues: 0,
    collisionsAvoided: 0,
    exhaustedPseudonymPools: 0,
    opaqueTokenValues: 0,
    smartReplacementValues: 0,
    smartReplacementRejections: 0,
    smartReplacementRejectionReasons: [],
    smartReplacementFallbacks: 0,
    shapeFallbackValues: 0,
    readiness: {
      status: 'verified',
      blockers: [],
      reviewItems: [],
      verifiedItems: [],
    },
    evidence: [],
    columnReports: [],
    utilityMetrics: [],
    notes: [],
    ...overrides,
  }
}

export function verifiedPreflightFixture(overrides: Partial<PreflightData> = {}): PreflightData {
  return {
    mode: 'anonymize',
    readiness: {
      status: 'verified',
      blockers: [],
      reviewItems: [],
      verifiedItems: [],
    },
    evidence: [],
    columnReports: [],
    ...overrides,
  }
}

export function localAiStatusFixture(overrides: Partial<LocalAiStatus> = {}): LocalAiStatus {
  return {
    enabled: false,
    provider: 'ollama',
    model: 'gemma3:4b',
    availableModels: [],
    endpoint: 'http://127.0.0.1:11434',
    runtimeAvailable: false,
    modelInstalled: false,
    ready: false,
    runtimeVersion: null,
    message: 'Local AI is off.',
    ...overrides,
  }
}
