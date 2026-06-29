import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'

declare global {
  interface Window {
    __CSV_ANONYMIZER_TEST_INVOKE__?: (command: string, args?: Record<string, unknown>) => unknown
    __CSV_ANONYMIZER_TEST_CALLS__?: Array<{ command: string; args?: Record<string, unknown> }>
    __CSV_ANONYMIZER_COPIED_TEXT__?: string
  }
}

test.beforeEach(async ({ page }) => {
  await installTauriMock(page)
})

test('covers disabled states, privacy scale warnings, and glossary help', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByLabel('Output Path')).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Create anonymized CSV' })).toBeDisabled()

  await page.getByRole('button', { name: 'Browse for CSV file' }).click()
  await expect(page.getByLabel('Output Path')).toBeEnabled()
  await expect(page.getByRole('checkbox', { name: 'Select column email' })).toBeChecked()

  await page.getByLabel('Privacy release mode').selectOption('formalTabular')
  await expect(page.getByRole('alert').filter({ hasText: 'will materialize 150,000 data rows' })).toBeVisible()

  await page.getByRole('button', { name: 'How privacy release works' }).click()
  const helpDialog = page.getByRole('dialog', { name: 'Privacy Release' })
  await expect(helpDialog).toBeVisible()
  await expect(helpDialog).toContainText('Every CSV column is included')
  await expect(helpDialog).toContainText('same schema, row count, roles, types, and seed')

  await helpDialog.getByRole('button', { name: 'k-anonymity', exact: true }).click()
  await expect(page.getByRole('tooltip')).toContainText('k-anonymity')

  await page.keyboard.press('Escape')
  await expect(page.getByRole('tooltip')).toBeHidden()
  await expect(page.getByRole('dialog', { name: 'Privacy Release' })).toBeVisible()

  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog', { name: 'Privacy Release' })).toBeHidden()
})

test('keeps Synthetic data as a global release mode with all CSV columns included', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Browse for CSV file' }).click()

  await expect(page.getByText('2 of 3 columns selected, 150,000 rows loaded')).toBeVisible()

  await page.getByLabel('Privacy release mode').selectOption('syntheticData')

  await expect(page.getByText(/Synthetic data is selected globally/)).toBeVisible()
  await expect(page.getByText('3 of 3 columns selected, 150,000 rows loaded')).toBeVisible()
  await expect(page.getByRole('button', { name: 'Deselect All' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Select High Detector Risk' })).toHaveCount(0)
  await expect(page.getByRole('button', { name: 'Select Detected Risk' })).toHaveCount(0)
  await expect(page.getByRole('checkbox', { name: 'Column notes included in synthetic data' })).toBeDisabled()
  await expect(page.getByLabel('Strategy for email')).toBeDisabled()
  await expect(page.getByText(/Preview is disabled for Synthetic data/)).toBeVisible()
  await expect(page.getByRole('button', { name: 'Show Preview' })).toBeDisabled()
})

test('recovers from preview errors and cancels a running job', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Browse for CSV file' }).click()

  await page.getByRole('button', { name: 'Show Preview' }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'Preview failed from e2e' })).toBeVisible()
  await page.getByRole('button', { name: 'Dismiss error message' }).click()

  await page.getByRole('button', { name: 'Show Preview' }).click()
  await expect(page.getByText('anon@example.test')).toBeVisible()

  await page.getByRole('button', { name: 'Create anonymized CSV' }).click()
  await expect(page.getByRole('status')).toContainText('Preparing 150,000 rows')

  await page.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'Output creation canceled.' })).toBeVisible()
})

