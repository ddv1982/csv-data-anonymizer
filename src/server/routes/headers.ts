/**
 * Headers Route
 * Analyzes a CSV file and returns column metadata.
 */

import { Router, type Request, type Response, type NextFunction } from 'express';
import { z } from 'zod';
import { resolve, normalize, isAbsolute } from 'node:path';

import { validateRequest } from '../middleware/validation.js';
import { readSample, readAllRows } from '../../core/sampleReader.js';
import { buildColumnMetadata } from '../../core/metadataBuilder.js';
import { AnonymizerError, ErrorCodes } from '../../types/errors.js';
import type { ColumnMetadata, Confidence, PiiRisk, DataType } from '../../types/column.js';

/**
 * Request schema for /api/headers
 */
const HeadersRequestSchema = z.object({
  filePath: z.string().min(1, 'File path is required'),
});

/**
 * Column info in response
 */
interface ColumnInfo {
  index: number;
  name: string;
  detectedType: DataType;
  confidence: Confidence;
  piiRisk: PiiRisk;
  sampleValues: string[];
}

/**
 * Response format for /api/headers
 */
interface HeadersResponse {
  success: true;
  data: {
    filePath: string;
    rowCount: number;
    columns: ColumnInfo[];
  };
}

/**
 * Number of sample rows to read for analysis
 */
const SAMPLE_ROW_COUNT = 100;

/**
 * Validates file path for security (prevents path traversal attacks)
 */
function validateFilePath(filePath: string): string {
  // Check for obvious traversal attempts in raw input
  if (filePath.includes('\0')) {
    throw new AnonymizerError(
      'Invalid file path: null bytes not allowed',
      ErrorCodes.FILE_NOT_FOUND,
      'Provide a valid file path.'
    );
  }

  const resolved = isAbsolute(filePath)
    ? normalize(filePath)
    : resolve(process.cwd(), filePath);

  const normalized = normalize(resolved);

  // After normalization, the path should not contain '..' segments
  // normalize() resolves '..' so if it still appears, something is wrong
  const segments = normalized.split(/[/\\]/);
  if (segments.includes('..')) {
    throw new AnonymizerError(
      'Invalid file path: path traversal detected',
      ErrorCodes.FILE_NOT_FOUND,
      'Provide an absolute path or a path relative to the current working directory.'
    );
  }

  return normalized;
}

/**
 * Converts internal ColumnMetadata to API response format
 */
function toColumnInfo(column: ColumnMetadata): ColumnInfo {
  return {
    index: column.index,
    name: column.name,
    detectedType: column.detectedType,
    confidence: column.confidence,
    piiRisk: column.piiRisk,
    sampleValues: column.sampleValues,
  };
}

const router = Router();

/**
 * POST /api/headers
 * Analyzes a CSV file and returns column metadata
 */
router.post(
  '/',
  validateRequest(HeadersRequestSchema),
  async (
    req: Request<unknown, unknown, z.infer<typeof HeadersRequestSchema>>,
    res: Response<HeadersResponse>,
    next: NextFunction
  ): Promise<void> => {
    try {
      const { filePath } = req.body;

      // Validate and normalize file path
      const normalizedPath = validateFilePath(filePath);

      // Read sample rows for analysis
      const sample = await readSample(normalizedPath, SAMPLE_ROW_COUNT);

      // Build column metadata
      const metadata = buildColumnMetadata(sample.headers, sample.rows);

      // Get approximate row count by reading all rows (with limit)
      let rowCount = sample.rows.length;
      try {
        const allRows = await readAllRows(normalizedPath);
        rowCount = allRows.rows.length;
      } catch {
        // If we can't read all rows, use sample count
        rowCount = sample.rows.length;
      }

      // Convert to response format
      const columns = metadata.map(toColumnInfo);

      const response: HeadersResponse = {
        success: true,
        data: {
          filePath: normalizedPath,
          rowCount,
          columns,
        },
      };

      res.status(200).json(response);
    } catch (error) {
      next(error);
    }
  }
);

export { router as headersRouter };
