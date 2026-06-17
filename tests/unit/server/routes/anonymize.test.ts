/**
 * Anonymize Route Tests
 */

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import request from 'supertest';
import { join } from 'node:path';
import { existsSync, unlinkSync, copyFileSync } from 'node:fs';
import { createServer } from '../../../../src/server/index.js';

const FIXTURES_DIR = join(process.cwd(), 'tests/fixtures');
const TEST_INPUT = join(FIXTURES_DIR, 'sample.csv');
const TEST_OUTPUT = join(FIXTURES_DIR, 'test-output-anonymized.csv');
const TEST_INPUT_COPY = join(FIXTURES_DIR, 'test-input-copy.csv');

describe('POST /api/anonymize', () => {
  const app = createServer();

  // Clean up test files before and after each test
  beforeEach(() => {
    if (existsSync(TEST_OUTPUT)) {
      unlinkSync(TEST_OUTPUT);
    }
    if (existsSync(TEST_INPUT_COPY)) {
      unlinkSync(TEST_INPUT_COPY);
    }
  });

  afterEach(() => {
    if (existsSync(TEST_OUTPUT)) {
      unlinkSync(TEST_OUTPUT);
    }
    if (existsSync(TEST_INPUT_COPY)) {
      unlinkSync(TEST_INPUT_COPY);
    }
  });

  describe('Validation Errors', () => {
    it('should return 400 for empty request body', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({});

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('CONFIG_INVALID');
    });

    it('should return 400 for missing filePath', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          outputPath: TEST_OUTPUT,
          columns: [0],
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for missing outputPath', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          columns: [0],
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for missing columns', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for empty columns array', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [],
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for column index out of range', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [100],
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('COLUMN_NOT_FOUND');
    });
  });

  describe('File Not Found', () => {
    it('should return 404 for non-existent input file', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: '/nonexistent/file.csv',
          outputPath: TEST_OUTPUT,
          columns: [0],
        });

      expect(res.status).toBe(404);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('FILE_NOT_FOUND');
    });
  });

  describe('Output File Exists', () => {
    it('should return 409 if output file exists and force is false', async () => {
      // Create output file first
      copyFileSync(TEST_INPUT, TEST_OUTPUT);

      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [0],
          force: false,
        });

      expect(res.status).toBe(409);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('OUTPUT_EXISTS');
    });

    it('should succeed if output file exists and force is true', async () => {
      // Create output file first
      copyFileSync(TEST_INPUT, TEST_OUTPUT);

      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1], // email column
          force: true,
        });

      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
    });
  });

  describe('Successful Requests', () => {
    it('should anonymize file successfully', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1], // email column
        });

      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toBeDefined();
    });

    it('should return output path in response', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1],
        });

      expect(res.status).toBe(200);
      expect(res.body.data.outputPath).toContain('test-output-anonymized.csv');
    });

    it('should return row count in response', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1],
        });

      expect(res.status).toBe(200);
      expect(res.body.data.rowCount).toBeGreaterThan(0);
      expect(typeof res.body.data.rowCount).toBe('number');
    });

    it('should return columns anonymized count', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1, 2],
        });

      expect(res.status).toBe(200);
      expect(res.body.data.columnsAnonymized).toBe(2);
    });

    it('should return duration in response', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1],
        });

      expect(res.status).toBe(200);
      expect(res.body.data.duration).toBeDefined();
      expect(typeof res.body.data.duration).toBe('number');
      expect(res.body.data.duration).toBeGreaterThanOrEqual(0);
    });

    it('should create output file', async () => {
      expect(existsSync(TEST_OUTPUT)).toBe(false);

      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1],
        });

      expect(res.status).toBe(200);
      expect(existsSync(TEST_OUTPUT)).toBe(true);
    });

    it('should support deterministic mode', async () => {
      const output1 = TEST_OUTPUT;
      const output2 = join(FIXTURES_DIR, 'test-output-2.csv');

      try {
        const res1 = await request(app)
          .post('/api/anonymize')
          .send({
            filePath: TEST_INPUT,
            outputPath: output1,
            columns: [1],
            deterministic: true,
            seed: 'test-seed',
          });

        const res2 = await request(app)
          .post('/api/anonymize')
          .send({
            filePath: TEST_INPUT,
            outputPath: output2,
            columns: [1],
            deterministic: true,
            seed: 'test-seed',
          });

        expect(res1.status).toBe(200);
        expect(res2.status).toBe(200);

        // Both outputs should exist
        expect(existsSync(output1)).toBe(true);
        expect(existsSync(output2)).toBe(true);
      } finally {
        if (existsSync(output2)) {
          unlinkSync(output2);
        }
      }
    });
  });

  describe('Response Format', () => {
    it('should return correct success response shape', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_INPUT,
          outputPath: TEST_OUTPUT,
          columns: [1],
        });

      expect(res.body).toEqual({
        success: true,
        data: {
          outputPath: expect.any(String),
          rowCount: expect.any(Number),
          columnsAnonymized: expect.any(Number),
          duration: expect.any(Number),
        },
      });
    });

    it('should return correct error response shape', async () => {
      const res = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: '/nonexistent.csv',
          outputPath: TEST_OUTPUT,
          columns: [1],
        });

      expect(res.body).toEqual({
        success: false,
        error: {
          code: expect.any(String),
          message: expect.any(String),
          suggestion: expect.any(String),
        },
      });
    });
  });
});
