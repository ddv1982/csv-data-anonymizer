/**
 * Error codes for all anonymizer errors
 */
export const ErrorCodes = {
  FILE_NOT_FOUND: 'FILE_NOT_FOUND',
  CSV_PARSE_ERROR: 'CSV_PARSE_ERROR',
  CONFIG_INVALID: 'CONFIG_INVALID',
  COLUMN_NOT_FOUND: 'COLUMN_NOT_FOUND',
  OUTPUT_EXISTS: 'OUTPUT_EXISTS',
  INVALID_SELECTION: 'INVALID_SELECTION',
} as const;

export type ErrorCode = (typeof ErrorCodes)[keyof typeof ErrorCodes];

/**
 * Base error class for all anonymizer errors.
 * Provides consistent error structure with code, message, and recovery suggestion.
 */
export class AnonymizerError extends Error {
  public readonly code: ErrorCode;
  public readonly suggestion?: string;

  constructor(message: string, code: ErrorCode, suggestion?: string) {
    super(message);
    this.name = 'AnonymizerError';
    this.code = code;
    this.suggestion = suggestion;
    // Maintain proper prototype chain for instanceof checks
    Object.setPrototypeOf(this, new.target.prototype);
  }

  /**
   * Format error for user display
   */
  toUserMessage(): string {
    let msg = `Error [${this.code}]: ${this.message}`;
    if (this.suggestion) {
      msg += `\n\nSuggestion: ${this.suggestion}`;
    }
    return msg;
  }
}

/**
 * Error thrown when the input CSV file cannot be found
 */
export class FileNotFoundError extends AnonymizerError {
  public readonly path: string;

  constructor(path: string) {
    super(
      `File not found: ${path}`,
      ErrorCodes.FILE_NOT_FOUND,
      'Check the file path and ensure the file exists.'
    );
    this.name = 'FileNotFoundError';
    this.path = path;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/**
 * Error thrown when CSV parsing fails
 */
export class CsvParseError extends AnonymizerError {
  public readonly row?: number;

  constructor(message: string, row?: number) {
    const fullMessage = row !== undefined
      ? `CSV parse error at row ${row}: ${message}`
      : `CSV parse error: ${message}`;

    super(
      fullMessage,
      ErrorCodes.CSV_PARSE_ERROR,
      row !== undefined
        ? `Check the CSV format at row ${row}. Ensure proper quoting and escaping.`
        : 'Verify the CSV file is properly formatted with consistent columns.'
    );
    this.name = 'CsvParseError';
    this.row = row;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/**
 * Error thrown when a specified column is not found in the CSV
 */
export class ColumnNotFoundError extends AnonymizerError {
  public readonly column: string;
  public readonly availableColumns?: string[];

  constructor(column: string, availableColumns?: string[]) {
    let suggestion = 'Use --preview to see available columns.';
    if (availableColumns && availableColumns.length > 0) {
      suggestion = `Available columns: ${availableColumns.join(', ')}`;
    } else {
      suggestion = 'Review the detected CSV columns and select an available column.'
    }

    super(
      `Column not found: "${column}"`,
      ErrorCodes.COLUMN_NOT_FOUND,
      suggestion
    );
    this.name = 'ColumnNotFoundError';
    this.column = column;
    this.availableColumns = availableColumns;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/**
 * Error thrown when output file already exists and --force is not used
 */
export class OutputExistsError extends AnonymizerError {
  public readonly outputPath: string;

  constructor(outputPath: string) {
    super(
      `Output file already exists: ${outputPath}`,
      ErrorCodes.OUTPUT_EXISTS,
      'Enable overwrite output or choose a different output path.'
    );
    this.name = 'OutputExistsError';
    this.outputPath = outputPath;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}

/**
 * Error thrown when column selection input is invalid
 */
export class InvalidSelectionError extends AnonymizerError {
  public readonly input: string;

  constructor(input: string, reason: string) {
    super(
      `Invalid column selection "${input}": ${reason}`,
      ErrorCodes.INVALID_SELECTION,
      'Enter comma-separated column numbers (e.g., 1,3,5), ranges (e.g., 1-5), "all", or "none".'
    );
    this.name = 'InvalidSelectionError';
    this.input = input;
    Object.setPrototypeOf(this, new.target.prototype);
  }
}
