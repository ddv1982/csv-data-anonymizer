/**
 * CSV Sample Reader
 * Reads first N rows of CSV file using streaming for efficient memory usage.
 */

import { createReadStream } from 'node:fs';
import Papa from 'papaparse';
import { validateFile, stripBom } from './fileReader.js';
import { getFatalParseError } from './papaParseErrors.js';
import { CsvParseError } from '../types/errors.js';
import type { ParsedSample } from '../types/config.js';

/**
 * Options for reading CSV samples
 */
export interface SampleReadOptions {
  /** Whether to skip empty rows */
  skipEmptyRows?: boolean;
  /** Whether to trim whitespace from values */
  trimValues?: boolean;
}

const DEFAULT_OPTIONS: SampleReadOptions = {
  skipEmptyRows: true,
  trimValues: true,
};

/**
 * Reads the first N rows of a CSV file using streaming.
 * Stops reading after the specified number of rows to avoid loading the entire file.
 *
 * @param filePath - Path to the CSV file
 * @param rowCount - Maximum number of data rows to read (excluding header)
 * @param options - Optional reading configuration
 * @returns ParsedSample with headers and sample rows
 * @throws FileNotFoundError if file doesn't exist
 * @throws CsvParseError if CSV parsing fails
 */
export async function readSample(
  filePath: string,
  rowCount: number,
  options: SampleReadOptions = {}
): Promise<ParsedSample> {
  // Validate file exists first
  await validateFile(filePath);

  const opts = { ...DEFAULT_OPTIONS, ...options };

  return new Promise((resolve, reject) => {
    const rows: string[][] = [];
    let headers: string[] = [];
    let headerProcessed = false;
    let rowsParsed = 0;
    let firstChunk = true;

    const fileStream = createReadStream(filePath, { encoding: 'utf-8' });

    // Handle stream errors
    fileStream.on('error', (error) => {
      reject(new CsvParseError(`Failed to read file: ${error.message}`));
    });

    Papa.parse<string[]>(fileStream, {
      // Don't use Papa's header option - we need to handle BOM manually
      header: false,
      skipEmptyLines: opts.skipEmptyRows,
      transform: opts.trimValues ? (value: string) => value.trim() : undefined,

      step: (results, parserInstance) => {
        // Handle parse errors
        const fatalError = getFatalParseError(results.errors);
        if (fatalError) {
          parserInstance.abort();
          reject(new CsvParseError(fatalError.message, fatalError.row));
          return;
        }

        let row = results.data;

        // Handle BOM in first value of first chunk
        if (firstChunk && row.length > 0) {
          row[0] = stripBom(row[0]);
          firstChunk = false;
        }

        // First row is headers
        if (!headerProcessed) {
          headers = row;
          headerProcessed = true;
          return;
        }

        // Skip truly empty rows (all values empty)
        if (opts.skipEmptyRows && row.every(v => v === '')) {
          return;
        }

        rowsParsed++;
        rows.push(row);

        // Stop after reaching desired row count
        if (rowsParsed >= rowCount) {
          parserInstance.abort();
        }
      },

      complete: () => {
        // If no headers were found, it's an empty file
        if (!headerProcessed || headers.length === 0) {
          reject(new CsvParseError('CSV file is empty or has no valid headers'));
          return;
        }

        resolve({
          headers,
          rows,
        });
      },

      error: (error: Error) => {
        reject(new CsvParseError(`CSV parsing error: ${error.message}`));
      },
    });
  });
}

/**
 * Reads all rows from a CSV file.
 * Use with caution for large files - prefer readSample for sampling.
 *
 * @param filePath - Path to the CSV file
 * @param options - Optional reading configuration
 * @returns ParsedSample with all headers and rows
 */
export async function readAllRows(
  filePath: string,
  options: SampleReadOptions = {}
): Promise<ParsedSample> {
  await validateFile(filePath);

  const opts = { ...DEFAULT_OPTIONS, ...options };

  return new Promise((resolve, reject) => {
    const rows: string[][] = [];
    let headers: string[] = [];
    let headerProcessed = false;
    let firstChunk = true;

    const fileStream = createReadStream(filePath, { encoding: 'utf-8' });

    fileStream.on('error', (error) => {
      reject(new CsvParseError(`Failed to read file: ${error.message}`));
    });

    Papa.parse<string[]>(fileStream, {
      header: false,
      skipEmptyLines: opts.skipEmptyRows,
      transform: opts.trimValues ? (value: string) => value.trim() : undefined,

      step: (results, parserInstance) => {
        const fatalError = getFatalParseError(results.errors);
        if (fatalError) {
          parserInstance.abort();
          reject(new CsvParseError(fatalError.message, fatalError.row));
          return;
        }

        let row = results.data;

        if (firstChunk && row.length > 0) {
          row[0] = stripBom(row[0]);
          firstChunk = false;
        }

        if (!headerProcessed) {
          headers = row;
          headerProcessed = true;
          return;
        }

        if (opts.skipEmptyRows && row.every(v => v === '')) {
          return;
        }

        rows.push(row);
      },

      complete: () => {
        if (!headerProcessed || headers.length === 0) {
          reject(new CsvParseError('CSV file is empty or has no valid headers'));
          return;
        }

        resolve({
          headers,
          rows,
        });
      },

      error: (error: Error) => {
        reject(new CsvParseError(`CSV parsing error: ${error.message}`));
      },
    });
  });
}
