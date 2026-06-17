import { readFileSync, existsSync } from 'node:fs';
import yaml from 'js-yaml';
import type { AnonymizationConfig } from '../types/config.js';
import { ConfigValidationError, FileNotFoundError } from '../types/errors.js';
import { safeValidateConfig } from './schema.js';

/**
 * Load and parse a YAML configuration file.
 *
 * @param filePath - Path to the YAML configuration file
 * @returns Validated AnonymizationConfig
 * @throws FileNotFoundError if the file does not exist
 * @throws ConfigValidationError if the config is invalid
 */
export function loadConfig(filePath: string): AnonymizationConfig {
  // Check if file exists
  if (!existsSync(filePath)) {
    throw new FileNotFoundError(filePath);
  }

  // Read the file content
  let content: string;
  try {
    content = readFileSync(filePath, 'utf-8');
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown read error';
    throw new FileNotFoundError(`${filePath} (${message})`);
  }

  // Parse YAML
  let parsed: unknown;
  try {
    parsed = yaml.load(content);
  } catch (error) {
    const message = error instanceof Error ? error.message : 'Unknown parse error';
    throw new ConfigValidationError([
      {
        code: 'custom',
        path: [],
        message: `YAML parse error: ${message}`,
      },
    ]);
  }

  // Handle empty file
  if (parsed === null || parsed === undefined) {
    throw new ConfigValidationError([
      {
        code: 'custom',
        path: [],
        message: 'Configuration file is empty',
      },
    ]);
  }

  // Validate against schema
  const result = safeValidateConfig(parsed);
  if (!result.success) {
    throw new ConfigValidationError(result.error.issues);
  }

  return result.data as AnonymizationConfig;
}
