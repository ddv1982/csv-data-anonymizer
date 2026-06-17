/**
 * Preview Route Tests
 */

import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { join } from 'node:path';
import { createServer } from '../../../../src/server/index.js';

const FIXTURES_DIR = join(process.cwd(), 'tests/fixtures');

describe('POST /api/preview', () => {
  const app = createServer();

  describe('Validation Errors', () => {
    it('should return 400 for empty request body', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({});

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('CONFIG_INVALID');
    });

    it('should return 400 for missing filePath', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({ columns: [0] });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for missing columns', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({ filePath: join(FIXTURES_DIR, 'sample.csv') });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for empty columns array', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [],
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.details).toContainEqual(
        expect.objectContaining({
          message: expect.stringContaining('At least one column required'),
        })
      );
    });

    it('should return 400 for sampleCount > 10', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [0],
          sampleCount: 11,
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for sampleCount < 1', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [0],
          sampleCount: 0,
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });

    it('should return 400 for negative column index', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [-1],
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
    });
  });

  describe('File Not Found', () => {
    it('should return 404 for non-existent file', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: '/nonexistent/file.csv',
          columns: [0],
        });

      expect(res.status).toBe(404);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('FILE_NOT_FOUND');
    });
  });

  describe('Invalid Column Index', () => {
    it('should return 400 for column index out of range', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [100], // Way out of range
        });

      expect(res.status).toBe(400);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('COLUMN_NOT_FOUND');
    });
  });

  describe('Successful Requests', () => {
    it('should return previews for valid request', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [1], // email column
        });

      expect(res.status).toBe(200);
      expect(res.body.success).toBe(true);
      expect(res.body.data).toBeDefined();
      expect(res.body.data.previews).toBeDefined();
      expect(Array.isArray(res.body.data.previews)).toBe(true);
    });

    it('should include column metadata in previews', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [1], // email column
        });

      expect(res.status).toBe(200);

      const preview = res.body.data.previews[0];
      expect(preview.columnIndex).toBeDefined();
      expect(preview.columnName).toBeDefined();
      expect(preview.samples).toBeDefined();
      expect(Array.isArray(preview.samples)).toBe(true);
    });

    it('should include original and anonymized values in samples', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [1], // email column
          sampleCount: 3,
        });

      expect(res.status).toBe(200);

      const samples = res.body.data.previews[0].samples;
      expect(samples.length).toBeGreaterThan(0);
      expect(samples.length).toBeLessThanOrEqual(3);

      const sample = samples[0];
      expect(sample.original).toBeDefined();
      expect(sample.anonymized).toBeDefined();
      expect(sample.original).not.toBe(sample.anonymized);
    });

    it('should respect sampleCount parameter', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [1],
          sampleCount: 2,
        });

      expect(res.status).toBe(200);

      const samples = res.body.data.previews[0].samples;
      expect(samples.length).toBeLessThanOrEqual(2);
    });

    it('should support multiple columns', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [0, 1, 2], // id, email, uuid columns
        });

      expect(res.status).toBe(200);
      expect(res.body.data.previews.length).toBe(3);
    });

    it('should use defaults for optional parameters', async () => {
      const res = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [1],
        });

      expect(res.status).toBe(200);
      // Default sampleCount is 5
      const samples = res.body.data.previews[0].samples;
      expect(samples.length).toBeLessThanOrEqual(5);
    });

    it('should support deterministic mode', async () => {
      const res1 = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [1],
          deterministic: true,
          seed: 'test-seed-123',
        });

      const res2 = await request(app)
        .post('/api/preview')
        .send({
          filePath: join(FIXTURES_DIR, 'sample.csv'),
          columns: [1],
          deterministic: true,
          seed: 'test-seed-123',
        });

      expect(res1.status).toBe(200);
      expect(res2.status).toBe(200);

      // Same seed should produce same results
      expect(res1.body.data.previews[0].samples[0].anonymized)
        .toBe(res2.body.data.previews[0].samples[0].anonymized);
    });
  });
});
