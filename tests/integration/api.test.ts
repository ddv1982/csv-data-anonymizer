/**
 * API Integration Tests
 * Tests the complete workflow: headers → preview → anonymize
 */

import { describe, it, expect, beforeAll, afterAll, afterEach } from 'vitest';
import request from 'supertest';
import { join } from 'node:path';
import { writeFileSync, unlinkSync, existsSync, readFileSync } from 'node:fs';
import { createServer } from '../../src/server/index.js';

const FIXTURES_DIR = join(process.cwd(), 'tests/fixtures');
const TEST_CSV_PATH = join(FIXTURES_DIR, 'test-integration.csv');
const TEST_OUTPUT_PATH = join(FIXTURES_DIR, 'test-integration-output.csv');

// Test CSV content
const TEST_CSV_CONTENT = `id,email,name,phone
1,john@example.com,John Doe,+1-555-123-4567
2,jane@test.org,Jane Smith,+1-555-987-6543
3,bob@company.net,Bob Wilson,+44-20-7946-0958
4,alice@domain.io,Alice Jones,+61-2-9876-5432
5,charlie@web.com,Charlie Brown,+1-555-456-7890`;

describe('API Integration', () => {
  const app = createServer();

  beforeAll(() => {
    // Create test CSV file
    writeFileSync(TEST_CSV_PATH, TEST_CSV_CONTENT);
  });

  afterAll(() => {
    // Clean up test files
    if (existsSync(TEST_CSV_PATH)) {
      unlinkSync(TEST_CSV_PATH);
    }
    if (existsSync(TEST_OUTPUT_PATH)) {
      unlinkSync(TEST_OUTPUT_PATH);
    }
  });

  afterEach(() => {
    // Clean up output file after each test
    if (existsSync(TEST_OUTPUT_PATH)) {
      unlinkSync(TEST_OUTPUT_PATH);
    }
  });

  describe('Complete Workflow: headers → preview → anonymize', () => {
    it('should complete full anonymization workflow', async () => {
      // Step 1: Get headers to understand file structure
      const headersRes = await request(app)
        .post('/api/headers')
        .send({ filePath: TEST_CSV_PATH });

      expect(headersRes.status).toBe(200);
      expect(headersRes.body.success).toBe(true);

      const columns = headersRes.body.data.columns;
      expect(columns).toHaveLength(4);
      expect(columns.map((c: { name: string }) => c.name)).toEqual(['id', 'email', 'name', 'phone']);

      // Find high-risk columns
      const emailCol = columns.find((c: { name: string }) => c.name === 'email');
      expect(emailCol.piiRisk).toBe('high');
      expect(emailCol.detectedType).toBe('email');

      const phoneCol = columns.find((c: { name: string }) => c.name === 'phone');
      expect(phoneCol.detectedType).toBe('phone');

      // Step 2: Preview anonymization for email and phone columns
      const previewRes = await request(app)
        .post('/api/preview')
        .send({
          filePath: TEST_CSV_PATH,
          columns: [emailCol.index, phoneCol.index],
          sampleCount: 3,
        });

      expect(previewRes.status).toBe(200);
      expect(previewRes.body.success).toBe(true);
      expect(previewRes.body.data.previews).toHaveLength(2);

      // Verify preview shows original and anonymized values
      const emailPreview = previewRes.body.data.previews.find(
        (p: { columnName: string }) => p.columnName === 'email'
      );
      expect(emailPreview.samples.length).toBeGreaterThan(0);
      expect(emailPreview.samples[0].original).toContain('@');
      expect(emailPreview.samples[0].anonymized).toContain('@');
      expect(emailPreview.samples[0].original).not.toBe(emailPreview.samples[0].anonymized);

      // Step 3: Anonymize the file
      const anonymizeRes = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_CSV_PATH,
          outputPath: TEST_OUTPUT_PATH,
          columns: [emailCol.index, phoneCol.index],
        });

      expect(anonymizeRes.status).toBe(200);
      expect(anonymizeRes.body.success).toBe(true);
      expect(anonymizeRes.body.data.rowCount).toBe(5);
      expect(anonymizeRes.body.data.columnsAnonymized).toBe(2);

      // Verify output file was created
      expect(existsSync(TEST_OUTPUT_PATH)).toBe(true);

      // Verify output file content
      const outputContent = readFileSync(TEST_OUTPUT_PATH, 'utf-8');
      expect(outputContent).toContain('id,email,name,phone'); // Headers preserved
      expect(outputContent).not.toContain('john@example.com'); // Email anonymized
      expect(outputContent).toContain('John Doe'); // Name preserved (not selected)
    });
  });

  describe('Deterministic Mode Consistency', () => {
    it('should produce same results with same seed', async () => {
      const seed = 'integration-test-seed-12345';

      // Run anonymization twice with same seed
      const res1 = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_CSV_PATH,
          outputPath: TEST_OUTPUT_PATH,
          columns: [1], // email column
          deterministic: true,
          seed,
          force: true,
        });

      expect(res1.status).toBe(200);
      const output1 = readFileSync(TEST_OUTPUT_PATH, 'utf-8');

      const res2 = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_CSV_PATH,
          outputPath: TEST_OUTPUT_PATH,
          columns: [1],
          deterministic: true,
          seed,
          force: true,
        });

      expect(res2.status).toBe(200);
      const output2 = readFileSync(TEST_OUTPUT_PATH, 'utf-8');

      // Same seed should produce identical output
      expect(output1).toBe(output2);
    });

    it('should produce different results with different seeds', async () => {
      const res1 = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_CSV_PATH,
          outputPath: TEST_OUTPUT_PATH,
          columns: [1],
          deterministic: true,
          seed: 'seed-alpha',
          force: true,
        });

      expect(res1.status).toBe(200);
      const output1 = readFileSync(TEST_OUTPUT_PATH, 'utf-8');

      const res2 = await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_CSV_PATH,
          outputPath: TEST_OUTPUT_PATH,
          columns: [1],
          deterministic: true,
          seed: 'seed-beta',
          force: true,
        });

      expect(res2.status).toBe(200);
      const output2 = readFileSync(TEST_OUTPUT_PATH, 'utf-8');

      // Different seeds should produce different output
      expect(output1).not.toBe(output2);
    });
  });

  describe('Error Handling Across Workflow', () => {
    it('should handle invalid file gracefully across all endpoints', async () => {
      const invalidPath = '/nonexistent/path/file.csv';

      // Headers endpoint
      const headersRes = await request(app)
        .post('/api/headers')
        .send({ filePath: invalidPath });
      expect(headersRes.status).toBe(404);
      expect(headersRes.body.success).toBe(false);

      // Preview endpoint
      const previewRes = await request(app)
        .post('/api/preview')
        .send({ filePath: invalidPath, columns: [0] });
      expect(previewRes.status).toBe(404);
      expect(previewRes.body.success).toBe(false);

      // Anonymize endpoint
      const anonymizeRes = await request(app)
        .post('/api/anonymize')
        .send({ filePath: invalidPath, outputPath: TEST_OUTPUT_PATH, columns: [0] });
      expect(anonymizeRes.status).toBe(404);
      expect(anonymizeRes.body.success).toBe(false);
    });

    it('should have consistent error response format', async () => {
      const invalidPath = '/nonexistent.csv';

      const responses = await Promise.all([
        request(app).post('/api/headers').send({ filePath: invalidPath }),
        request(app).post('/api/preview').send({ filePath: invalidPath, columns: [0] }),
        request(app).post('/api/anonymize').send({
          filePath: invalidPath,
          outputPath: TEST_OUTPUT_PATH,
          columns: [0]
        }),
      ]);

      // All should have same error format
      for (const res of responses) {
        expect(res.body).toEqual({
          success: false,
          error: {
            code: expect.any(String),
            message: expect.any(String),
            suggestion: expect.any(String),
          },
        });
      }
    });
  });

  describe('Health Check Integration', () => {
    it('should always respond regardless of other operations', async () => {
      // Health check should work before any file operations
      const res1 = await request(app).get('/api/health');
      expect(res1.status).toBe(200);
      expect(res1.body.status).toBe('ok');

      // Perform some operations
      await request(app).post('/api/headers').send({ filePath: TEST_CSV_PATH });

      // Health check should still work after operations
      const res2 = await request(app).get('/api/health');
      expect(res2.status).toBe(200);
      expect(res2.body.status).toBe('ok');
    });
  });

  describe('File Preservation', () => {
    it('should not modify input file during anonymization', async () => {
      const originalContent = readFileSync(TEST_CSV_PATH, 'utf-8');

      await request(app)
        .post('/api/anonymize')
        .send({
          filePath: TEST_CSV_PATH,
          outputPath: TEST_OUTPUT_PATH,
          columns: [1, 3],
        });

      const afterContent = readFileSync(TEST_CSV_PATH, 'utf-8');
      expect(afterContent).toBe(originalContent);
    });
  });
});
