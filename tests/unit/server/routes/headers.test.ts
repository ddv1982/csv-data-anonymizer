/**
 * Headers Route Tests
 */

import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { join } from 'node:path';
import { createServer } from '../../../../src/server/index.js';

const FIXTURES_DIR = join(process.cwd(), 'tests/fixtures');

describe('POST /api/headers', () => {
  const app = createServer();

  describe('Validation Errors', () => {
    it('should return 400 for empty request body', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({});

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('CONFIG_INVALID');
      expect(res.body.error.message).toBe('Validation failed');
      expect(res.body.error.details).toBeDefined();
      expect(Array.isArray(res.body.error.details)).toBe(true);
    });

    it('should return 400 for missing filePath', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ otherField: 'value' });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('CONFIG_INVALID');
    });

    it('should return 400 for empty filePath', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: '' });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.details).toContainEqual(
        expect.objectContaining({
          path: expect.any(String),
          message: expect.any(String),
        })
      );
    });
  });

  describe('File Not Found', () => {
    it('should return 404 for non-existent file', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: '/nonexistent/path/file.csv' });

      expect(res.status).toBe(404);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('FILE_NOT_FOUND');
    });

    it('should return 404 with helpful suggestion', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: '/missing/file.csv' });

      expect(res.status).toBe(404);
      expect(res.body.error.suggestion).toBeDefined();
    });
  });

  describe('Successful Requests', () => {
    it('should return headers for valid CSV file', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: join(FIXTURES_DIR, 'sample.csv') });

      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toBeDefined();
      expect(res.body.data.columns).toBeDefined();
      expect(Array.isArray(res.body.data.columns)).toBe(true);
    });

    it('should include file path in response', async () => {
      const filePath = join(FIXTURES_DIR, 'sample.csv');
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath });

      expect(res.status).toBe(200);
      expect(res.body.data.filePath).toContain('sample.csv');
    });

    it('should include row count in response', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: join(FIXTURES_DIR, 'sample.csv') });

      expect(res.status).toBe(200);
      expect(res.body.data.rowCount).toBeGreaterThan(0);
      expect(typeof res.body.data.rowCount).toBe('number');
    });

    it('should return column metadata with required fields', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: join(FIXTURES_DIR, 'sample.csv') });

      expect(res.status).toBe(200);

      const columns = res.body.data.columns;
      expect(columns.length).toBeGreaterThan(0);

      // Check first column has all required fields
      const firstColumn = columns[0];
      expect(firstColumn.index).toBeDefined();
      expect(firstColumn.name).toBeDefined();
      expect(firstColumn.detectedType).toBeDefined();
      expect(firstColumn.confidence).toBeDefined();
      expect(firstColumn.piiRisk).toBeDefined();
      expect(firstColumn.sampleValues).toBeDefined();
      expect(Array.isArray(firstColumn.sampleValues)).toBe(true);
    });

    it('should detect email columns as high PII risk', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: join(FIXTURES_DIR, 'sample.csv') });

      expect(res.status).toBe(200);

      const emailColumn = res.body.data.columns.find(
        (col: { name: string }) => col.name.toLowerCase() === 'email'
      );

      expect(emailColumn).toBeDefined();
      expect(emailColumn.detectedType).toBe('email');
      expect(emailColumn.piiRisk).toBe('high');
    });
  });

  describe('Error Response Format', () => {
    it('should return consistent error format for all errors', async () => {
      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: '/nonexistent.csv' });

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
