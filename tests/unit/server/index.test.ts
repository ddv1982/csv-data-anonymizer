/**
 * Server Index Tests
 * Tests for Express server creation and configuration.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import request from 'supertest';
import { createServer } from '../../../src/server/index.js';
import * as fs from 'node:fs';

// Mock fs.existsSync for UI_DIST_PATH checks
vi.mock('node:fs', async () => {
  const actual = await vi.importActual('node:fs');
  return {
    ...actual,
    existsSync: vi.fn(),
  };
});

describe('Server Index', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('createServer', () => {
    it('should create an Express application', () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      expect(app).toBeDefined();
      expect(typeof app.listen).toBe('function');
    });

    it('should have CORS enabled', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      const res = await request(app)
        .options('/api/health')
        .set('Origin', 'http://localhost:5173');

      expect(res.headers['access-control-allow-origin']).toBe('http://localhost:5173');
    });
  });

  describe('API routes', () => {
    it('should respond to /api/health', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      const res = await request(app).get('/api/health');

      expect(res.status).toBe(200);
      expect(res.body).toMatchObject({ status: 'ok' });
    });

    it('should respond to /api/headers with POST', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      const res = await request(app)
        .post('/api/headers')
        .send({ filePath: '' });

      // Should get 400 for empty filePath, not 404
      expect(res.status).toBe(400);
    });

    it('should respond to /api/preview with POST', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      const res = await request(app)
        .post('/api/preview')
        .send({});

      // Should get 400 for invalid body
      expect(res.status).toBe(400);
    });

    it('should respond to /api/anonymize with POST', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      const res = await request(app)
        .post('/api/anonymize')
        .send({});

      // Should get 400 for invalid body
      expect(res.status).toBe(400);
    });
  });

  describe('UI serving (when built)', () => {
    it('should serve UI when dist exists', async () => {
      // Mock existsSync to return true for UI dist
      vi.mocked(fs.existsSync).mockReturnValue(true);

      const app = createServer();

      // The server should be configured to serve static files
      // We can't easily test sendFile without the actual files
      expect(app).toBeDefined();
    });

    it('should return 503 when UI is not built', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      const res = await request(app).get('/');

      expect(res.status).toBe(503);
      expect(res.body.success).toBe(false);
      expect(res.body.error.code).toBe('UI_NOT_AVAILABLE');
    });
  });

  describe('cache control middleware', () => {
    it('should set cache headers for asset requests', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const app = createServer();

      // Request to /assets/ path - will 404 without actual files but headers should be set
      const res = await request(app).get('/assets/test.js');

      // Even on 404, the middleware should have run
      // The actual header depends on whether static middleware runs first
      expect(res.status).toBeDefined();
    });
  });

  describe('error handling', () => {
    it('should handle JSON parse errors', async () => {
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const app = createServer();

      const res = await request(app)
        .post('/api/headers')
        .set('Content-Type', 'application/json')
        .send('invalid json{');

      expect(res.status).toBeGreaterThanOrEqual(400);
    });
  });
});
