/**
 * Health Check Route
 * Returns server status, version, and timestamp.
 */

import { Router, type Request, type Response } from 'express';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = fileURLToPath(new URL('.', import.meta.url));

/**
 * Get version from package.json
 */
function getVersion(): string {
  try {
    const packageJsonPath = join(__dirname, '../../../package.json');
    const packageJson = JSON.parse(readFileSync(packageJsonPath, 'utf-8'));
    return packageJson.version || '1.0.0';
  } catch {
    return '1.0.0';
  }
}

/**
 * Health check response format
 */
interface HealthResponse {
  status: 'ok';
  version: string;
  timestamp: string;
}

const router = Router();

/**
 * GET /api/health
 * Returns server health status
 */
router.get('/', (_req: Request, res: Response<HealthResponse>) => {
  const response: HealthResponse = {
    status: 'ok',
    version: getVersion(),
    timestamp: new Date().toISOString(),
  };

  res.status(200).json(response);
});

export { router as healthRouter };
