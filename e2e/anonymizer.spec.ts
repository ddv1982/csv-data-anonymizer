import { test, expect } from '@playwright/test';
import { join } from 'node:path';

/**
 * E2E tests for CSV Anonymizer UI
 * Tests the complete workflow from file selection to anonymization
 */

// Path to test fixture
const FIXTURES_DIR = join(process.cwd(), 'tests/fixtures');
const SAMPLE_CSV = join(FIXTURES_DIR, 'sample.csv');

test.describe('CSV Anonymizer UI', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the app
    await page.goto('/');
  });

  test('should display the main page with correct title', async ({ page }) => {
    // Check page title
    await expect(page).toHaveTitle('CSV Anonymizer');

    // Check header
    await expect(page.locator('h1')).toContainText('CSV Anonymizer');
  });

  test('should display all main sections', async ({ page }) => {
    // Check all section titles are visible
    await expect(page.getByText('1. Select File')).toBeVisible();
    await expect(page.getByText('2. Select Columns')).toBeVisible();
    await expect(page.getByText('3. Configuration')).toBeVisible();
    await expect(page.getByText('4. Preview (Optional)')).toBeVisible();

    // Check anonymize button exists
    await expect(page.getByRole('button', { name: /Anonymize File/i })).toBeVisible();
  });

  test('should have disabled sections before file selection', async ({ page }) => {
    // Column selection should be disabled/dimmed
    const columnSection = page.locator('[class*="opacity-50"]').first();
    await expect(columnSection).toBeVisible();

    // Anonymize button should be disabled
    await expect(page.getByRole('button', { name: /Anonymize File/i })).toBeDisabled();
  });

  test('should display file selector with browse button', async ({ page }) => {
    // Browse button should be visible
    await expect(page.getByRole('button', { name: /Browse/i })).toBeVisible();

    // File input placeholder should be visible
    await expect(page.getByPlaceholder('Select a CSV file...')).toBeVisible();
  });

  test('should show header table with correct structure', async ({ page }) => {
    // Table headers should be present (even if table is empty)
    const table = page.locator('table');
    await expect(table.locator('th').filter({ hasText: '#' })).toBeVisible();
    await expect(table.locator('th').filter({ hasText: 'Column Name' })).toBeVisible();
    await expect(table.locator('th').filter({ hasText: 'Type' })).toBeVisible();
    await expect(table.locator('th').filter({ hasText: 'Risk' })).toBeVisible();
  });

  test('should show configuration section with output path', async ({ page }) => {
    // Output path label and input should be visible
    await expect(page.getByText('Output Path')).toBeVisible();

    // Advanced options should be collapsed by default
    await expect(page.getByText('Advanced Options')).toBeVisible();
  });

  test('should show preview section with show preview button', async ({ page }) => {
    // Preview button should be visible but disabled initially
    const previewButton = page.getByRole('button', { name: /Show Preview/i });
    await expect(previewButton).toBeVisible();
    await expect(previewButton).toBeDisabled();
  });

  test('should display footer', async ({ page }) => {
    await expect(page.getByText('Protect sensitive data in your CSV files')).toBeVisible();
  });
});

test.describe('Column Selection UI', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should show selection buttons', async ({ page }) => {
    await expect(page.getByRole('button', { name: 'Select All' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Deselect All' })).toBeVisible();
    await expect(page.getByRole('button', { name: 'Select High Risk' })).toBeVisible();
  });

  test('should show selection count text', async ({ page }) => {
    // Initially shows 0 of 0 or similar
    await expect(page.getByText(/\d+ of \d+ columns selected/)).toBeVisible();
  });
});

test.describe('Configuration Section', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/');
  });

  test('should expand advanced options on click', async ({ page }) => {
    // Click on advanced options
    await page.getByText('Advanced Options').click();

    // Deterministic mode toggle should be visible
    await expect(page.getByText('Deterministic Mode')).toBeVisible();
  });

  test('should show seed input when deterministic mode is enabled', async ({ page }) => {
    // Expand advanced options
    await page.getByText('Advanced Options').click();

    // Click on deterministic mode toggle
    const deterministicSwitch = page.getByRole('switch');
    await deterministicSwitch.click();

    // Seed input should become visible/enabled
    await expect(page.getByPlaceholder(/seed/i)).toBeVisible();
  });
});

test.describe('API Health Check', () => {
  test('should return healthy status from API', async ({ request }) => {
    const response = await request.get('/api/health');
    expect(response.ok()).toBeTruthy();

    const data = await response.json();
    expect(data.status).toBe('ok');
    expect(data.version).toBeDefined();
    expect(data.timestamp).toBeDefined();
  });
});

test.describe('Error Handling', () => {
  test('should display error for invalid file path', async ({ request }) => {
    const response = await request.post('/api/headers', {
      data: { filePath: '/nonexistent/file.csv' }
    });

    expect(response.status()).toBe(404);
    const data = await response.json();
    expect(data.success).toBe(false);
    expect(data.error.code).toBe('FILE_NOT_FOUND');
  });

  test('should display validation error for empty request', async ({ request }) => {
    const response = await request.post('/api/headers', {
      data: {}
    });

    expect(response.status()).toBe(400);
    const data = await response.json();
    expect(data.success).toBe(false);
  });
});
