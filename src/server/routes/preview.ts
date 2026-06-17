/**
 * Preview Route
 * Generates preview transformations for selected columns.
 */

import { Router, type Request, type Response, type NextFunction } from 'express';
import { z } from 'zod';
import { resolve, normalize, isAbsolute } from 'node:path';

import { validateRequest } from '../middleware/validation.js';
import { readSample } from '../../core/sampleReader.js';
import { buildColumnMetadata, applyColumnSelection } from '../../core/metadataBuilder.js';
import { transformValue, createTransformContext } from '../../core/transformer.js';
import { AnonymizerError, ErrorCodes } from '../../types/errors.js';
import type { ColumnMetadata } from '../../types/column.js';

/**
 * Request schema for /api/preview
 */
const PreviewRequestSchema = z.object({
  filePath: z.string().min(1, 'File path is required'),
  columns: z.array(z.number().int().min(0)).min(1, 'At least one column required'),
  deterministic: z.boolean().default(false),
  seed: z.string().nullable().optional(),
  sampleCount: z.number().int().min(1).max(10).default(5),
});

/**
 * Sample transformation result
 */
interface SampleTransform {
  original: string;
  anonymized: string;
}

/**
 * Column preview result
 */
interface ColumnPreview {
  columnIndex: number;
  columnName: string;
  samples: SampleTransform[];
}

/**
 * Response format for /api/preview
 */
interface PreviewResponse {
  success: true;
  data: {
    previews: ColumnPreview[];
  };
}

/**
 * Validates file path for security (prevents path traversal attacks)
 */
function validateFilePath(filePath: string): string {
  const resolved = isAbsolute(filePath)
    ? normalize(filePath)
    : resolve(process.cwd(), filePath);

  const normalized = normalize(resolved);

  // Check for path traversal attempts
  if (normalized.includes('..')) {
    throw new AnonymizerError(
      'Invalid file path: path traversal detected',
      ErrorCodes.FILE_NOT_FOUND,
      'Provide an absolute path or a path relative to the current working directory.'
    );
  }

  return normalized;
}

/**
 * Generates preview samples for a single column
 */
function generateColumnPreview(
  column: ColumnMetadata,
  rows: string[][],
  sampleCount: number,
  deterministic: boolean,
  seed: string
): ColumnPreview {
  const samples: SampleTransform[] = [];

  // Get non-empty values from the column
  const columnValues: Array<{ value: string; rowIndex: number }> = [];
  for (let rowIndex = 0; rowIndex < rows.length && columnValues.length < sampleCount; rowIndex++) {
    const value = rows[rowIndex][column.index];
    if (value && value.trim() !== '' && value.toLowerCase() !== 'null') {
      columnValues.push({ value, rowIndex });
    }
  }

  // Transform each sample value
  for (const { value, rowIndex } of columnValues) {
    const context = createTransformContext(column, rowIndex, seed, deterministic);
    const anonymized = transformValue(value, column, context);
    samples.push({
      original: value,
      anonymized,
    });
  }

  return {
    columnIndex: column.index,
    columnName: column.name,
    samples,
  };
}

const router = Router();

/**
 * POST /api/preview
 * Generates preview transformations for selected columns
 */
router.post(
  '/',
  validateRequest(PreviewRequestSchema),
  async (
    req: Request<unknown, unknown, z.infer<typeof PreviewRequestSchema>>,
    res: Response<PreviewResponse>,
    next: NextFunction
  ): Promise<void> => {
    try {
      const { filePath, columns, deterministic, seed, sampleCount } = req.body;

      // Validate and normalize file path
      const normalizedPath = validateFilePath(filePath);

      // Read enough rows for preview samples
      const sample = await readSample(normalizedPath, sampleCount * 2);

      // Build column metadata
      const metadata = buildColumnMetadata(sample.headers, sample.rows);

      // Validate column indices
      const maxColumnIndex = metadata.length - 1;
      for (const colIndex of columns) {
        if (colIndex > maxColumnIndex) {
          throw new AnonymizerError(
            `Column index ${colIndex} is out of range. Valid range: 0-${maxColumnIndex}`,
            ErrorCodes.COLUMN_NOT_FOUND,
            `Use column indices between 0 and ${maxColumnIndex}`
          );
        }
      }

      // Apply column selection
      const selectedMetadata = applyColumnSelection(metadata, columns);

      // Generate previews for each selected column
      const previews: ColumnPreview[] = [];
      const seedValue = seed ?? '';

      for (const column of selectedMetadata) {
        if (column.isSelected) {
          const preview = generateColumnPreview(
            column,
            sample.rows,
            sampleCount,
            deterministic,
            seedValue
          );
          previews.push(preview);
        }
      }

      const response: PreviewResponse = {
        success: true,
        data: {
          previews,
        },
      };

      res.status(200).json(response);
    } catch (error) {
      next(error);
    }
  }
);

export { router as previewRouter };
