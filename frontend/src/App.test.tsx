import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App'
import { defaultSettings } from './defaults'
import { MAX_PASTE_CONTENT_BYTES } from './limits'
import type { AppSettings, ColumnMetadata, PrivacyReport } from './types'

const tauriMocks = vi.hoisted(() => ({
  loadSettings: vi.fn(),
  saveSettings: vi.fn(),
  resetDpBudgetLedger: vi.fn(),
  pickInputCsv: vi.fn(),
  pickOutputCsv: vi.fn(),
  analyzeCsv: vi.fn(),
  analyzePasteData: vi.fn(),
  previewPasteData: vi.fn(),
  transformPasteData: vi.fn(),
  generateQuickValues: vi.fn(),
  countCsvRows: vi.fn(),
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
}))

vi.mock('./tauri', () => tauriMocks)

const clipboardWriteText = vi.fn()

describe('App input mode tabs', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    tauriMocks.loadSettings.mockResolvedValue(settingsFixture())
    tauriMocks.saveSettings.mockImplementation(async (settings: AppSettings) => settings)
    tauriMocks.getLocalAiStatus.mockResolvedValue({
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
    })
    clipboardWriteText.mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: clipboardWriteText,
      },
    })
  })

  it('keeps controls scoped to the active tab', async () => {
    const user = userEvent.setup()
    render(<App />)

    expect(screen.getByRole('button', { name: /browse for csv file/i })).toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: /paste data/i }))
    expect(screen.getByLabelText(/format/i)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /detect fields/i })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /browse for csv file/i })).not.toBeInTheDocument()

    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    expect(screen.getByRole('combobox', { name: 'Data Type' })).toBeInTheDocument()
    expect(screen.getByRole('spinbutton', { name: /quantity/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /generate values/i })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /detect fields/i })).not.toBeInTheDocument()
  })

  it('supports keyboard navigation for input mode tabs', async () => {
    const user = userEvent.setup()
    render(<App />)

    const csvTab = screen.getByRole('tab', { name: /csv file/i })
    const pasteTab = screen.getByRole('tab', { name: /paste data/i })
    const quickTab = screen.getByRole('tab', { name: /quick by data type/i })

    csvTab.focus()
    await user.keyboard('{ArrowRight}')
    expect(pasteTab).toHaveAttribute('aria-selected', 'true')
    expect(pasteTab).toHaveFocus()
    expect(screen.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', 'input-mode-tab-paste')

    await user.keyboard('{End}')
    expect(quickTab).toHaveAttribute('aria-selected', 'true')
    expect(quickTab).toHaveFocus()
    expect(screen.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', 'input-mode-tab-quick')

    await user.keyboard('{Home}')
    expect(csvTab).toHaveAttribute('aria-selected', 'true')
    expect(csvTab).toHaveFocus()
    expect(screen.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', 'input-mode-tab-csv')
  })

  it('analyzes pasted JSON, transforms selected fields, and copies output', async () => {
    const user = userEvent.setup()
    tauriMocks.analyzePasteData.mockResolvedValue({
      format: 'json',
      rowCount: 1,
      rowCountIsComplete: true,
      columns: [columnFixture(0, '[].email', 'email', 'high')],
    })
    tauriMocks.previewPasteData.mockResolvedValue({
      previews: [
        {
          columnIndex: 0,
          columnName: '[].email',
          samples: [{ original: 'ada@example.com', anonymized: 'masked@example.com' }],
        },
      ],
      warnings: [],
      smartReplacements: [],
    })
    tauriMocks.transformPasteData.mockResolvedValue({
      output: '[{"email":"masked@example.com"}]',
      rowCount: 1,
      columnsAnonymized: 1,
      durationMs: 4,
      privacyReport: privacyReportFixture(),
    })
    render(<App />)

    await user.click(screen.getByRole('tab', { name: /paste data/i }))
    fireEvent.change(screen.getByLabelText(/pasted data/i), {
      target: { value: '[{"email":"ada@example.com"}]' },
    })
    await user.click(screen.getByRole('button', { name: /detect fields/i }))

    expect(await screen.findByText('[].email')).toBeInTheDocument()
    expect(screen.getByText('Detected: JSON')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: /show preview/i }))
    expect(await screen.findByText('masked@example.com')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /anonymize pasted data/i }))

    expect(tauriMocks.transformPasteData).toHaveBeenCalledWith(
      '[{"email":"ada@example.com"}]',
      'json',
      [0],
      [],
      false,
      '',
      { enabled: false, model: 'gemma3:4b' },
    )
    expect(await screen.findByDisplayValue('[{"email":"masked@example.com"}]')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /^copy$/i }))
    expect(await screen.findByText('Copied')).toBeInTheDocument()
  })

  it('blocks pasted content that is too large for direct input', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('tab', { name: /paste data/i }))
    fireEvent.change(screen.getByLabelText(/pasted data/i), {
      target: { value: 'x'.repeat(MAX_PASTE_CONTENT_BYTES + 1) },
    })

    expect(screen.getByRole('button', { name: /detect fields/i })).toBeDisabled()
    expect(screen.getByRole('alert')).toHaveTextContent(/Paste at most 5 MiB/)
    expect(tauriMocks.analyzePasteData).not.toHaveBeenCalled()
  })

  it('shows Local AI setup for pasted fields using Smart replacement', async () => {
    const user = userEvent.setup()
    tauriMocks.analyzePasteData.mockResolvedValue({
      format: 'json',
      rowCount: 1,
      rowCountIsComplete: true,
      columns: [columnFixture(0, '[].email', 'email', 'high')],
    })
    render(<App />)

    await user.click(screen.getByRole('tab', { name: /paste data/i }))
    fireEvent.change(screen.getByLabelText(/pasted data/i), {
      target: { value: '[{"email":"ada@example.com"}]' },
    })
    await user.click(screen.getByRole('button', { name: /detect fields/i }))
    await user.selectOptions(await screen.findByLabelText('Strategy for [].email'), 'localAi')

    expect(screen.getByText('Local AI')).toBeInTheDocument()
    expect(screen.getByText(/Set up Local AI before previewing or anonymizing/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /show preview/i })).toBeDisabled()
  })

  it('generates quick values without requiring input or field detection', async () => {
    const user = userEvent.setup()
    tauriMocks.generateQuickValues.mockResolvedValue({
      output: 'masked@example.com',
      rowCount: 1,
      values: [{ original: 'person1@example.invalid', anonymized: 'masked@example.com' }],
      privacyReport: privacyReportFixture(),
    })
    render(<App />)

    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    await user.click(screen.getByRole('button', { name: /generate values/i }))

    expect(tauriMocks.generateQuickValues).toHaveBeenCalledWith('email', 'auto', 1, false, '', {
      enabled: false,
      model: 'gemma3:4b',
    })
    expect(screen.queryByLabelText(/values to anonymize/i)).not.toBeInTheDocument()
    expect(await screen.findByLabelText(/generated values/i)).toHaveValue('masked@example.com')
    expect(screen.queryByRole('button', { name: /detect fields/i })).not.toBeInTheDocument()
  })

  it('shows Local AI setup for quick Smart replacement generation', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    await user.selectOptions(screen.getByRole('combobox', { name: 'Strategy' }), 'localAi')

    expect(screen.getByText('Local AI')).toBeInTheDocument()
    expect(screen.getByText(/Set up Local AI before generating Smart replacement values/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /generate values/i })).toBeDisabled()
  })
})

