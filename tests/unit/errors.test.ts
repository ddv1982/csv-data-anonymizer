import { describe, it, expect } from 'vitest';
import {
  ErrorCodes,
  AnonymizerError,
  FileNotFoundError,
  CsvParseError,
  ConfigValidationError,
  ColumnNotFoundError,
  OutputExistsError,
  InvalidSelectionError,
} from '../../src/types/errors.js';
import type { ZodIssue } from 'zod';

describe('ErrorCodes', () => {
  it('should have all expected error codes', () => {
    expect(ErrorCodes.FILE_NOT_FOUND).toBe('FILE_NOT_FOUND');
    expect(ErrorCodes.CSV_PARSE_ERROR).toBe('CSV_PARSE_ERROR');
    expect(ErrorCodes.CONFIG_INVALID).toBe('CONFIG_INVALID');
    expect(ErrorCodes.COLUMN_NOT_FOUND).toBe('COLUMN_NOT_FOUND');
    expect(ErrorCodes.OUTPUT_EXISTS).toBe('OUTPUT_EXISTS');
    expect(ErrorCodes.INVALID_SELECTION).toBe('INVALID_SELECTION');
  });
});

describe('AnonymizerError', () => {
  it('should create error with code and message', () => {
    const error = new AnonymizerError('Test error', ErrorCodes.FILE_NOT_FOUND);
    expect(error.message).toBe('Test error');
    expect(error.code).toBe('FILE_NOT_FOUND');
    expect(error.name).toBe('AnonymizerError');
    expect(error.suggestion).toBeUndefined();
  });

  it('should create error with suggestion', () => {
    const error = new AnonymizerError('Test error', ErrorCodes.FILE_NOT_FOUND, 'Try again');
    expect(error.suggestion).toBe('Try again');
  });

  it('should be instanceof Error', () => {
    const error = new AnonymizerError('Test', ErrorCodes.FILE_NOT_FOUND);
    expect(error).toBeInstanceOf(Error);
    expect(error).toBeInstanceOf(AnonymizerError);
  });

  it('should format user message without suggestion', () => {
    const error = new AnonymizerError('Test error', ErrorCodes.FILE_NOT_FOUND);
    expect(error.toUserMessage()).toBe('Error [FILE_NOT_FOUND]: Test error');
  });

  it('should format user message with suggestion', () => {
    const error = new AnonymizerError('Test error', ErrorCodes.FILE_NOT_FOUND, 'Try this');
    expect(error.toUserMessage()).toContain('Test error');
    expect(error.toUserMessage()).toContain('Suggestion: Try this');
  });
});

describe('FileNotFoundError', () => {
  it('should create error with path', () => {
    const error = new FileNotFoundError('/path/to/file.csv');
    expect(error.message).toBe('File not found: /path/to/file.csv');
    expect(error.code).toBe('FILE_NOT_FOUND');
    expect(error.name).toBe('FileNotFoundError');
    expect(error.path).toBe('/path/to/file.csv');
  });

  it('should include recovery suggestion', () => {
    const error = new FileNotFoundError('/path/to/file.csv');
    expect(error.suggestion).toContain('Check the file path');
  });

  it('should be instanceof AnonymizerError', () => {
    const error = new FileNotFoundError('/path');
    expect(error).toBeInstanceOf(AnonymizerError);
    expect(error).toBeInstanceOf(FileNotFoundError);
  });
});

describe('CsvParseError', () => {
  it('should create error without row number', () => {
    const error = new CsvParseError('Invalid format');
    expect(error.message).toBe('CSV parse error: Invalid format');
    expect(error.code).toBe('CSV_PARSE_ERROR');
    expect(error.name).toBe('CsvParseError');
    expect(error.row).toBeUndefined();
  });

  it('should create error with row number', () => {
    const error = new CsvParseError('Unexpected quote', 42);
    expect(error.message).toBe('CSV parse error at row 42: Unexpected quote');
    expect(error.row).toBe(42);
  });

  it('should include row-specific suggestion when row is provided', () => {
    const error = new CsvParseError('error', 10);
    expect(error.suggestion).toContain('row 10');
  });

  it('should include general suggestion when row is not provided', () => {
    const error = new CsvParseError('error');
    expect(error.suggestion).toContain('CSV file is properly formatted');
  });

  it('should be instanceof AnonymizerError', () => {
    const error = new CsvParseError('error');
    expect(error).toBeInstanceOf(AnonymizerError);
    expect(error).toBeInstanceOf(CsvParseError);
  });
});

