/**
 * Streaming CSV Processor
 * Processes large CSV files using streaming to maintain constant memory usage.
 */

import { createReadStream, createWriteStream } from 'node:fs';
import Papa from 'papaparse';
import { validateFile, stripBom } from './fileReader.js';
import { createRowTransformer } from './transformer.js';
import { CsvParseError } from '../types/errors.js';
import type { ColumnMetadata } from '../types/column.js';
import type { ProcessResult, ProcessOptions } from '../types/config.js';

/**
 * Progress report interval in rows
 */
const PROGRESS_INTERVAL = 10000;

/**
 * Converts a row array to CSV line with proper escaping
 */
function rowToCsvLine(row: string[]): string {
  return row.map(value => {
    // If value contains comma, quote, or newline, quote it and escape internal quotes
    if (value.includes(',') || value.includes('"') || value.includes('\n') || value.includes('\r')) {
      return `"${value.replace(/"/g, '""')}"`;
    }
    return value;
  }).join(',');
}

/**
 * Processes a CSV file, applying transformations to selected columns.
 * Uses streaming to handle large files without loading into memory.
 *
 * @param inputPath - Path to the input CSV file
 * @param outputPath - Path for the output CSV file
 * @param columns - Array of column metadata with selection flags
 * @param options - Processing options
 * @returns Processing result with stats
 */
export async function processFile(
  inputPath: string,
  outputPath: string,
  columns: ColumnMetadata[],
  options: ProcessOptions
): Promise<ProcessResult> {
  // Validate input file exists
  await validateFile(inputPath);

  const startTime = Date.now();
  let rowCount = 0;

  // Create row transformer
  const transformRow = createRowTransformer(
    columns,
    options.seed,
    options.deterministic
  );

  return new Promise((resolve, reject) => {
    let headerProcessed = false;
    let firstChunk = true;
    let writeStream: ReturnType<typeof createWriteStream> | null = null;

    const inputStream = createReadStream(inputPath, { encoding: 'utf-8' });

    // Handle input stream errors
    inputStream.on('error', (error) => {
      if (writeStream) {
        writeStream.end();
      }
      reject(new CsvParseError(`Failed to read input file: ${error.message}`));
    });

    // Create output stream
    try {
      writeStream = createWriteStream(outputPath, { encoding: 'utf-8' });
    } catch (error) {
      reject(new CsvParseError(`Failed to create output file: ${(error as Error).message}`));
      return;
    }

    // Handle output stream errors
    writeStream.on('error', (error) => {
      reject(new CsvParseError(`Failed to write output file: ${error.message}`));
    });

    Papa.parse<string[]>(inputStream, {
      header: false,
      skipEmptyLines: false, // Preserve structure

      step: (results, parserInstance) => {
        // Handle parse errors
        if (results.errors.length > 0) {
          const error = results.errors[0];
          parserInstance.abort();
          if (writeStream) {
            writeStream.end();
          }
          reject(new CsvParseError(error.message, error.row));
          return;
        }

        let row = results.data;

        // Handle BOM in first value of first chunk
        if (firstChunk && row.length > 0) {
          row[0] = stripBom(row[0]);
          firstChunk = false;
        }

        // First row is headers - write unchanged
        if (!headerProcessed) {
          const headerLine = rowToCsvLine(row);
          writeStream!.write(headerLine + '\n');
          headerProcessed = true;
          return;
        }

        // Transform the row
        const transformedRow = transformRow(row, rowCount);
        const outputLine = rowToCsvLine(transformedRow);
        writeStream!.write(outputLine + '\n');

        rowCount++;

        // Report progress
        if (rowCount % PROGRESS_INTERVAL === 0 && options.onProgress) {
          options.onProgress(rowCount);
        }
      },

      complete: () => {
        // Final progress callback
        if (options.onProgress) {
          options.onProgress(rowCount);
        }

        // Close write stream properly
        writeStream!.end(() => {
          const duration = Date.now() - startTime;

          resolve({
            rowCount,
            success: true,
            outputPath,
            duration,
          });
        });
      },

      error: (error: Error) => {
        if (writeStream) {
          writeStream.end();
        }
        reject(new CsvParseError(`CSV parsing error: ${error.message}`));
      },
    });
  });
}

/**
 * Processes a CSV file with default options.
 * Convenience wrapper for simple use cases.
 *
 * @param inputPath - Path to the input CSV file
 * @param outputPath - Path for the output CSV file
 * @param columns - Array of column metadata with selection flags
 * @returns Processing result with stats
 */
export async function processFileSimple(
  inputPath: string,
  outputPath: string,
  columns: ColumnMetadata[]
): Promise<ProcessResult> {
  return processFile(inputPath, outputPath, columns, {
    deterministic: false,
    seed: '',
  });
}
