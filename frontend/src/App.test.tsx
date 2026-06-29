import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import App from './App'
import { DpBudgetSettings } from './components/privacy-settings/DpBudgetSettings'
import { defaultSettings } from './defaults'
import { MAX_PASTE_CONTENT_BYTES } from './limits'
import type { AppSettings, ColumnMetadata, PrivacyReport } from './types'

type PreflightLike = { readiness: { blockers: string[] } }

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
}))

vi.mock('./tauri', () => tauriMocks)

const clipboardWriteText = vi.fn()

describe('App input mode tabs', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    tauriMocks.loadSettings.mockResolvedValue(settingsFixture())
    tauriMocks.saveSettings.mockImplementation(async (settings: AppSettings) => settings)
    tauriMocks.preflightAnonymization.mockResolvedValue(verifiedPreflightFixture())
    tauriMocks.firstPreflightBlocker.mockImplementation(
      (preflight: PreflightLike) => preflight.readiness.blockers[0] ?? null,
    )
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

  it('keeps Local AI settings in the global topbar modal', async () => {
    const user = userEvent.setup()
    render(<App />)

    expect(screen.getByRole('switch', { name: /use local ai/i })).toBeInTheDocument()
    expect(screen.queryByLabelText('Model')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: /open local ai settings/i }))

    const dialog = await screen.findByRole('dialog', { name: /local ai settings/i })
    expect(within(dialog).getByText('Local AI')).toBeInTheDocument()
    expect(within(dialog).getByLabelText('Model')).toHaveValue('gemma3:4b')

    await user.click(within(dialog).getByRole('button', { name: /close local ai settings/i }))
    await waitFor(() => {
      expect(screen.queryByRole('dialog', { name: /local ai settings/i })).not.toBeInTheDocument()
    })

    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    await user.click(screen.getByRole('button', { name: /open local ai settings/i }))
    expect(await screen.findByRole('dialog', { name: /local ai settings/i })).toBeInTheDocument()
  })

  it('saves Local AI toggle changes from the topbar', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('switch', { name: /use local ai/i }))

    await waitFor(() => {
      expect(tauriMocks.saveSettings).toHaveBeenCalledWith(expect.objectContaining({ localAiEnabled: true }))
    })
  })

  it('does not save default settings before the initial settings load completes', async () => {
    const user = userEvent.setup()
    let resolveSettings: (settings: AppSettings) => void = () => undefined
    tauriMocks.loadSettings.mockReturnValue(
      new Promise<AppSettings>((resolve) => {
        resolveSettings = resolve
      }),
    )
    render(<App />)

    const localAiSwitch = screen.getByRole('switch', { name: /use local ai/i })
    const browseButton = screen.getByRole('button', { name: /browse for csv file/i })
    expect(localAiSwitch).toBeDisabled()
    expect(browseButton).toBeDisabled()
    await user.click(localAiSwitch)
    await user.click(browseButton)
    expect(tauriMocks.saveSettings).not.toHaveBeenCalled()
    expect(tauriMocks.pickInputCsv).not.toHaveBeenCalled()

    resolveSettings(settingsFixture({ localAiEnabled: true }))
    await waitFor(() => {
      expect(localAiSwitch).toHaveAttribute('aria-checked', 'true')
    })
    expect(browseButton).not.toBeDisabled()
    expect(tauriMocks.saveSettings).not.toHaveBeenCalled()
  })

  it('keeps paste and quick processing disabled until settings load', async () => {
    const user = userEvent.setup()
    let resolveSettings: (settings: AppSettings) => void = () => undefined
    tauriMocks.loadSettings.mockReturnValue(
      new Promise<AppSettings>((resolve) => {
        resolveSettings = resolve
      }),
    )
    render(<App />)

    await user.click(screen.getByRole('tab', { name: /paste data/i }))
    fireEvent.change(screen.getByLabelText(/pasted data/i), {
      target: { value: '[{"email":"ada@example.com"}]' },
    })
    const detectButton = screen.getByRole('button', { name: /detect fields/i })
    expect(detectButton).toBeDisabled()
    await user.click(detectButton)
    expect(tauriMocks.analyzePasteData).not.toHaveBeenCalled()

    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    const generateButton = screen.getByRole('button', { name: /generate values/i })
    expect(generateButton).toBeDisabled()
    await user.click(generateButton)
    expect(tauriMocks.generateQuickValues).not.toHaveBeenCalled()

    resolveSettings(settingsFixture())
    await waitFor(() => {
      expect(generateButton).not.toBeDisabled()
    })
  })

  it('blocks Smart replacement when the ready Local AI status is for another model', async () => {
    const user = userEvent.setup()
    tauriMocks.loadSettings.mockResolvedValue(
      settingsFixture({ localAiEnabled: true, localAiModel: 'llama3.2:3b' }),
    )
    tauriMocks.getLocalAiStatus.mockResolvedValue({
      enabled: true,
      provider: 'ollama',
      model: 'gemma3:4b',
      availableModels: ['gemma3:4b'],
      endpoint: 'http://127.0.0.1:11434',
      runtimeAvailable: true,
      modelInstalled: true,
      ready: true,
      runtimeVersion: '0.9.0',
      message: 'Ready.',
    })
    render(<App />)

    await waitFor(() => {
      expect(screen.getByRole('switch', { name: /use local ai/i })).toHaveAttribute('aria-checked', 'true')
    })
    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    await user.selectOptions(screen.getByRole('combobox', { name: 'Strategy' }), 'localAi')

    expect(screen.getByRole('alert')).toHaveTextContent(/Set up Local AI before generating Smart replacement values/)
    expect(screen.getByRole('button', { name: /generate values/i })).toBeDisabled()
  })

  it('disables CSV output creation when selected Smart replacement needs Local AI setup', async () => {
    const user = userEvent.setup()
    tauriMocks.pickInputCsv.mockResolvedValue('/data/input.csv')
    tauriMocks.analyzeCsv.mockResolvedValue({
      headers: {
        filePath: '/data/input.csv',
        rowCount: 1,
        rowCountIsComplete: true,
        defaultOutputPath: '/data/input_private_output.csv',
        columns: [columnFixture(0, 'email', 'email', 'high')],
      },
      selectedColumns: [0],
      suggestedOutputPath: '/data/input_private_output.csv',
    })
    render(<App />)

    await user.click(screen.getByRole('button', { name: /browse for csv file/i }))
    await user.selectOptions(await screen.findByLabelText('Strategy for email'), 'localAi')

    expect(screen.getByText(/Set up Local AI before previewing or creating output/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /create anonymized csv/i })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: /create anonymized csv/i }))
    expect(tauriMocks.preflightAnonymization).not.toHaveBeenCalled()
  })

  it('renders Redact for column workflows but not quick generation', async () => {
    const user = userEvent.setup()
    tauriMocks.pickInputCsv.mockResolvedValue('/data/input.csv')
    tauriMocks.analyzeCsv.mockResolvedValue({
      headers: {
        filePath: '/data/input.csv',
        rowCount: 1,
        rowCountIsComplete: true,
        defaultOutputPath: '/data/input_private_output.csv',
        columns: [columnFixture(0, 'email', 'email', 'high')],
      },
      selectedColumns: [0],
      suggestedOutputPath: '/data/input_private_output.csv',
    })
    tauriMocks.analyzePasteData.mockResolvedValue({
      format: 'json',
      rowCount: 1,
      rowCountIsComplete: true,
      columns: [columnFixture(0, '[].email', 'email', 'high')],
    })
    render(<App />)

    await user.click(screen.getByRole('button', { name: /browse for csv file/i }))
    const csvStrategy = await screen.findByRole('combobox', { name: 'Strategy for email' })
    expect(within(csvStrategy).getByRole('option', { name: 'Redact' })).toHaveValue('redact')
    await user.selectOptions(csvStrategy, 'redact')
    expect(csvStrategy).toHaveValue('redact')

    await user.click(screen.getByRole('tab', { name: /paste data/i }))
    fireEvent.change(screen.getByLabelText(/pasted data/i), {
      target: { value: '[{"email":"ada@example.com"}]' },
    })
    await user.click(screen.getByRole('button', { name: /detect fields/i }))
    const pasteStrategy = await screen.findByRole('combobox', { name: 'Strategy for [].email' })
    expect(within(pasteStrategy).getByRole('option', { name: 'Redact' })).toHaveValue('redact')
    await user.selectOptions(pasteStrategy, 'redact')
    expect(pasteStrategy).toHaveValue('redact')

    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    const quickStrategy = screen.getByRole('combobox', { name: 'Strategy' })
    expect(within(quickStrategy).queryByRole('option', { name: 'Redact' })).not.toBeInTheDocument()
    expect(quickStrategy).not.toHaveValue('redact')
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
      smartReplacements: [
        {
          columnIndex: 0,
          original: 'ada@example.com',
          replacement: 'masked@example.com',
        },
      ],
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
      [{ columnIndex: 0, original: 'ada@example.com', replacement: 'masked@example.com' }],
      { enabled: false, model: 'gemma3:4b' },
    )
    expect(await screen.findByDisplayValue('[{"email":"masked@example.com"}]')).toBeInTheDocument()
    expect(screen.getByRole('heading', { name: /privacy report/i })).toBeInTheDocument()
    expect(screen.getByText('Direct identifiers')).toBeInTheDocument()

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
    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent(/Set up Local AI before previewing or anonymizing/)
    await user.click(within(alert).getByRole('button', { name: /open local ai settings/i }))
    expect(await screen.findByRole('dialog', { name: /local ai settings/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /show preview/i })).toBeDisabled()
  })

  it('treats Synthetic data as a global CSV release mode and locks column selection', async () => {
    const user = userEvent.setup()
    tauriMocks.pickInputCsv.mockResolvedValue('/data/input.csv')
    tauriMocks.analyzeCsv.mockResolvedValue({
      headers: {
        filePath: '/data/input.csv',
        rowCount: 3,
        rowCountIsComplete: true,
        defaultOutputPath: '/data/input_private_output.csv',
        columns: [
          columnFixture(0, 'email', 'email', 'high'),
          columnFixture(1, 'country', 'countryCode', 'medium'),
          columnFixture(2, 'notes', 'string', 'low'),
        ],
      },
      selectedColumns: [0, 1],
      suggestedOutputPath: '/data/input_private_output.csv',
    })
    render(<App />)

    await user.click(screen.getByRole('button', { name: /browse for csv file/i }))
    expect(await screen.findByText('2 of 3 columns selected, 3 rows loaded')).toBeInTheDocument()

    await user.selectOptions(screen.getByLabelText('Strategy for email'), 'localAi')
    expect(screen.getByText(/Set up Local AI before previewing or creating output/)).toBeInTheDocument()

    const columnStep = screen.getByRole('heading', { name: '2. Release Mode and Columns' }).closest('section')
    expect(columnStep).not.toBeNull()
    await user.selectOptions(within(columnStep as HTMLElement).getByLabelText('Privacy release mode'), 'syntheticData')

    expect(screen.getByText(/Synthetic data creates a complete replacement dataset/)).toBeInTheDocument()
    expect(screen.getByText('3 of 3 columns selected, 3 rows loaded')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /deselect all/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /select high detector risk/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /select detected risk/i })).not.toBeInTheDocument()
    expect(screen.getByRole('checkbox', { name: /column notes included in synthetic data/i })).toBeDisabled()
    expect(screen.getByLabelText('Type override for email')).not.toBeDisabled()
    expect(screen.getByLabelText('Privacy role for email')).not.toBeDisabled()
    expect(screen.getByLabelText('Strategy for email')).toBeDisabled()
    expect(screen.getByLabelText('Strategy for email')).toHaveValue('auto')
    expect(screen.queryByText(/Set up Local AI before previewing or creating output/)).not.toBeInTheDocument()
    expect(screen.getByText(/Preview is disabled for Synthetic data/)).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /show preview/i })).toBeDisabled()
  })

  it('shows detector review highlights, redaction output, and strict evidence', async () => {
    const user = userEvent.setup()
    tauriMocks.analyzeCsv.mockResolvedValue({
      headers: {
        filePath: '/data/input.csv',
        rowCount: 2,
        rowCountIsComplete: true,
        defaultOutputPath: '/data/input_private_output.csv',
        columns: [
          {
            ...columnFixture(0, 'notes', 'string', 'high'),
            sampleValues: ['contact ada@example.com'],
            privacyEvidence: [
              {
                kind: 'contact',
                dataType: 'email',
                confidence: 'high',
                matchCount: 1,
                sampleCount: 1,
                score: 96,
                reason: 'Email address pattern.',
              },
            ],
            privacyFindings: [
              {
                kind: 'contact',
                dataType: 'email',
                rowIndex: 0,
                start: 8,
                end: 23,
                matchValue: 'ada@example.com',
                sampleValue: 'contact ada@example.com',
                confidence: 'high',
                score: 96,
                detector: 'pattern:email',
                reason: 'Email address pattern.',
              },
            ],
          },
          {
            ...columnFixture(1, 'event_date', 'timestamp', 'low'),
            sampleValues: ['2026-06-29'],
            privacyEvidence: [
              {
                kind: 'privateDate',
                dataType: 'timestamp',
                confidence: 'low',
                matchCount: 1,
                sampleCount: 1,
                score: 54,
                reason: 'Date or timestamp pattern; review context before treating it as private.',
              },
            ],
            privacyFindings: [
              {
                kind: 'privateDate',
                dataType: 'timestamp',
                rowIndex: 0,
                start: 0,
                end: 10,
                matchValue: '2026-06-29',
                sampleValue: '2026-06-29',
                confidence: 'low',
                score: 54,
                detector: 'pattern:date',
                reason: 'Date or timestamp pattern; review context before treating it as private.',
              },
            ],
          },
        ],
      },
      selectedColumns: [0],
      suggestedOutputPath: '/data/input_private_output.csv',
    })
    render(<App />)

    await user.click(screen.getByRole('button', { name: /browse for csv file/i }))
    await user.click(await screen.findByRole('tab', { name: /detector review/i }))

    expect(screen.getByText('Detected spans')).toBeInTheDocument()
    expect(screen.getByText('ada@example.com')).toBeInTheDocument()
    expect(screen.getByText('contact [EMAIL]')).toBeInTheDocument()
    expect(screen.queryByText('Private date')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: 'Strict' }))

    expect(screen.getByText(/Strict detector review includes/)).toBeInTheDocument()
    expect(screen.getAllByText('Private date').length).toBeGreaterThan(0)
    expect(screen.getByText('[DATE]')).toBeInTheDocument()
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
    expect(screen.getByRole('heading', { name: /privacy report/i })).toBeInTheDocument()
    expect(screen.getByText('Pseudonymized columns')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /detect fields/i })).not.toBeInTheDocument()
  })

  it('shows Local AI setup for quick Smart replacement generation', async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(screen.getByRole('tab', { name: /quick by data type/i }))
    await user.selectOptions(screen.getByRole('combobox', { name: 'Strategy' }), 'localAi')

    expect(screen.getByText('Local AI')).toBeInTheDocument()
    const alert = screen.getByRole('alert')
    expect(alert).toHaveTextContent(/Set up Local AI before generating Smart replacement values/)
    await user.click(within(alert).getByRole('button', { name: /open local ai settings/i }))
    expect(await screen.findByRole('dialog', { name: /local ai settings/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /generate values/i })).toBeDisabled()
  })

  it('requires typed confirmation before resetting the DP budget', async () => {
    const user = userEvent.setup()
    const onResetBudget = vi.fn()
    const prompt = vi.spyOn(window, 'prompt')
    prompt.mockReturnValueOnce('')

    render(
      <DpBudgetSettings
        budget={{ enabled: true, limitEpsilon: 1, spentEpsilon: 0.5, action: 'block' }}
        disabled={false}
        onResetBudget={onResetBudget}
        onChange={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: /reset budget/i }))
    expect(onResetBudget).not.toHaveBeenCalled()

    prompt.mockReturnValueOnce('RESET DP BUDGET')
    await user.click(screen.getByRole('button', { name: /reset budget/i }))
    expect(onResetBudget).toHaveBeenCalledTimes(1)

    prompt.mockRestore()
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
    redactedColumns: 0,
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
    smartReplacementRejections: 0,
    smartReplacementRejectionReasons: [],
    smartReplacementFallbacks: 0,
    formalModels: [],
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

function verifiedPreflightFixture() {
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
  }
}
