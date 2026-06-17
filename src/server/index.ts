/**
 * Express Server Module
 * Creates and configures the Express server for the CSV Anonymizer UI.
 */

import express, { type Express, type Request, type Response, type NextFunction } from 'express';
import cors from 'cors';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { existsSync } from 'node:fs';

import { healthRouter } from './routes/health.js';
import { headersRouter } from './routes/headers.js';
import { previewRouter } from './routes/preview.js';
import { anonymizeRouter } from './routes/anonymize.js';
import { errorHandler } from './middleware/errorHandler.js';

/**
 * Request timeout in milliseconds (5 minutes for large file processing)
 */
const REQUEST_TIMEOUT_MS = 5 * 60 * 1000;

/**
 * Get the directory name for ESM modules
 */
const __dirname = fileURLToPath(new URL('.', import.meta.url));

/**
 * Path to the Vue UI build directory
 */
const UI_DIST_PATH = join(__dirname, '../../ui/dist');

/**
 * Cache control durations
 */
const CACHE_CONTROL = {
  // Hashed assets can be cached for 1 year (immutable)
  ASSETS: 'public, max-age=31536000, immutable',
  // HTML should be revalidated on each request
  HTML: 'no-cache',
  // Index file should not be cached to ensure fresh app loads
  INDEX: 'no-store, no-cache, must-revalidate',
};

/**
 * CORS configuration for localhost origins
 */
const corsOptions: cors.CorsOptions = {
  origin: [
    'http://localhost:3456',
    'http://localhost:5173', // Vite dev server
    'http://127.0.0.1:3456',
    'http://127.0.0.1:5173',
  ],
  methods: ['GET', 'POST'],
  allowedHeaders: ['Content-Type'],
};

/**
 * Middleware to set cache headers for static assets
 */
function cacheControl(req: Request, res: Response, next: NextFunction): void {
  const path = req.path;

  // Hashed assets in /assets/ directory - long-term cache
  if (path.startsWith('/assets/')) {
    res.setHeader('Cache-Control', CACHE_CONTROL.ASSETS);
  }
  // HTML files - no cache
  else if (path.endsWith('.html')) {
    res.setHeader('Cache-Control', CACHE_CONTROL.HTML);
  }
  // Other static files (favicon, etc.) - short cache
  else if (path.includes('.')) {
    res.setHeader('Cache-Control', 'public, max-age=3600');
  }

  next();
}

/**
 * Creates and configures the Express application.
 *
 * @returns Configured Express application
 */
export function createServer(): Express {
  const app = express();

  // Middleware
  app.use(cors(corsOptions));
  app.use(express.json({ limit: '1mb' }));

  // Request timeout middleware for API routes
  app.use('/api', (_req: Request, res: Response, next: NextFunction) => {
    res.setTimeout(REQUEST_TIMEOUT_MS, () => {
      res.status(408).json({
        success: false,
        error: {
          code: 'REQUEST_TIMEOUT',
          message: 'Request timed out',
          suggestion: 'The operation took too long. Try with a smaller file or use the CLI for large files.',
        },
      });
    });
    next();
  });

  // API routes
  app.use('/api/health', healthRouter);
  app.use('/api/headers', headersRouter);
  app.use('/api/preview', previewRouter);
  app.use('/api/anonymize', anonymizeRouter);

  // Serve Vue build in production if it exists
  if (existsSync(UI_DIST_PATH)) {
    // Apply cache control middleware for static files
    app.use(cacheControl);

    // Serve static files from UI dist directory
    app.use(express.static(UI_DIST_PATH, {
      // Enable etag for cache validation
      etag: true,
      // Don't set Last-Modified for hashed assets
      lastModified: true,
      // Don't redirect directories to trailing slash
      redirect: false,
    }));

    // SPA fallback routing - all non-API routes serve index.html
    // Express 5 uses path-to-regexp v8 which requires named parameters for wildcards
    app.get('/{*path}', (req: Request, res: Response) => {
      // Skip if it's an API route (use req to avoid unused variable warning)
      const requestPath = req.path;
      if (requestPath.startsWith('/api')) {
        res.status(404).json({
          success: false,
          error: {
            code: 'NOT_FOUND',
            message: `API endpoint not found: ${requestPath}`,
          },
        });
        return;
      }

      // Set no-cache for SPA index to ensure fresh app loads
      res.setHeader('Cache-Control', CACHE_CONTROL.INDEX);
      res.sendFile(join(UI_DIST_PATH, 'index.html'));
    });
  } else {
    // No UI build available - return a helpful message
    app.get('/', (_req: Request, res: Response) => {
      res.status(503).json({
        success: false,
        error: {
          code: 'UI_NOT_AVAILABLE',
          message: 'Web UI is not yet built',
          suggestion: 'Run "npm run build:ui" to build the Vue UI',
        },
      });
    });
  }

  // Error handling middleware (must be last)
  app.use(errorHandler);

  return app;
}

/**
 * Starts the Express server on the specified port.
 *
 * @param port - Port number to listen on (default: 3456)
 * @param host - Host address to bind to (default: localhost)
 * @returns Promise that resolves with the HTTP server instance
 */
export function startServer(
  port: number = 3456,
  host: string = 'localhost'
): Promise<ReturnType<Express['listen']>> {
  return new Promise((resolve) => {
    const app = createServer();

    const server = app.listen(port, host, () => {
      resolve(server);
    });
  });
}

export { type Express };
