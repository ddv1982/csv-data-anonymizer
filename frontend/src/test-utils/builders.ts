import type {
  ColumnMetadata,
  ColumnReleaseReport,
  DetectionCoverageSummary,
  LocalAiStatus,
  PreflightData,
  PrivacyReport,
  RowUniquenessSummary,
} from '../types'

// Detection read the whole paste, so no sampling caveat applies. The default for
// paste-analyze fixtures whose subject is something other than coverage: a partial
// value there would put a warning banner into every unrelated assertion.
export const completeDetectionCoverage: DetectionCoverageSummary = {
  examined: 1,
  total: 1,
  unit: 'rows',
  isPartial: false,
}

export function partialDetectionCoverage(
  overrides: Partial<DetectionCoverageSummary> = {},
): DetectionCoverageSummary {
  return { examined: 100, total: 400, unit: 'rows', isPartial: true, ...overrides }
}

export function columnMetadataFixture(overrides: Partial<ColumnMetadata> = {}): ColumnMetadata {
  const piiRisk = overrides.piiRisk ?? 'high'
  return {
    name: 'email',
    headerLabelIsAmbiguous: false,
    index: 0,
    detectedType: 'email',
    confidence: 'high',
    piiRisk,
    sampleValues: ['sample'],
    sampleValueDistribution: { columnIndex: 0, distinctValues: 0, totalValues: 0, singletonValues: 0, doubletonValues: 0, maxValueOccurrences: 0 },
    emptyFormat: 'emptyString',
    isSelected: false,
    strategy: piiRisk === 'high' || piiRisk === 'medium' ? 'redact' : 'auto',
    reviewReasons: [],
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
    labelledColumns: 0,
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
    columnValueDistributions: [],
    // Absent by default, which is what the paths with no rows to measure report. Tests
    // that care about the joint measure pass one in explicitly.
    rowUniqueness: null,
    utilityMetrics: [],
    notes: [],
    ...overrides,
  }
}

export function columnReportFixture(
  overrides: Partial<ColumnReleaseReport> = {},
): ColumnReleaseReport {
  return {
    columnIndex: 0,
    columnName: 'email',
    selected: true,
    detectedType: 'email',
    piiRisk: 'high',
    strategy: 'redact',
    action: 'Redacted values',
    status: 'verified',
    detail: 'All selected email values were redacted.',
    ...overrides,
  }
}

/**
 * A measured joint re-identifiability summary in which nothing was singled out.
 *
 * The default is the harmless file on purpose: a fixture that arrives already exposed would
 * let a panel that renders the exposure unconditionally pass every test that never mentions
 * it. Tests that are about an exposed file say so by passing `uniqueRows` and the figures it
 * has to agree with — `rowsMeasured` and `distinctClasses` — because a builder cannot guess
 * how many rows or classes a story wants.
 *
 * `smallestClass` and the 5% figure are the exception: a row alone in its combination *is* a
 * class of one, so `uniqueRows > 0` beside a smallest class of 40 describes a file that
 * cannot exist. They follow `uniqueRows` rather than sitting at a fixed default, and both
 * stay overridable for a test whose subject they are.
 */
export function rowUniquenessFixture(
  overrides: Partial<RowUniquenessSummary> = {},
): RowUniquenessSummary {
  const uniqueRows = overrides.uniqueRows ?? 0
  const smallestClass = uniqueRows > 0 ? 1 : 40
  return {
    rowsMeasured: 100,
    matchedColumns: [{ columnIndex: 1, matchedOn: 'wholeValue', matchedEveryRow: true }],
    distinctClasses: 12,
    uniqueRows,
    smallestClass,
    fifthPercentileClassSize: smallestClass,
    distinctRowsAllColumns: 100,
    measurementIncomplete: false,
    // Empty with the flag set: nothing was attributed, which is what a file the drop pass
    // never reached reports. A test about the advice line passes both.
    dropColumnEffects: [],
    dropAttributionIncomplete: true,
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
