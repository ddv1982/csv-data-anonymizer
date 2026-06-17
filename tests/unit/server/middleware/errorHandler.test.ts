/**
 * Server Error Handler Middleware Tests
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';
import type { Request, Response, NextFunction } from 'express';
import {
  errorHandler,
  ValidationError,
} from '../../../../src/server/middleware/errorHandler.js';
import {
  AnonymizerError,
  FileNotFoundError,
  CsvParseError,
  ConfigValidationError,
  ColumnNotFoundError,
  OutputExistsError,
  InvalidSelectionError,
  ErrorCodes,
} from '../../../../src/types/errors.js';

describe('Server Error Handler Middleware', () => {
  let mockReq: Partial<Request>;
  let mockRes: Partial<Response>;
  let mockNext: NextFunction;
  let statusCode: number;
  let jsonBody: unknown;

  beforeEach(() => {
    statusCode = 0;
    jsonBody = null;

    mockReq = {
      method: 'POST',
      path: '/api/test',
    };

    mockRes = {
      status: vi.fn().mockImplementation((code: number) => {
        statusCode = code;
        return mockRes;
      }),
      json: vi.fn().mockImplementation((body: unknown) => {
        jsonBody = body;
        return mockRes;
      }),
    };

    mockNext = vi.fn();

    // Suppress console.error during tests
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  describe('HTTP status code mapping', () => {
    it('should return 404 for FileNotFoundError', () => {
      const error = new FileNotFoundError('/missing.csv');

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(404);
    });

    it('should return 400 for CsvParseError', () => {
      const error = new CsvParseError('Invalid CSV', 10);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should return 400 for ConfigValidationError', () => {
      const error = new ConfigValidationError([
        { code: 'invalid_type', path: ['columns'], message: 'Required' } as never,
      ]);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should return 400 for ColumnNotFoundError', () => {
      const error = new ColumnNotFoundError('email', ['name', 'age']);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should return 409 for OutputExistsError', () => {
      const error = new OutputExistsError('/output.csv');

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(409);
    });

    it('should return 400 for InvalidSelectionError', () => {
      const error = new InvalidSelectionError('abc', 'not a number');

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should return 500 for generic AnonymizerError', () => {
      const error = new AnonymizerError('Unknown error', ErrorCodes.CONFIG_INVALID);
      // Override the code to something not mapped
      Object.defineProperty(error, 'code', { value: 'UNKNOWN_CODE' });

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(500);
    });

    it('should return 500 for non-AnonymizerError', () => {
      const error = new Error('Generic error');

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(500);
    });
  });

  describe('error response format', () => {
    it('should return proper structure for AnonymizerError', () => {
      const error = new FileNotFoundError('/test.csv');

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(jsonBody).toEqual({
        success: false,
        error: {
          code: 'FILE_NOT_FOUND',
          message: 'File not found: /test.csv',
          suggestion: 'Check the file path and ensure the file exists.',
        },
      });
    });

    it('should return generic error for non-AnonymizerError', () => {
      const error = new Error('Internal failure');

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(jsonBody).toEqual({
        success: false,
        error: {
          code: 'INTERNAL_ERROR',
          message: 'An unexpected error occurred',
          suggestion: 'Please try again or contact support if the problem persists.',
        },
      });
    });

    it('should not expose stack trace to clients', () => {
      const error = new Error('Secret error');
      error.stack = 'Error at secret/path/file.ts:123';

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      const body = jsonBody as { error: { stack?: string } };
      expect(body.error.stack).toBeUndefined();
    });
  });

  describe('ValidationError class', () => {
    it('should create ValidationError with message and details', () => {
      const error = new ValidationError('Invalid input', { field: 'name' });

      expect(error.message).toBe('Invalid input');
      expect(error.code).toBe('CONFIG_INVALID');
      expect(error.details).toEqual({ field: 'name' });
      expect(error.suggestion).toContain('Check the request body');
    });

    it('should be instanceof AnonymizerError', () => {
      const error = new ValidationError('Test');

      expect(error).toBeInstanceOf(AnonymizerError);
      expect(error).toBeInstanceOf(ValidationError);
    });

    it('should work with errorHandler', () => {
      const error = new ValidationError('Bad request', { missing: 'filePath' });

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
      expect(jsonBody).toMatchObject({
        success: false,
        error: {
          code: 'CONFIG_INVALID',
          message: 'Bad request',
        },
      });
    });
  });

  describe('error code to status mapping via AnonymizerError', () => {
    it('should map FILE_NOT_FOUND code to 404', () => {
      const error = new AnonymizerError('Not found', ErrorCodes.FILE_NOT_FOUND);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(404);
    });

    it('should map CSV_PARSE_ERROR code to 400', () => {
      const error = new AnonymizerError('Parse error', ErrorCodes.CSV_PARSE_ERROR);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should map CONFIG_INVALID code to 400', () => {
      const error = new AnonymizerError('Invalid', ErrorCodes.CONFIG_INVALID);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should map COLUMN_NOT_FOUND code to 400', () => {
      const error = new AnonymizerError('Column missing', ErrorCodes.COLUMN_NOT_FOUND);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should map INVALID_SELECTION code to 400', () => {
      const error = new AnonymizerError('Bad selection', ErrorCodes.INVALID_SELECTION);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(400);
    });

    it('should map OUTPUT_EXISTS code to 409', () => {
      const error = new AnonymizerError('Exists', ErrorCodes.OUTPUT_EXISTS);

      errorHandler(error, mockReq as Request, mockRes as Response, mockNext);

      expect(statusCode).toBe(409);
    });
  });
});
