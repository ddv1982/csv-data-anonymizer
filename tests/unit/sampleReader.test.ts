import { describe, it, expect } from 'vitest';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readSample, readAllRows } from '../../src/core/sampleReader.js';
import { FileNotFoundError } from '../../src/types/errors.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, '..', 'fixtures');

describe('sampleReader', () => {
  const sampleCsvPath = path.join(fixturesDir, 'sample.csv');
  const bomFilePath = path.join(fixturesDir, 'bom-file.csv');
  const edgeCasesPath = path.join(fixturesDir, 'edge-cases.csv');

  describe('readSample', () => {
    it('should read headers correctly', async () => {
      const result = await readSample(sampleCsvPath, 5);

      expect(result.headers).toEqual([
        'id', 'email', 'user_uuid', 'created_at', 'country', 'status', 'name'
      ]);
    });

    it('should read requested number of rows', async () => {
      const result = await readSample(sampleCsvPath, 3);

      expect(result.rows.length).toBe(3);
    });

    it('should stop at end of file if fewer rows than requested', async () => {
      // Sample has 5 data rows
      const result = await readSample(sampleCsvPath, 100);

      expect(result.rows.length).toBe(5);
    });

    it('should handle BOM correctly', async () => {
      const result = await readSample(bomFilePath, 5);

      // Header should not have BOM
      expect(result.headers[0]).toBe('id');
    });

    it('should handle quoted values with commas', async () => {
      const result = await readSample(edgeCasesPath, 5);

      // Row 2 (index 1) has "Value with, comma"
      expect(result.rows[1][1]).toBe('Value with, comma');
    });

    it('should handle quoted values with quotes', async () => {
      const result = await readSample(edgeCasesPath, 5);

      // Row 3 (index 2) has escaped quotes
      expect(result.rows[2][1]).toBe('Value with "quotes"');
    });

    it('should handle newlines in quoted fields', async () => {
      const result = await readSample(edgeCasesPath, 5);

      // Row 4 (index 3) has newline in field
      expect(result.rows[3][1]).toContain('newline');
    });

    it('should throw FileNotFoundError for missing file', async () => {
      const nonExistentPath = path.join(fixturesDir, 'missing.csv');

      await expect(readSample(nonExistentPath, 5)).rejects.toThrow(FileNotFoundError);
    });

    it('should return rows with correct column count', async () => {
      const result = await readSample(sampleCsvPath, 5);

      result.rows.forEach(row => {
        expect(row.length).toBe(result.headers.length);
      });
    });

    it('should trim values when option is set', async () => {
      const result = await readSample(sampleCsvPath, 5, { trimValues: true });

      // Values should not have leading/trailing whitespace
      result.rows.forEach(row => {
        row.forEach(value => {
          expect(value).toBe(value.trim());
        });
      });
    });
  });

  describe('readAllRows', () => {
    it('should read all rows from file', async () => {
      const result = await readAllRows(sampleCsvPath);

      expect(result.headers.length).toBe(7);
      expect(result.rows.length).toBe(5);
    });

    it('should handle BOM in file', async () => {
      const result = await readAllRows(bomFilePath);

      expect(result.headers[0]).toBe('id');
    });

    it('should throw FileNotFoundError for missing file', async () => {
      const nonExistentPath = path.join(fixturesDir, 'missing.csv');

      await expect(readAllRows(nonExistentPath)).rejects.toThrow(FileNotFoundError);
    });
  });
});
