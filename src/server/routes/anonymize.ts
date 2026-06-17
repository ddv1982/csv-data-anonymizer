/**
 * Anonymize Route
 * Processes a CSV file and anonymizes selected columns.
 */

import { Router, type Request, type Response, type NextFunction } from 'express';
import { z } from 'zod';
import { resolve, normalize, isAbsolute, dirname } from 'node:path';
import { access, constants } from 'node:fs/promises';
import { existsSync } from 'node:fs';

import { validateRequest } from '../middleware/validation.js';
import { readSample } from '../../core/sampleReader.js';
import { buildColumnMetadata, applyColumnSelection } from '../../core/metadataBuilder.js';
import { processFile } from '../../core/processor.js';
import { AnonymizerError, ErrorCodes, OutputExistsError } from '../../types/errors.js';

/**
 * Request schema for /api/anonymize
 */
const AnonymizeRequestSchema = z.object({
  filePath: z.string().min(1, 'File path is required'),
  outputPath: z.string().min(1, 'Output path is required'),
  columns: z.array(z.number().int().min(0)).min(1, 'At least one column required'),
  deterministic: z.boolean().default(false),
  seed: z.string().optional(),
  force: z.boolean().default(false),
});

/**
 * Response format for /api/anonymize
 */
interface AnonymizeResponse {
  success: true;
  data: {
    outputPath: string;
    rowCount: number;
    columnsAnonymized: number;
    duration: number;
  };
}

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
 * Validates output path for security and writability
 */
async function validateOutputPath(outputPath: string, force: boolean): Promise<string> {
  const normalized = validateFilePath(outputPath);

  // Check if output file already exists
  if (existsSync(normalized) && !force) {
    throw new OutputExistsError(normalized);
  }

  // Check if output directory is writable
  const outputDir = dirname(normalized);
  try {
    await access(outputDir, constants.W_OK);
  } catch {
    throw new AnonymizerError(
      `Output directory is not writable: ${outputDir}`,
      ErrorCodes.FILE_NOT_FOUND,
      'Ensure the output directory exists and you have write permissions.'
    );
  }

  return normalized;
}

const router = Router();

/**
 * POST /api/anonymize
 * Processes a CSV file and anonymizes selected columns
 */
router.post(
  '/',
  validateRequest(AnonymizeRequestSchema),
  async (
    req: Request<unknown, unknown, z.infer<typeof AnonymizeRequestSchema>>,
    res: Response<AnonymizeResponse>,
    next: NextFunction
  ): Promise<void> => {
    try {
      const { filePath, outputPath, columns, deterministic, seed, force } = req.body;

      // Validate and normalize file paths
      const normalizedInputPath = validateFilePath(filePath);
      const normalizedOutputPath = await validateOutputPath(outputPath, force);

      // Read sample to build metadata
      const sample = await readSample(normalizedInputPath, 100);

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

      // Process the file
      const result = await processFile(
        normalizedInputPath,
        normalizedOutputPath,
        selectedMetadata,
        {
          deterministic,
          seed: seed ?? '',
        }
      );

      const response: AnonymizeResponse = {
        success: true,
        data: {
          outputPath: normalizedOutputPath,
          rowCount: result.rowCount,
          columnsAnonymized: columns.length,
          duration: result.duration,
        },
      };

      res.status(200).json(response);
    } catch (error) {
      next(error);
    }
  }
);

export { router as anonymizeRouter };
