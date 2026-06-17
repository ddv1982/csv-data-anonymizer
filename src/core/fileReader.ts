/**
 * CSV File Reader
 * Provides file validation, stat retrieval, and encoding handling for CSV processing.
 */

import { promises as fs } from 'node:fs';
import { FileNotFoundError } from '../types/errors.js';

/**
 * File statistics for progress estimation
 */
export interface FileStats {
  /** File size in bytes */
  size: number;
  /** Whether the file is readable */
  isReadable: boolean;
}

/**
 * Result from file validation
 */
export interface FileValidationResult {
  /** Whether the file is valid */
  valid: boolean;
  /** File path */
  path: string;
  /** File statistics */
  stats: FileStats;
}

/**
 * Validates that a file exists and is readable.
 *
 * @param filePath - Path to the CSV file
 * @returns File validation result with stats
 * @throws FileNotFoundError if file doesn't exist or is not readable
 */
export async function validateFile(filePath: string): Promise<FileValidationResult> {
  try {
    const stats = await fs.stat(filePath);

    if (!stats.isFile()) {
      throw new FileNotFoundError(filePath);
    }

    // Check if file is readable by attempting to open it
    const fileHandle = await fs.open(filePath, 'r');
    await fileHandle.close();

    return {
      valid: true,
      path: filePath,
      stats: {
        size: stats.size,
        isReadable: true,
      },
    };
  } catch (error) {
    if (error instanceof FileNotFoundError) {
      throw error;
    }

    // Handle ENOENT (file not found) and EACCES (permission denied)
    if (error instanceof Error && 'code' in error) {
      const nodeError = error as NodeJS.ErrnoException;
      if (nodeError.code === 'ENOENT' || nodeError.code === 'EACCES') {
        throw new FileNotFoundError(filePath);
      }
    }

    throw new FileNotFoundError(filePath);
  }
}

/**
 * UTF-8 BOM (Byte Order Mark) as a string
 */
const UTF8_BOM = '\uFEFF';

/**
 * Strips UTF-8 BOM from the beginning of a string if present.
 *
 * @param content - Content that may contain BOM
 * @returns Content with BOM removed
 */
export function stripBom(content: string): string {
  if (content.startsWith(UTF8_BOM)) {
    return content.slice(1);
  }
  return content;
}

/**
 * Reads file content with UTF-8 encoding and BOM stripping.
 *
 * @param filePath - Path to the file
 * @returns File content as string with BOM stripped
 * @throws FileNotFoundError if file doesn't exist
 */
export async function readFileContent(filePath: string): Promise<string> {
  await validateFile(filePath);

  const content = await fs.readFile(filePath, 'utf-8');
  return stripBom(content);
}

/**
 * Gets file size in bytes without reading the entire file.
 *
 * @param filePath - Path to the file
 * @returns File size in bytes
 * @throws FileNotFoundError if file doesn't exist
 */
export async function getFileSize(filePath: string): Promise<number> {
  const result = await validateFile(filePath);
  return result.stats.size;
}
