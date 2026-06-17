import { randomBytes } from 'node:crypto';
import type { AnonymizationConfig, AnonymizeCommandOptions } from '../types/config.js';

/**
 * Default configuration values for optional settings.
 */
export const DEFAULT_CONFIG = {
  /** Use non-deterministic transformations by default */
  deterministic: false,
  /** Output path is generated from input filename if not specified */
  output: undefined as string | undefined,
} as const;

/**
 * Generate a random seed string for deterministic mode.
 *
 * @returns A random 16-byte hex string
 */
export function generateRandomSeed(): string {
  return randomBytes(16).toString('hex');
}

/**
 * Generate default output path from input path.
 *
 * @param inputPath - The input CSV file path
 * @returns Output path with "_anonymized" suffix before extension
 */
export function generateDefaultOutputPath(inputPath: string): string {
  const lastDot = inputPath.lastIndexOf('.');
  if (lastDot === -1) {
    return `${inputPath}_anonymized`;
  }
  const baseName = inputPath.slice(0, lastDot);
  const extension = inputPath.slice(lastDot);
  return `${baseName}_anonymized${extension}`;
}

/**
 * Merge user configuration with defaults.
 * Precedence: CLI flags > config file > defaults
 *
 * @param cliOptions - CLI command options (highest precedence)
 * @param fileConfig - Configuration loaded from file (optional)
 * @returns Merged configuration with all required fields set
 */
export function mergeConfig(
  cliOptions: Partial<AnonymizeCommandOptions>,
  fileConfig?: Partial<AnonymizationConfig>
): {
  deterministic: boolean;
  seed: string;
  output: string | undefined;
} {
  // Determine deterministic mode with precedence
  const deterministic =
    cliOptions.deterministic !== undefined
      ? cliOptions.deterministic
      : fileConfig?.deterministic !== undefined
        ? fileConfig.deterministic
        : DEFAULT_CONFIG.deterministic;

  // Determine seed with precedence
  // CLI seed > file seed > generate new seed
  const seed =
    cliOptions.seed !== undefined
      ? cliOptions.seed
      : fileConfig?.seed !== undefined
        ? fileConfig.seed
        : generateRandomSeed();

  // Determine output with precedence
  // CLI output > file output > undefined (will be generated from input path later)
  const output =
    cliOptions.output !== undefined
      ? cliOptions.output
      : fileConfig?.output !== undefined
        ? fileConfig.output
        : DEFAULT_CONFIG.output;

  return {
    deterministic,
    seed,
    output,
  };
}

/**
 * Create a complete configuration by merging defaults with user input.
 * This is a convenience function that handles the common case of
 * merging defaults with partial user configuration.
 *
 * @param userConfig - Partial user configuration
 * @returns Complete configuration with all defaults applied
 */
export function applyDefaults(
  userConfig: Partial<AnonymizationConfig>
): AnonymizationConfig & { seed: string } {
  return {
    columns: userConfig.columns ?? [],
    output: userConfig.output ?? DEFAULT_CONFIG.output,
    deterministic: userConfig.deterministic ?? DEFAULT_CONFIG.deterministic,
    seed: userConfig.seed ?? generateRandomSeed(),
  };
}