describe('ConfigValidationError', () => {
  it('should create error from Zod issues', () => {
    const issues: ZodIssue[] = [
      {
        code: 'invalid_type',
        expected: 'string',
        received: 'number',
        path: ['columns', 0, 'name'],
        message: 'Expected string, received number',
      },
    ];
    const error = new ConfigValidationError(issues);
    expect(error.code).toBe('CONFIG_INVALID');
    expect(error.name).toBe('ConfigValidationError');
    expect(error.issues).toEqual(issues);
  });

  it('should format issues in message', () => {
    const issues: ZodIssue[] = [
      {
        code: 'too_small',
        minimum: 1,
        type: 'string',
        inclusive: true,
        exact: false,
        path: ['columns', 0, 'name'],
        message: 'Column name cannot be empty',
      },
      {
        code: 'invalid_enum_value',
        options: ['email', 'uuid'],
        received: 'invalid',
        path: ['columns', 0, 'type'],
        message: 'Invalid type',
      },
    ];
    const error = new ConfigValidationError(issues);
    expect(error.message).toContain('columns.0.name');
    expect(error.message).toContain('columns.0.type');
    expect(error.message).toContain('Column name cannot be empty');
    expect(error.message).toContain('Invalid type');
  });

  it('should handle root-level issues', () => {
    const issues: ZodIssue[] = [
      {
        code: 'invalid_type',
        expected: 'object',
        received: 'null',
        path: [],
        message: 'Expected object, received null',
      },
    ];
    const error = new ConfigValidationError(issues);
    expect(error.message).toContain('root');
  });

  it('should include recovery suggestion', () => {
    const issues: ZodIssue[] = [
      { code: 'custom', path: ['test'], message: 'error' },
    ];
    const error = new ConfigValidationError(issues);
    expect(error.suggestion).toContain('configuration file');
  });

  it('should be instanceof AnonymizerError', () => {
    const error = new ConfigValidationError([]);
    expect(error).toBeInstanceOf(AnonymizerError);
    expect(error).toBeInstanceOf(ConfigValidationError);
  });
});

describe('ColumnNotFoundError', () => {
  it('should create error with column name', () => {
    const error = new ColumnNotFoundError('email_address');
    expect(error.message).toBe('Column not found: "email_address"');
    expect(error.code).toBe('COLUMN_NOT_FOUND');
    expect(error.name).toBe('ColumnNotFoundError');
    expect(error.column).toBe('email_address');
  });

  it('should include default suggestion without available columns', () => {
    const error = new ColumnNotFoundError('col');
    expect(error.suggestion).toContain('--preview');
  });

  it('should include available columns in suggestion when provided', () => {
    const error = new ColumnNotFoundError('col', ['id', 'name', 'email']);
    expect(error.suggestion).toContain('id');
    expect(error.suggestion).toContain('name');
    expect(error.suggestion).toContain('email');
    expect(error.availableColumns).toEqual(['id', 'name', 'email']);
  });

  it('should be instanceof AnonymizerError', () => {
    const error = new ColumnNotFoundError('col');
    expect(error).toBeInstanceOf(AnonymizerError);
    expect(error).toBeInstanceOf(ColumnNotFoundError);
  });
});

describe('OutputExistsError', () => {
  it('should create error with output path', () => {
    const error = new OutputExistsError('/path/to/output.csv');
    expect(error.message).toBe('Output file already exists: /path/to/output.csv');
    expect(error.code).toBe('OUTPUT_EXISTS');
    expect(error.name).toBe('OutputExistsError');
    expect(error.outputPath).toBe('/path/to/output.csv');
  });

  it('should include suggestion about --force flag', () => {
    const error = new OutputExistsError('/path');
    expect(error.suggestion).toContain('--force');
    expect(error.suggestion).toContain('--output');
  });

  it('should be instanceof AnonymizerError', () => {
    const error = new OutputExistsError('/path');
    expect(error).toBeInstanceOf(AnonymizerError);
    expect(error).toBeInstanceOf(OutputExistsError);
  });
});

describe('InvalidSelectionError', () => {
  it('should create error with input and reason', () => {
    const error = new InvalidSelectionError('abc', 'not a number');
    expect(error.message).toBe('Invalid column selection "abc": not a number');
    expect(error.code).toBe('INVALID_SELECTION');
    expect(error.name).toBe('InvalidSelectionError');
    expect(error.input).toBe('abc');
  });

  it('should include selection format examples in suggestion', () => {
    const error = new InvalidSelectionError('invalid', 'bad format');
    expect(error.suggestion).toContain('1,3,5');
    expect(error.suggestion).toContain('1-5');
    expect(error.suggestion).toContain('all');
    expect(error.suggestion).toContain('none');
  });

  it('should be instanceof AnonymizerError', () => {
    const error = new InvalidSelectionError('x', 'reason');
    expect(error).toBeInstanceOf(AnonymizerError);
    expect(error).toBeInstanceOf(InvalidSelectionError);
  });
});
