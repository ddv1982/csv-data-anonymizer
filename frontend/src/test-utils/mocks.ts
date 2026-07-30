import { vi } from 'vitest'
import { defaultSettings } from '../defaults'
import type { AppSettings, ColumnMetadata, PrivacyReport } from '../types'
import {
  columnMetadataFixture,
  localAiStatusFixture,
  privacyReportFixture,
  verifiedPreflightFixture,
} from './builders'

/**
 * The shape `firstPreflightBlocker` reads, rather than the whole `PreflightData`.
 *
 * The mock is handed whatever a test's preflight fixture happens to be, and narrowing it to
 * the one field the real function reads keeps a fixture that omits an unrelated field usable.
 */
type PreflightLike = { readiness: { blockers: string[] } }

/**
 * Every export of `src/tauri`, mocked.
 *
 * Complete rather than per-test on purpose: a partial mock turns a component reaching for a
 * command the test did not think about into `undefined is not a function`, which reads as a
 * bug in the component. Vitest gives each test file its own module registry, so this single
 * object is still one fresh set of spies per file.
 *
 * Consumed through a dynamic import in the `vi.mock` factory, because that factory runs while
 * the module under test is being imported — before this module's own bindings exist.
 */
export const tauriMocks = {
  loadSettings: vi.fn(),
  saveSettings: vi.fn(),
  pickInputCsv: vi.fn(),
  pickOutputCsv: vi.fn(),
  analyzeCsv: vi.fn(),
  analyzePasteData: vi.fn(),
  previewPasteData: vi.fn(),
  transformPasteData: vi.fn(),
  generateQuickValues: vi.fn(),
  countCsvRows: vi.fn(),
  preflightAnonymization: vi.fn(),
  firstPreflightBlocker: vi.fn((preflight: PreflightLike) => preflight.readiness.blockers[0] ?? null),
  previewAnonymization: vi.fn(),
  startAnonymizeJob: vi.fn(),
  getAnonymizeJobStatus: vi.fn(),
  cancelAnonymizeJob: vi.fn(),
  openOutputLocation: vi.fn(),
  getLocalAiStatus: vi.fn(),
  startLocalAiModelDownload: vi.fn(),
  getLocalAiModelDownloadStatus: vi.fn(),
  cancelLocalAiModelDownload: vi.fn(),
  openLocalAiSetupUrl: vi.fn(),
  setAppTheme: vi.fn(),
}

/**
 * Clears every call record and restores the baseline every workflow needs to start at all:
 * settings that load, a preflight that verifies, and Local AI switched off.
 *
 * `firstPreflightBlocker` is re-implemented here as well as at construction, so a test file
 * that resets rather than clears still gets the real reading of a blocked preflight — a mock
 * that silently returns `undefined` would let a blocked run proceed.
 */
export function resetTauriMocks() {
  vi.clearAllMocks()
  tauriMocks.loadSettings.mockResolvedValue(settingsFixture())
  tauriMocks.saveSettings.mockImplementation(async (settings: AppSettings) => settings)
  tauriMocks.preflightAnonymization.mockResolvedValue(verifiedPreflightFixture())
  tauriMocks.firstPreflightBlocker.mockImplementation(
    (preflight: PreflightLike) => preflight.readiness.blockers[0] ?? null,
  )
  tauriMocks.getLocalAiStatus.mockResolvedValue(localAiStatusFixture())
}

export function settingsFixture(overrides: Partial<AppSettings> = {}): AppSettings {
  return { ...defaultSettings, ...overrides }
}

/**
 * A detected column, addressed the way the analyze responses address one.
 *
 * `isSelected` follows the risk, which is what the backend does with a freshly analyzed file.
 * A test whose subject is a selection the user or the backend chose passes it explicitly.
 */
export function columnFixture(
  index: number,
  name: string,
  detectedType: ColumnMetadata['detectedType'],
  piiRisk: ColumnMetadata['piiRisk'],
  overrides: Partial<ColumnMetadata> = {},
): ColumnMetadata {
  return columnMetadataFixture({
    name,
    index,
    detectedType,
    piiRisk,
    isSelected: piiRisk === 'high' || piiRisk === 'medium',
    ...overrides,
  })
}

/** The report a run that actually transformed something comes back with. */
export function transformedPrivacyReportFixture(
  overrides: Partial<PrivacyReport> = {},
): PrivacyReport {
  return privacyReportFixture({
    directIdentifiers: 1,
    pseudonymizedColumns: 1,
    uniquePseudonymValues: 1,
    ...overrides,
  })
}
