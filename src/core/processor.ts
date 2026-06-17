/**
 * Streaming CSV Processor
 * Processes large CSV files using streaming to maintain constant memory usage.
 */

import { createReadStream, createWriteStream } from 'node:fs';
import { rename, rm } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';
import Papa from 'papaparse';
import { validateFile, stripBom } from './fileReader.js';
import { getFatalParseError } from './papaParseErrors.js';
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

function createTemporaryOutputPath(outputPath: string): string {
  const suffix = `${process.pid}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
  return join(dirname(outputPath), `.${basename(outputPath)}.${suffix}.tmp`);
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
    let settled = false;
    const temporaryOutputPath = createTemporaryOutputPath(outputPath);

    const inputStream = createReadStream(inputPath, { encoding: 'utf-8' });

    const removeTemporaryOutput = async (): Promise<void> => {
      await rm(temporaryOutputPath, { force: true }).catch(() => undefined);
    };

    const fail = (error: CsvParseError): void => {
      if (settled) {
        return;
      }

      settled = true;
      inputStream.destroy();
      writeStream?.destroy();
      removeTemporaryOutput().finally(() => reject(error));
    };

    // Handle input stream errors
    inputStream.on('error', (error) => {
      fail(new CsvParseError(`Failed to read input file: ${error.message}`));
    });

    // Create output stream
    try {
      writeStream = createWriteStream(temporaryOutputPath, { encoding: 'utf-8' });
    } catch (error) {
      fail(new CsvParseError(`Failed to create output file: ${(error as Error).message}`));
      return;
    }

    // Handle output stream errors
    writeStream.on('error', (error) => {
      fail(new CsvParseError(`Failed to write output file: ${error.message}`));
    });

    Papa.parse<string[]>(inputStream, {
      header: false,
      skipEmptyLines: false, // Preserve structure

      step: (results, parserInstance) => {
        // Handle parse errors
        const fatalError = getFatalParseError(results.errors);
        if (fatalError) {
          parserInstance.abort();
          fail(new CsvParseError(fatalError.message, fatalError.row));
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
        if (settled) {
          return;
        }

        // Final progress callback
        if (options.onProgress) {
          options.onProgress(rowCount);
        }

        // Close write stream properly
        writeStream!.end(async () => {
          if (settled) {
            return;
          }

          const duration = Date.now() - startTime;

          try {
            await rename(temporaryOutputPath, outputPath);
            settled = true;

            resolve({
              rowCount,
              success: true,
              outputPath,
              duration,
            });
          } catch (error) {
            await removeTemporaryOutput();
            fail(new CsvParseError(`Failed to finalize output file: ${(error as Error).message}`));
          }
        });
      },

      error: (error: Error) => {
        fail(new CsvParseError(`CSV parsing error: ${error.message}`));
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