test('switches tabs, pastes JSON, copies output, and quick-generates values', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByRole('button', { name: 'Browse for CSV file' })).toBeVisible()

  await page.getByRole('tab', { name: 'Paste Data' }).click()
  await expect(page.getByRole('button', { name: 'Browse for CSV file' })).toBeHidden()
  await page.getByLabel('Pasted data').fill('[{"email":"ada@example.com"}]')
  await page.getByRole('button', { name: 'Detect Fields' }).click()
  await expect(page.getByText('Detected: JSON')).toBeVisible()
  await expect(page.getByText('[].email')).toBeVisible()

  await page.getByRole('button', { name: 'Show Preview' }).click()
  await expect(page.getByText('anon@example.test')).toBeVisible()

  await page.getByRole('button', { name: 'Anonymize pasted data' }).click()
  await expect(page.getByLabel('Anonymized pasted data')).toHaveValue('[{"email":"anon@example.test"}]')
  await page.getByRole('button', { name: 'Copy' }).click()
  await expect(page.getByText('Copied')).toBeVisible()
  await expect.poll(() => page.evaluate(() => window.__CSV_ANONYMIZER_COPIED_TEXT__)).toBe('[{"email":"anon@example.test"}]')

  await page.getByRole('tab', { name: 'Quick by Data Type' }).click()
  await expect(page.getByRole('button', { name: 'Detect Fields' })).toBeHidden()
  await page.getByRole('combobox', { name: 'Data Type' }).selectOption('uuid')
  await page.getByRole('combobox', { name: 'Strategy' }).selectOption('tokenize')
  await page.getByRole('spinbutton', { name: 'Quantity' }).fill('2')
  await expect(page.getByLabel('Values to anonymize')).toHaveCount(0)
  await page.getByRole('button', { name: 'Generate values' }).click()
  await expect(page.getByLabel('Generated values')).toHaveValue('tok_e2e_1\ntok_e2e_2')

  const calls = await page.evaluate(() => window.__CSV_ANONYMIZER_TEST_CALLS__ ?? [])
  expect(calls.some((call) => call.command === 'analyze_pasted_data')).toBe(true)
  expect(calls.some((call) => call.command === 'preview_pasted_data')).toBe(true)
  expect(calls.some((call) => call.command === 'anonymize_pasted_data')).toBe(true)
  expect(
    calls.some(
      (call) =>
        call.command === 'generate_quick_values' &&
        (call.args?.request as { count?: number; dataType?: string; strategy?: string } | undefined)?.count === 2 &&
        (call.args?.request as { count?: number; dataType?: string; strategy?: string } | undefined)?.dataType === 'uuid' &&
        (call.args?.request as { count?: number; dataType?: string; strategy?: string } | undefined)?.strategy === 'tokenize',
    ),
  ).toBe(true)
})

