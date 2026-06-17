/**
 * Health Route Tests
 */

import { describe, it, expect } from 'vitest';
import request from 'supertest';
import { createServer } from '../../../../src/server/index.js';

describe('GET /api/health', () => {
  const app = createServer();

  it('should return 200 with health status', async () => {
    const res = await request(app).get('/api/health');

    expect(res.status).toBe(200);
    expect(res.body.status).toBe('ok');
  });

  it('should include version in response', async () => {
    const res = await request(app).get('/api/health');

    expect(res.body.version).toBeDefined();
    expect(typeof res.body.version).toBe('string');
    expect(res.body.version).toMatch(/^\d+\.\d+\.\d+$/);
  });

  it('should include timestamp in ISO format', async () => {
    const res = await request(app).get('/api/health');

    expect(res.body.timestamp).toBeDefined();
    expect(typeof res.body.timestamp).toBe('string');
    // Verify it's a valid ISO date
    const date = new Date(res.body.timestamp);
    expect(date.toISOString()).toBe(res.body.timestamp);
  });

  it('should have correct response shape', async () => {
    const res = await request(app).get('/api/health');

    expect(res.body).toEqual({
      status: 'ok',
      version: expect.any(String),
      timestamp: expect.any(String),
    });
  });
});
