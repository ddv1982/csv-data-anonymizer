import { _electron as electron, expect, test } from '@playwright/test';
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

test('opens the desktop anonymizer shell', async () => {
  const app = await electron.launch({ args: ['.'] });
  const page = await app.firstWindow();

  await expect(page.getByRole('heading', { name: 'CSV Anonymizer' })).toBeVisible();

  await app.close();
});

test('previews and anonymizes small CSVs through the desktop bridge', async () => {
  const tempDir = await mkdtemp(join(tmpdir(), 'csv-anonymizer-e2e-'));
  let app: Awaited<ReturnType<typeof electron.launch>> | undefined;
  const cases = [
    {
      name: 'one-column',
      contents: 'email\nalice@example.com\nbob@example.com\n',
      columns: [0],
    },
    {
      name: 'two-column',
      contents: 'id,email\n1,alice@example.com\n2,bob@example.com\n',
      columns: [1],
    },
  ];

  try {
    app = await electron.launch({ args: ['.'] });
    const page = await app.firstWindow();

    for (const testCase of cases) {
      const inputPath = join(tempDir, `${testCase.name}.csv`);
      const outputPath = join(tempDir, `${testCase.name}-output.csv`);
      await writeFile(inputPath, testCase.contents, 'utf-8');

      const result = await page.evaluate(async ({ filePath, anonymizedPath, columns }) => {
        const api = window.csvAnonymizer;
        if (!api) {
          throw new Error('csvAnonymizer bridge is unavailable');
        }

        const headers = await api.getHeaders({ filePath, sampleRows: 10 });
        const preview = await api.getPreview({
          filePath,
          columns,
          deterministic: true,
          seed: 'e2e-seed',
          sampleCount: 2,
        });
        const anonymized = await api.anonymizeFile({
          filePath,
          outputPath: anonymizedPath,
          columns,
          deterministic: true,
          seed: 'e2e-seed',
          force: false,
        });

        return { headers, preview, anonymized };
      }, { filePath: inputPath, anonymizedPath: outputPath, columns: testCase.columns });

      expect(result.headers.success).toBe(true);
      expect(result.preview.success).toBe(true);
      expect(result.anonymized.success).toBe(true);
      expect(await readFile(outputPath, 'utf-8')).toContain('@example.com');
    }
  } finally {
    await app?.close();
    await rm(tempDir, { recursive: true, force: true });
  }
});
