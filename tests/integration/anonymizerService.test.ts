import { mkdtemp, rm } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { AnonymizerService } from '../../src/main/services/anonymizerService';

describe('AnonymizerService', () => {
  let tempDir: string;
  let service: AnonymizerService;
  const samplePath = join(process.cwd(), 'tests/fixtures/sample.csv');

  beforeEach(async () => {
    tempDir = await mkdtemp(join(tmpdir(), 'csv-anonymizer-service-'));
    service = new AnonymizerService('test-version');
  });

  afterEach(async () => {
    await rm(tempDir, { recursive: true, force: true });
  });

  it('analyzes CSV headers and returns default desktop output path', async () => {
    const result = await service.analyzeCsv({ filePath: samplePath });

    expect(result.filePath).toBe(samplePath);
    expect(result.rowCount).toBe(5);
    expect(result.defaultOutputPath).toContain('_anonymized.csv');
    expect(result.columns.map((column) => column.name)).toEqual([
      'id',
      'email',
      'user_uuid',
      'created_at',
      'country',
      'status',
      'name',
    ]);
  });

  it('generates deterministic previews through the service API', async () => {
    const first = await service.previewAnonymization({
      filePath: samplePath,
      columns: [1],
      deterministic: true,
      seed: 'preview-seed',
      sampleCount: 2,
    });
    const second = await service.previewAnonymization({
      filePath: samplePath,
      columns: [1],
      deterministic: true,
      seed: 'preview-seed',
      sampleCount: 2,
    });

    expect(first).toEqual(second);
    expect(first.previews[0].columnName).toBe('email');
    expect(first.previews[0].samples).toHaveLength(2);
  });

  it('anonymizes selected columns without the CLI or HTTP server', async () => {
    const outputPath = join(tempDir, 'sample-anonymized.csv');

    const result = await service.anonymizeCsv({
      filePath: samplePath,
      outputPath,
      columns: [1],
      deterministic: true,
      seed: 'service-seed',
      force: false,
    });

    expect(result.outputPath).toBe(outputPath);
    expect(result.rowCount).toBe(5);
    expect(result.columnsAnonymized).toBe(1);
  });
});
