import { _electron as electron, expect } from '@playwright/test';
import { access, mkdtemp, readdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const packageJson = JSON.parse(await readFile(join(process.cwd(), 'package.json'), 'utf-8'));
const releaseDir = join(process.cwd(), 'release', packageJson.version);
const releaseEntries = await readdir(releaseDir, { withFileTypes: true });
const macAppDir = releaseEntries.find(entry => entry.isDirectory() && entry.name.startsWith('mac'));

if (!macAppDir) {
  throw new Error(`No macOS packaged app found in ${releaseDir}. Run pnpm run dist:dir first.`);
}

const appExecutable = join(
  releaseDir,
  macAppDir.name,
  'CSV Anonymizer.app/Contents/MacOS/CSV Anonymizer'
);

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

await access(appExecutable);

const tempDir = await mkdtemp(join(tmpdir(), 'csv-anonymizer-packaged-smoke-'));
let app;

try {
  app = await electron.launch({ executablePath: appExecutable });
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
        seed: 'packaged-smoke',
        sampleCount: 2,
      });
      const anonymized = await api.anonymizeFile({
        filePath,
        outputPath: anonymizedPath,
        columns,
        deterministic: true,
        seed: 'packaged-smoke',
        force: false,
      });

      return { headers, preview, anonymized };
    }, { filePath: inputPath, anonymizedPath: outputPath, columns: testCase.columns });

    if (!result.headers.success || !result.preview.success || !result.anonymized.success) {
      throw new Error(`${testCase.name} packaged smoke failed: ${JSON.stringify(result)}`);
    }

    const output = await readFile(outputPath, 'utf-8');
    if (!output.includes('@example.com')) {
      throw new Error(`${testCase.name} packaged smoke output did not contain anonymized email domain`);
    }

    console.log(`${testCase.name}: ok`);
  }

  const uiInputPath = join(tempDir, 'visible-workflow.csv');
  const uiOutputPath = join(tempDir, 'visible-workflow-output.csv');
  await writeFile(uiInputPath, 'id,email\n1,alice@example.com\n2,bob@example.com\n', 'utf-8');

  await page.getByLabel('File path input').fill(uiInputPath);
  await expect(page.getByText('1 of 2 columns selected')).toBeVisible();
  await page.locator('#output-path').fill(uiOutputPath);

  await page.getByRole('button', { name: 'Show Preview' }).click();
  await expect(page.getByText('alice@example.com')).toBeVisible();

  await page.getByRole('button', { name: 'Anonymize File' }).click();
  await expect(page.getByText('Success!')).toBeVisible();
  await expect(page.getByText('An unexpected error occurred. Please try again.')).toHaveCount(0);

  const uiOutput = await readFile(uiOutputPath, 'utf-8');
  if (!uiOutput.includes('@example.com')) {
    throw new Error('visible-workflow packaged smoke output did not contain anonymized email domain');
  }

  console.log('visible-workflow: ok');
} finally {
  await app?.close();
  await rm(tempDir, { recursive: true, force: true });
}
