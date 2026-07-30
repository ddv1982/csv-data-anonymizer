import AxeBuilder from '@axe-core/playwright'
import { expect, test, type Page } from '@playwright/test'
import { defaultSettings } from '../src/defaults'
import {
  columnMetadataFixture,
  completeDetectionCoverage,
  localAiStatusFixture,
  privacyReportFixture,
  verifiedPreflightFixture,
} from '../src/test-utils/builders'
import type { AppSettings, ColumnMetadata } from '../src/types'

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

test('covers disabled states, simplified column review, and glossary help', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByLabel('Output Path')).toBeDisabled()
  await expect(page.getByRole('button', { name: 'Create protected CSV' })).toBeDisabled()

  await page.getByRole('button', { name: 'Browse for CSV file' }).click()
  await expect(page.getByLabel('Output Path')).toBeEnabled()
  await expect(page.getByRole('checkbox', { name: 'Select column email' })).toBeChecked()
  await expect(page.getByRole('heading', { name: '2. Review Sensitive Columns' })).toBeVisible()
  await expect(page.getByLabel('Privacy release mode')).toHaveCount(0)
  await expect(page.getByText('2 of 3 columns selected, 150,000 rows loaded')).toBeVisible()

  await page.getByRole('button', { name: 'How does this work?' }).click()
  const helpDialog = page.getByRole('dialog', { name: 'Review Sensitive Columns' })
  await expect(helpDialog).toBeVisible()
  await expect(helpDialog).toContainText('Defaults')
  await expect(helpDialog).toContainText('Review signals')
  await expect(helpDialog).toContainText('Methods')
  await expect(helpDialog).toContainText('Run behavior')
  await expect(helpDialog).toContainText('current run')

  await helpDialog.getByRole('button', { name: 'Pseudonymize', exact: true }).first().click()
  await expect(page.getByRole('tooltip')).toContainText('Pseudonymize')

  await page.keyboard.press('Escape')
  await expect(page.getByRole('tooltip')).toBeHidden()
  await expect(page.getByRole('dialog', { name: 'Review Sensitive Columns' })).toBeVisible()

  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog', { name: 'Review Sensitive Columns' })).toBeHidden()
})

test('recovers from preview errors and cancels a running job', async ({ page }) => {
  await page.goto('/')
  await page.getByRole('button', { name: 'Browse for CSV file' }).click()

  await page.getByRole('button', { name: 'Show Preview' }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'Preview failed from e2e' })).toBeVisible()
  await page.getByRole('button', { name: 'Dismiss error message' }).click()

  await page.getByRole('button', { name: 'Show Preview' }).click()
  await expect(page.getByText('anon@example.test')).toBeVisible()

  await page.getByRole('button', { name: 'Create protected CSV' }).click()
  await expect(page.getByRole('status')).toContainText('Preparing 150,000 rows')

  await page.getByRole('button', { name: 'Cancel' }).click()
  await expect(page.getByRole('alert').filter({ hasText: 'Output creation canceled.' })).toBeVisible()
})

test('switches tabs, pastes JSON, copies output, and quick-generates values', async ({ page }) => {
  await page.goto('/')

  await expect(page.getByRole('button', { name: 'Browse for CSV file' })).toBeVisible()

  await page.getByRole('tab', { name: 'Paste Sample' }).click()
  await expect(page.getByRole('button', { name: 'Browse for CSV file' })).toBeHidden()
  await page.getByLabel('Pasted data').fill('[{"email":"ada@example.com"}]')
  await page.keyboard.press('Tab')
  await expect(page.getByText('Detected: JSON')).toBeVisible()
  await expect(page.getByText('[].email')).toBeVisible()

  await page.getByRole('button', { name: 'Show Preview' }).click()
  await expect(page.getByText('anon@example.test')).toBeVisible()

  await page.getByRole('button', { name: 'Transform pasted sample' }).click()
  await expect(page.getByLabel('Anonymized pasted data')).toHaveValue('[{"email":"anon@example.test"}]')
  await page.getByRole('button', { name: 'Copy' }).click()
  await expect(page.getByText('Copied')).toBeVisible()
  await expect.poll(() => page.evaluate(() => window.__CSV_ANONYMIZER_COPIED_TEXT__)).toBe('[{"email":"anon@example.test"}]')
  await page.getByRole('button', { name: 'Clear' }).click()
  await expect(page.getByLabel('Pasted data')).toHaveValue('')
  await expect(page.getByText('[].email')).toBeHidden()

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
  const pasteTab = page.getByRole('tab', { name: 'Paste Sample' })
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

  await page.getByRole('button', { name: 'Browse for CSV file' }).click()
  await page.getByLabel('Strategy for email').selectOption('localAi')
  const localAiSettingsButton = page
    .getByRole('alert')
    .filter({ hasText: 'Set up Local AI' })
    .getByRole('button', { name: 'Open Local AI settings' })
  await localAiSettingsButton.click()
  const localAiDialog = page.getByRole('dialog', { name: 'Local AI Settings' })
  await expect(localAiDialog).toBeVisible()
  await expect(localAiDialog.getByRole('button', { name: 'Close Local AI settings' })).toBeFocused()

  await page.keyboard.press('Escape')
  await expect(localAiDialog).toBeHidden()
  await expect(localAiSettingsButton).toBeFocused()

  const helpButton = page.getByRole('button', { name: 'How does this work?' })
  await helpButton.click()
  const dialog = page.getByRole('dialog', { name: 'Review Sensitive Columns' })
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

  await page.getByRole('tab', { name: 'Paste Sample' }).click()
  await page.getByLabel('Pasted data').fill('[{"email":"ada@example.com"}]')
  await expectNoAccessibilityViolations(page)

  await page.getByRole('tab', { name: 'Quick by Data Type' }).click()
  await expectNoAccessibilityViolations(page)

  await page.getByRole('tab', { name: 'CSV File' }).click()
  await page.getByRole('button', { name: 'How does this work?' }).click()
  await expect(page.getByRole('dialog', { name: 'Review Sensitive Columns' })).toBeVisible()
  await expectNoAccessibilityViolations(page)
})

