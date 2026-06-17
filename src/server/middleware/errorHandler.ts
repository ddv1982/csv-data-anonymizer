/**
 * Error Handling Middleware
 * Catches errors and returns consistent JSON error responses.
 */

import type { Request, Response, NextFunction, ErrorRequestHandler } from 'express';
import {
  AnonymizerError,
  FileNotFoundError,
  CsvParseError,
  ConfigValidationError,
  ColumnNotFoundError,
  OutputExistsError,
  InvalidSelectionError,
  ErrorCodes,
} from '../../types/errors.js';

/**
 * Error response format matching API contract
 */
interface ErrorResponse {
  success: false;
  error: {
    code: string;
    message: string;
    suggestion?: string;
    details?: unknown;
  };
}

/**
 * Maps AnonymizerError codes to HTTP status codes
 */
function getHttpStatusCode(error: Error): number {
  if (error instanceof FileNotFoundError) {
    return 404;
  }

  if (error instanceof CsvParseError) {
    return 400;
  }

  if (error instanceof ConfigValidationError) {
    return 400;
  }

  if (error instanceof ColumnNotFoundError) {
    return 400;
  }

  if (error instanceof OutputExistsError) {
    return 409;
  }

  if (error instanceof InvalidSelectionError) {
    return 400;
  }

  if (error instanceof AnonymizerError) {
    // Default mapping for other AnonymizerError types
    switch (error.code) {
      case ErrorCodes.FILE_NOT_FOUND:
        return 404;
      case ErrorCodes.CSV_PARSE_ERROR:
      case ErrorCodes.CONFIG_INVALID:
      case ErrorCodes.COLUMN_NOT_FOUND:
      case ErrorCodes.INVALID_SELECTION:
        return 400;
      case ErrorCodes.OUTPUT_EXISTS:
        return 409;
      default:
        return 500;
    }
  }

  return 500;
}

/**
 * Creates error response body from an error
 */
function createErrorResponse(error: Error): ErrorResponse {
  if (error instanceof AnonymizerError) {
    return {
      success: false,
      error: {
        code: error.code,
        message: error.message,
        suggestion: error.suggestion,
      },
    };
  }

  // For non-AnonymizerError, return generic error
  // Don't expose internal error details to clients
  return {
    success: false,
    error: {
      code: 'INTERNAL_ERROR',
      message: 'An unexpected error occurred',
      suggestion: 'Please try again or contact support if the problem persists.',
    },
  };
}

/**
 * Express error handling middleware.
 * Catches all errors and returns consistent JSON responses.
 * Stack traces are logged server-side but not exposed to clients.
 */
export const errorHandler: ErrorRequestHandler = (
  error: Error,
  req: Request,
  res: Response,
  _next: NextFunction
): void => {
  // Log error server-side for debugging
  console.error(`[${new Date().toISOString()}] Error handling ${req.method} ${req.path}:`);
  console.error(error.stack ?? error.message);

  const statusCode = getHttpStatusCode(error);
  const response = createErrorResponse(error);

  res.status(statusCode).json(response);
};

/**
 * Validation error class for Zod validation failures
 */
export class ValidationError extends AnonymizerError {
  public readonly details: unknown;

  constructor(message: string, details?: unknown) {
    super(
      message,
      ErrorCodes.CONFIG_INVALID,
      'Check the request body and ensure all required fields are provided.'
    );
    this.name = 'ValidationError';
    this.details = details;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

export { ErrorResponse };