test('supports keyboard focus for input tabs and help dialogs', async ({ page }) => {
  await page.goto('/')

  const csvTab = page.getByRole('tab', { name: 'CSV File' })
  const pasteTab = page.getByRole('tab', { name: 'Paste Data' })
  const quickTab = page.getByRole('tab', { name: 'Quick by Data Type' })

  await csvTab.focus()
  await page.keyboard.press('ArrowRight')
  await expect(pasteTab).toBeFocused()
  await expect(pasteTab).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', 'input-mode-tab-paste')

  await page.keyboard.press('End')
  await expect(quickTab).toBeFocused()
  await expect(quickTab).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', 'input-mode-tab-quick')

  await page.keyboard.press('Home')
  await expect(csvTab).toBeFocused()
  await expect(csvTab).toHaveAttribute('aria-selected', 'true')
  await expect(page.getByRole('tabpanel')).toHaveAttribute('aria-labelledby', 'input-mode-tab-csv')

  const localAiSettingsButton = page.getByRole('button', { name: 'Open Local AI settings' })
  await localAiSettingsButton.click()
  const localAiDialog = page.getByRole('dialog', { name: 'Local AI Settings' })
  await expect(localAiDialog).toBeVisible()
  await expect(localAiDialog.getByRole('button', { name: 'Close Local AI settings' })).toBeFocused()

  await page.keyboard.press('Escape')
  await expect(localAiDialog).toBeHidden()
  await expect(localAiSettingsButton).toBeFocused()

  const helpButton = page.getByRole('button', { name: 'How privacy release works' })
  await helpButton.click()
  const dialog = page.getByRole('dialog', { name: 'Privacy Release' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByRole('button', { name: 'Close help article' })).toBeFocused()

  await page.keyboard.press('Shift+Tab')
  await expect.poll(() => dialog.evaluate((node) => node.contains(document.activeElement))).toBe(true)
  await expect(helpButton).not.toBeFocused()

  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await expect(helpButton).toBeFocused()
})

test('has no automated accessibility violations across input modes @a11y', async ({ page }) => {
  await page.goto('/')
  await expectNoAccessibilityViolations(page)

  await page.getByRole('tab', { name: 'Paste Data' }).click()
  await page.getByLabel('Pasted data').fill('[{"email":"ada@example.com"}]')
  await expectNoAccessibilityViolations(page)

  await page.getByRole('tab', { name: 'Quick by Data Type' }).click()
  await expectNoAccessibilityViolations(page)

  await page.getByRole('tab', { name: 'CSV File' }).click()
  await page.getByRole('button', { name: 'How privacy release works' }).click()
  await expect(page.getByRole('dialog', { name: 'Privacy Release' })).toBeVisible()
  await expectNoAccessibilityViolations(page)
})

async function installTauriMock(page: Page) {
  await page.addInitScript(() => {
    let settings = {
      schemaVersion: 6,
      themeMode: 'system',
      deterministicDefault: false,
      seed: '',
      overwriteOutput: false,
      sampleRowCount: 100,
      previewSampleCount: 5,
      defaultOutputSuffix: '_private_output',
      dpBudgetEnabled: true,
      dpBudgetLimitEpsilon: 10,
      dpBudgetSpentEpsilon: 0,
      dpBudgetAction: 'block',
      dpReleaseHistory: [],
      rememberLastPaths: true,
      lastInputDirectory: null,
      lastOutputDirectory: null,
      localAiEnabled: false,
      localAiModel: 'gemma3:4b',
    }
    let previewAttempts = 0

    window.__CSV_ANONYMIZER_TEST_CALLS__ = []
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          window.__CSV_ANONYMIZER_COPIED_TEXT__ = text
        },
      },
    })
    window.__CSV_ANONYMIZER_TEST_INVOKE__ = async (command, args) => {
      window.__CSV_ANONYMIZER_TEST_CALLS__?.push({ command, args })

      if (command === 'load_settings') return settings
      if (command === 'save_settings') {
        settings = (args?.settings ?? settings) as typeof settings
        return settings
      }
      if (command === 'get_local_ai_status') {
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
        }
      }
      if (command === 'preflight_anonymization') {
        const request = args?.request as { mode?: string; columns?: unknown[] } | undefined
        return {
          mode: request?.mode ?? 'preview',
          readiness: {
            status: 'verified',
            blockers: [],
            reviewItems: [],
            verifiedItems: [`${request?.columns?.length ?? 0} column(s) selected.`],
          },
          evidence: [],
          columnReports: [],
        }
      }
      if (command === 'pick_input_csv') return '/data/input.csv'
      if (command === 'pick_output_csv') return '/data/custom-output.csv'
      if (command === 'analyze_csv') {
        return {
          headers: {
            filePath: '/data/input.csv',
            rowCount: 150_000,
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
        }
      }
      if (command === 'count_csv_rows') return 150_000
      if (command === 'preview_anonymization') {
        previewAttempts += 1
        if (previewAttempts === 1) throw new Error('Preview failed from e2e')
        return {
          previews: [
            {
              columnIndex: 0,
              columnName: 'email',
              samples: [{ original: 'alice@example.test', anonymized: 'anon@example.test' }],
            },
          ],
          warnings: [],
          smartReplacements: [],
        }
      }
      if (command === 'analyze_pasted_data') {
        return {
          format: 'json',
          rowCount: 1,
          rowCountIsComplete: true,
          columns: [columnFixture(0, '[].email', 'email', 'high')],
        }
      }
      if (command === 'preview_pasted_data') {
        return {
          previews: [
            {
              columnIndex: 0,
              columnName: '[].email',
              samples: [{ original: 'ada@example.com', anonymized: 'anon@example.test' }],
            },
          ],
          warnings: [],
          smartReplacements: [],
        }
      }
      if (command === 'anonymize_pasted_data') {
        return {
          output: '[{"email":"anon@example.test"}]',
          rowCount: 1,
          columnsAnonymized: 1,
          durationMs: 4,
          privacyReport: privacyReportFixture(),
        }
      }
      if (command === 'generate_quick_values') {
        return {
          output: 'tok_e2e_1\ntok_e2e_2',
          rowCount: 2,
          values: [
            {
              original: '550e8400-e29b-41d4-a716-446655440000',
              anonymized: 'tok_e2e_1',
            },
            {
              original: '550e8400-e29b-41d4-a716-446655440001',
              anonymized: 'tok_e2e_2',
            },
          ],
          privacyReport: privacyReportFixture(),
        }
      }
      if (command === 'start_anonymize_job') {
        return {
          jobId: 'job-e2e',
          state: 'running',
          rowsProcessed: 0,
          totalRows: 150_000,
          cancelRequested: false,
          result: null,
          error: null,
        }
      }
      if (command === 'get_anonymize_job_status') {
        return {
          jobId: 'job-e2e',
          state: 'running',
          rowsProcessed: 10,
          totalRows: 150_000,
          cancelRequested: false,
          result: null,
          error: null,
        }
      }
      if (command === 'cancel_anonymize_job') {
        return {
          jobId: 'job-e2e',
          state: 'canceled',
          rowsProcessed: 10,
          totalRows: 150_000,
          cancelRequested: true,
          result: null,
          error: null,
        }
      }

      throw new Error(`Unhandled invoke: ${command}`)
    }

    function columnFixture(index: number, name: string, detectedType: string, piiRisk: string) {
      return {
        name,
        index,
        detectedType,
        confidence: 'high',
        piiRisk,
        sampleValues: ['sample'],
        emptyFormat: 'emptyString',
        isSelected: true,
        strategy: 'auto',
      }
    }

    function privacyReportFixture() {
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
      }
    }
  })
}

async function expectNoAccessibilityViolations(page: Page) {
  await expect(page.locator('html')).toHaveAttribute('data-resolved-theme', /^(light|dark)$/)

  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze()
  expect(
    results.violations.map((violation) => ({
      id: violation.id,
      impact: violation.impact,
      targets: violation.nodes.map((node) => node.target),
    })),
  ).toEqual([])
}