/**
 * Everything the in-page mock answers with, built out of the same builders the unit tests use.
 *
 * It has to be built out here and passed in: the init script body is serialised and evaluated
 * inside the browser, where it can close over nothing from this module. Only the data crosses
 * the boundary — the dispatch below stays in the script, because it is the part that has to
 * read the arguments of each call.
 */
function buildE2eFixtures() {
  const columnFixture = (
    index: number,
    name: string,
    detectedType: ColumnMetadata['detectedType'],
    piiRisk: ColumnMetadata['piiRisk'],
  ): ColumnMetadata =>
    columnMetadataFixture({
      name,
      index,
      detectedType,
      piiRisk,
      isSelected: true,
      // The workflow's own default, not the risk-derived one: these specs drive the strategy
      // selects by hand, and starting a column on Redact would change what they are choosing
      // between.
      strategy: 'auto',
    })

  const runningJob = {
    jobId: 'job-e2e',
    state: 'running',
    rowsProcessed: 0,
    totalRows: 150_000,
    cancelRequested: false,
    result: null,
    error: null,
  }

  return {
    settings: defaultSettings,
    localAiStatus: localAiStatusFixture(),
    // The readiness of this one is filled in from the request it answers, since the specs read
    // back the column count it was asked about.
    preflight: verifiedPreflightFixture(),
    csvHeaders: {
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
    pasteColumns: [columnFixture(0, '[].email', 'email', 'high')],
    // A one-value paste is examined whole, so this fixture exercises the complete-coverage
    // branch: no partial-detection warning should render.
    pasteDetectionCoverage: { ...completeDetectionCoverage, unit: 'values' as const },
    // A one-row paste has nothing to single out, so `rowUniqueness` stays at the builder's
    // absent default: the joint re-identifiability block must not render.
    privacyReport: privacyReportFixture({
      directIdentifiers: 1,
      pseudonymizedColumns: 1,
      uniquePseudonymValues: 1,
    }),
    runningJob,
    pollingJob: { ...runningJob, rowsProcessed: 10 },
    canceledJob: { ...runningJob, state: 'canceled', rowsProcessed: 10, cancelRequested: true },
  }
}

async function installTauriMock(page: Page) {
  await page.addInitScript((fixtures) => {
    let settings: AppSettings = fixtures.settings
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
      if (command === 'get_local_ai_status') return fixtures.localAiStatus
      if (command === 'preflight_anonymization') {
        const request = args?.request as { mode?: string; columns?: unknown[] } | undefined
        return {
          ...fixtures.preflight,
          mode: request?.mode ?? 'preview',
          readiness: {
            ...fixtures.preflight.readiness,
            verifiedItems: [`${request?.columns?.length ?? 0} column(s) selected.`],
          },
        }
      }
      if (command === 'pick_input_csv') return '/data/input.csv'
      if (command === 'pick_output_csv') return '/data/custom-output.csv'
      if (command === 'analyze_csv') {
        return {
          headers: fixtures.csvHeaders,
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
          detectionCoverage: fixtures.pasteDetectionCoverage,
          columns: fixtures.pasteColumns,
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
          privacyReport: fixtures.privacyReport,
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
          privacyReport: fixtures.privacyReport,
        }
      }
      if (command === 'start_anonymize_job') return fixtures.runningJob
      if (command === 'get_anonymize_job_status') return fixtures.pollingJob
      if (command === 'cancel_anonymize_job') return fixtures.canceledJob

      throw new Error(`Unhandled invoke: ${command}`)
    }
  }, buildE2eFixtures())
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