function settingsFixture(overrides: Partial<AppSettings> = {}): AppSettings {
  return {
    ...defaultSettings,
    ...overrides,
  }
}

function columnFixture(
  index: number,
  name: string,
  detectedType: ColumnMetadata['detectedType'],
  piiRisk: ColumnMetadata['piiRisk'],
): ColumnMetadata {
  return {
    name,
    index,
    detectedType,
    confidence: 'high',
    piiRisk,
    sampleValues: ['sample'],
    emptyFormat: 'emptyString',
    isSelected: false,
    strategy: 'auto',
  }
}

function privacyReportFixture(overrides: Partial<PrivacyReport> = {}): PrivacyReport {
  return {
    releaseMode: 'standard',
    directIdentifiers: 1,
    quasiIdentifiers: 0,
    sensitiveColumns: 0,
    pseudonymizedColumns: 1,
    smartReplacementColumns: 0,
    opaqueTokenColumns: 0,
    maskedColumns: 0,
    generalizedColumns: 0,
    passThroughColumns: 0,
    suppressedRows: 0,
    syntheticRows: 0,
    dpEpsilon: null,
    dpBudget: null,
    uniquePseudonymValues: 1,
    reusedPseudonymValues: 0,
    collisionsAvoided: 0,
    exhaustedPseudonymPools: 0,
    opaqueTokenValues: 0,
    smartReplacementValues: 0,
    smartReplacementFallbacks: 0,
    formalModels: [],
    notes: [],
    ...overrides,
  }
}
