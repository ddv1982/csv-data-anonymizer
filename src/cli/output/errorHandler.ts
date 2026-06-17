/**
 * CLI Error Handler
 * Shared error handling utility for CLI commands.
 */

import chalk from 'chalk';
import { AnonymizerError } from '../../types/errors.js';

/**
 * Handles command errors with consistent formatting and exits the process.
 *
 * @param error - The error to handle
 * @returns Never returns - always exits the process
 */
export function handleCommandError(error: unknown): never {
  if (error instanceof AnonymizerError) {
    process.stderr.write(chalk.red(`\n✖ ${error.toUserMessage()}\n\n`));
  } else {
    const message = error instanceof Error ? error.message : 'Unknown error occurred';
    process.stderr.write(chalk.red(`\n✖ Error: ${message}\n\n`));
  }
  process.exit(1);
}

/**
 * Wraps an async command action with error handling.
 * Use this to wrap command actions for consistent error handling.
 *
 * @param action - The async action to wrap
 * @returns A wrapped action that handles errors
 *
 * @example
 * ```typescript
 * command.action(withErrorHandling(async (file, options) => {
 *   await doSomething(file, options);
 * }));
 * ```
 */
export function withErrorHandling<T extends unknown[]>(
  action: (...args: T) => Promise<void>
): (...args: T) => Promise<void> {
  return async (...args: T): Promise<void> => {
    try {
      await action(...args);
    } catch (error) {
      handleCommandError(error);
    }
  };
}
