import { expect, test, type Page } from '@playwright/test'

declare global {
  interface Window {
    __CSV_ANONYMIZER_TEST_INVOKE__?: (command: string, args?: Record<string, unknown>) => unknown
    __CSV_ANONYMIZER_TEST_CALLS__?: Array<{ command: string; args?: Record<string, unknown> }>
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

  await helpDialog.getByRole('button', { name: 'k-anonymity', exact: true }).click()
  await expect(page.getByRole('tooltip')).toContainText('k-anonymity')

  await page.keyboard.press('Escape')
  await expect(page.getByRole('tooltip')).toBeHidden()
  await expect(page.getByRole('dialog', { name: 'Privacy Release' })).toBeVisible()

  await page.keyboard.press('Escape')
  await expect(page.getByRole('dialog', { name: 'Privacy Release' })).toBeHidden()
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
  })
}
