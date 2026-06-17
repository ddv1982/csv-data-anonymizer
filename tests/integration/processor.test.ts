import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { processFile } from '../../src/core/processor.js';
import { readSample } from '../../src/core/sampleReader.js';
import { buildColumnMetadata, applyColumnSelection } from '../../src/core/metadataBuilder.js';
import type { ColumnMetadata } from '../../src/types/column.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, '..', 'fixtures');

describe('processor', () => {
  const sampleCsvPath = path.join(fixturesDir, 'sample.csv');
  const outputPath = path.join(fixturesDir, 'output-test.csv');
  const edgeCasesPath = path.join(fixturesDir, 'edge-cases.csv');
  const edgeOutputPath = path.join(fixturesDir, 'edge-output-test.csv');
  const largeCsvPath = path.join(fixturesDir, 'large.csv');
  const largeOutputPath = path.join(fixturesDir, 'large-output-test.csv');

  afterAll(async () => {
    // Clean up output files
    try {
      await fs.unlink(outputPath);
    } catch {
      // Ignore if not created
    }
    try {
      await fs.unlink(edgeOutputPath);
    } catch {
      // Ignore if not created
    }
    try {
      await fs.unlink(largeOutputPath);
    } catch {
      // Ignore if not created
    }
  });

  describe('processFile', () => {
    it('should process a CSV file successfully', async () => {
      const sample = await readSample(sampleCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      // Select email column for anonymization
      columns = applyColumnSelection(columns, [1]); // email is index 1

      const result = await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: false,
        seed: '',
      });

      expect(result.success).toBe(true);
      expect(result.rowCount).toBe(5); // 5 data rows
      expect(result.outputPath).toBe(outputPath);
      expect(result.duration).toBeGreaterThanOrEqual(0);
    });

    it('should create output file', async () => {
      const sample = await readSample(sampleCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      columns = applyColumnSelection(columns, [1]);

      await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: false,
        seed: '',
      });

      // Verify file exists
      const exists = await fs.access(outputPath).then(() => true).catch(() => false);
      expect(exists).toBe(true);
    });

    it('should preserve header row unchanged', async () => {
      const sample = await readSample(sampleCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      columns = applyColumnSelection(columns, [1]);

      await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: false,
        seed: '',
      });

      // Read output and check header
      const outputSample = await readSample(outputPath, 1);
      expect(outputSample.headers).toEqual(sample.headers);
    });

    it('should transform selected columns', async () => {
      const sample = await readSample(sampleCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      // Select email column (index 1)
      columns = applyColumnSelection(columns, [1]);

      await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: false,
        seed: '',
      });

      // Read output and check email column
      const outputSample = await readSample(outputPath, 100);

      // Email should be transformed but still have same domain
      outputSample.rows.forEach((row, index) => {
        const originalEmail = sample.rows[index][1];
        const transformedEmail = row[1];

        // Domain should be preserved
        const originalDomain = originalEmail.split('@')[1];
        expect(transformedEmail).toContain(`@${originalDomain}`);

        // But username should be different
        const originalUsername = originalEmail.split('@')[0];
        const transformedUsername = transformedEmail.split('@')[0];
        expect(transformedUsername).not.toBe(originalUsername);
      });
    });

    it('should leave unselected columns unchanged', async () => {
      const sample = await readSample(sampleCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      // Only select email (index 1)
      columns = applyColumnSelection(columns, [1]);

      await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: false,
        seed: '',
      });

      const outputSample = await readSample(outputPath, 100);

      // Check that non-selected columns are unchanged
      outputSample.rows.forEach((row, index) => {
        // id (index 0) should be unchanged
        expect(row[0]).toBe(sample.rows[index][0]);
        // country (index 4) should be unchanged
        expect(row[4]).toBe(sample.rows[index][4]);
      });
    });

    it('should produce deterministic output with same seed', async () => {
      const sample = await readSample(sampleCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      columns = applyColumnSelection(columns, [1]);

      // Process twice with same seed
      await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: true,
        seed: 'test-seed-123',
      });
      const output1 = await readSample(outputPath, 100);

      await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: true,
        seed: 'test-seed-123',
      });
      const output2 = await readSample(outputPath, 100);

      // Outputs should be identical
      expect(output1.rows).toEqual(output2.rows);
    });

    it('should call progress callback', async () => {
      const sample = await readSample(sampleCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      columns = applyColumnSelection(columns, [1]);

      let progressCalled = false;
      let lastRowCount = 0;

      await processFile(sampleCsvPath, outputPath, columns, {
        deterministic: false,
        seed: '',
        onProgress: (rowCount) => {
          progressCalled = true;
          lastRowCount = rowCount;
        },
      });

      // Progress should be called at least once (at end)
      expect(progressCalled).toBe(true);
      expect(lastRowCount).toBe(5);
    });

    it('should handle edge cases in CSV', async () => {
      const sample = await readSample(edgeCasesPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      // Don't select any columns for transformation (just test parsing/writing)
      columns = applyColumnSelection(columns, []);

      const result = await processFile(edgeCasesPath, edgeOutputPath, columns, {
        deterministic: false,
        seed: '',
      });

      expect(result.success).toBe(true);

      // Read output and verify structure preserved
      const outputSample = await readSample(edgeOutputPath, 100);

      // Should have same number of rows
      expect(outputSample.rows.length).toBe(sample.rows.length);

      // Values with commas, quotes, newlines should be preserved
      expect(outputSample.rows[1][1]).toBe('Value with, comma');
      expect(outputSample.rows[2][1]).toBe('Value with "quotes"');
    });

    it('should handle large files with streaming (10,000+ rows)', async () => {
      const sample = await readSample(largeCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      // Select email column (index 1) for anonymization
      columns = applyColumnSelection(columns, [1]);

      const progressCounts: number[] = [];

      const result = await processFile(largeCsvPath, largeOutputPath, columns, {
        deterministic: true,
        seed: 'large-file-seed',
        onProgress: (rowCount) => {
          progressCounts.push(rowCount);
        },
      });

      // Verify success
      expect(result.success).toBe(true);
      expect(result.rowCount).toBeGreaterThanOrEqual(10000);

      // Verify output file exists
      const exists = await fs.access(largeOutputPath).then(() => true).catch(() => false);
      expect(exists).toBe(true);

      // Verify progress was reported for large files (every 10000 rows)
      expect(progressCounts.length).toBeGreaterThan(0);

      // Verify last progress equals final row count
      expect(progressCounts[progressCounts.length - 1]).toBe(result.rowCount);
    }, 30000); // 30 second timeout for large file

    it('should maintain memory stability during large file processing', async () => {
      const sample = await readSample(largeCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      columns = applyColumnSelection(columns, [1]);

      // Get initial memory usage
      const initialMemory = process.memoryUsage().heapUsed;

      const result = await processFile(largeCsvPath, largeOutputPath, columns, {
        deterministic: false,
        seed: '',
      });

      // Get final memory usage
      const finalMemory = process.memoryUsage().heapUsed;
      const memoryIncrease = finalMemory - initialMemory;

      // Memory increase should be reasonable (less than 50MB for 10k rows)
      // This is a loose check since exact memory is hard to predict
      expect(memoryIncrease).toBeLessThan(50 * 1024 * 1024);
      expect(result.success).toBe(true);
    }, 30000);

    it('should produce correct output for large files', async () => {
      const sample = await readSample(largeCsvPath, 100);
      let columns = buildColumnMetadata(sample.headers, sample.rows);
      columns = applyColumnSelection(columns, [1]); // email

      await processFile(largeCsvPath, largeOutputPath, columns, {
        deterministic: true,
        seed: 'verify-seed',
      });

      // Read a sample of the output to verify correctness
      const outputSample = await readSample(largeOutputPath, 10);

      // Headers should match
      expect(outputSample.headers).toEqual(sample.headers);

      // Verify emails are transformed but domains preserved
      outputSample.rows.forEach((row, index) => {
        const originalEmail = sample.rows[index][1];
        const transformedEmail = row[1];

        if (originalEmail.includes('@')) {
          const originalDomain = originalEmail.split('@')[1];
          expect(transformedEmail).toContain(`@${originalDomain}`);
        }
      });
    }, 30000);
  });
});
