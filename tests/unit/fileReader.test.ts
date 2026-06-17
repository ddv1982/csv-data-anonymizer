import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { validateFile, stripBom, getFileSize, readFileContent } from '../../src/core/fileReader.js';
import { FileNotFoundError } from '../../src/types/errors.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, '..', 'fixtures');

describe('fileReader', () => {
  const testFilePath = path.join(fixturesDir, 'test-temp.txt');

  beforeAll(async () => {
    // Create a temp test file
    await fs.writeFile(testFilePath, 'test content');
  });

  afterAll(async () => {
    // Clean up temp file
    try {
      await fs.unlink(testFilePath);
    } catch {
      // Ignore if already deleted
    }
  });

  describe('validateFile', () => {
    it('should validate existing file successfully', async () => {
      const result = await validateFile(testFilePath);

      expect(result.valid).toBe(true);
      expect(result.path).toBe(testFilePath);
      expect(result.stats.isReadable).toBe(true);
      expect(result.stats.size).toBeGreaterThan(0);
    });

    it('should throw FileNotFoundError for non-existent file', async () => {
      const nonExistentPath = path.join(fixturesDir, 'non-existent.csv');

      await expect(validateFile(nonExistentPath)).rejects.toThrow(FileNotFoundError);
    });

    it('should throw FileNotFoundError for directory path', async () => {
      await expect(validateFile(fixturesDir)).rejects.toThrow(FileNotFoundError);
    });

    it('should return correct file size', async () => {
      const result = await validateFile(testFilePath);
      const actualSize = (await fs.stat(testFilePath)).size;

      expect(result.stats.size).toBe(actualSize);
    });
  });

  describe('stripBom', () => {
    it('should strip BOM from beginning of string', () => {
      const withBom = '\uFEFFhello';
      const result = stripBom(withBom);

      expect(result).toBe('hello');
    });

    it('should return unchanged string without BOM', () => {
      const noBom = 'hello world';
      const result = stripBom(noBom);

      expect(result).toBe('hello world');
    });

    it('should handle empty string', () => {
      const result = stripBom('');
      expect(result).toBe('');
    });

    it('should only strip BOM at beginning', () => {
      const bomInMiddle = 'hello\uFEFFworld';
      const result = stripBom(bomInMiddle);

      expect(result).toBe('hello\uFEFFworld');
    });
  });

  describe('getFileSize', () => {
    it('should return file size in bytes', async () => {
      const size = await getFileSize(testFilePath);
      const actualSize = (await fs.stat(testFilePath)).size;

      expect(size).toBe(actualSize);
    });

    it('should throw FileNotFoundError for non-existent file', async () => {
      const nonExistentPath = path.join(fixturesDir, 'no-file.csv');

      await expect(getFileSize(nonExistentPath)).rejects.toThrow(FileNotFoundError);
    });
  });

  describe('readFileContent', () => {
    it('should read file content as string', async () => {
      const content = await readFileContent(testFilePath);

      expect(content).toBe('test content');
    });

    it('should strip BOM from file content', async () => {
      const bomFilePath = path.join(fixturesDir, 'bom-file.csv');
      const content = await readFileContent(bomFilePath);

      // First line should start with 'id', not BOM
      expect(content.startsWith('id')).toBe(true);
    });

    it('should throw FileNotFoundError for non-existent file', async () => {
      const nonExistentPath = path.join(fixturesDir, 'missing.csv');

      await expect(readFileContent(nonExistentPath)).rejects.toThrow(FileNotFoundError);
    });
  });
});
