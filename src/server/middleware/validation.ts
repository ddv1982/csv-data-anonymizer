/**
 * Zod Validation Middleware
 * Provides reusable middleware for validating request bodies using Zod schemas.
 */

import type { Request, Response, NextFunction, RequestHandler } from 'express';
import { type ZodSchema, type z, ZodError } from 'zod';
import { ErrorCodes } from '../../types/errors.js';

/**
 * Validation error response format
 */
interface ValidationErrorResponse {
  success: false;
  error: {
    code: string;
    message: string;
    details: Array<{
      path: string;
      message: string;
    }>;
  };
}

/**
 * Formats Zod validation issues into error details
 */
function formatZodIssues(issues: ZodError['issues']): ValidationErrorResponse['error']['details'] {
  return issues.map((issue) => ({
    path: issue.path.length > 0 ? issue.path.join('.') : 'body',
    message: issue.message,
  }));
}

/**
 * Creates validation middleware for the given Zod schema.
 * Validates request body and attaches typed data to req.body.
 *
 * @param schema - Zod schema to validate against
 * @returns Express middleware that validates the request body
 *
 * @example
 * ```typescript
 * const schema = z.object({ filePath: z.string().min(1) });
 * router.post('/headers', validateRequest(schema), (req, res) => {
 *   // req.body is now typed as z.infer<typeof schema>
 *   const { filePath } = req.body;
 * });
 * ```
 */
export function validateRequest<T extends ZodSchema>(
  schema: T
): RequestHandler {
  return (req: Request, res: Response, next: NextFunction): void => {
    const result = schema.safeParse(req.body);

    if (!result.success) {
      const response: ValidationErrorResponse = {
        success: false,
        error: {
          code: ErrorCodes.CONFIG_INVALID,
          message: 'Validation failed',
          details: formatZodIssues(result.error.issues),
        },
      };

      res.status(400).json(response);
      return;
    }

    // Replace body with parsed/coerced data
    req.body = result.data;
    next();
  };
}

/**
 * Type helper to extract the inferred type from a Zod schema
 */
export type ValidatedBody<T extends ZodSchema> = z.infer<T>;
