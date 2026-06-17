/**
 * CLI Error Handler Tests
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { handleCommandError, withErrorHandling } from '../../../src/cli/output/errorHandler.js';
import { AnonymizerError, FileNotFoundError, ErrorCodes } from '../../../src/types/errors.js';

describe('CLI Error Handler', () => {
  let mockExit: ReturnType<typeof vi.spyOn>;
  let mockStderr: ReturnType<typeof vi.spyOn>;
  let stderrOutput: string;

  beforeEach(() => {
    stderrOutput = '';
    mockExit = vi.spyOn(process, 'exit').mockImplementation(() => undefined as never);
    mockStderr = vi.spyOn(process.stderr, 'write').mockImplementation((chunk) => {
      stderrOutput += chunk.toString();
      return true;
    });
  });

  afterEach(() => {
    mockExit.mockRestore();
    mockStderr.mockRestore();
  });

  describe('handleCommandError', () => {
    it('should format AnonymizerError with toUserMessage', () => {
      const error = new FileNotFoundError('/path/to/file.csv');

      handleCommandError(error);

      expect(stderrOutput).toContain('FILE_NOT_FOUND');
      expect(stderrOutput).toContain('/path/to/file.csv');
      expect(stderrOutput).toContain('Suggestion');
      expect(mockExit).toHaveBeenCalledWith(1);
    });

    it('should handle generic AnonymizerError', () => {
      const error = new AnonymizerError(
        'Test error message',
        ErrorCodes.CONFIG_INVALID,
        'Try this instead'
      );

      handleCommandError(error);

      expect(stderrOutput).toContain('CONFIG_INVALID');
      expect(stderrOutput).toContain('Test error message');
      expect(stderrOutput).toContain('Try this instead');
      expect(mockExit).toHaveBeenCalledWith(1);
    });

    it('should handle standard Error', () => {
      const error = new Error('Something went wrong');

      handleCommandError(error);

      expect(stderrOutput).toContain('Something went wrong');
      expect(stderrOutput).not.toContain('Suggestion');
      expect(mockExit).toHaveBeenCalledWith(1);
    });

    it('should handle unknown error types', () => {
      handleCommandError('string error');

      expect(stderrOutput).toContain('Unknown error occurred');
      expect(mockExit).toHaveBeenCalledWith(1);
    });

    it('should handle null/undefined errors', () => {
      handleCommandError(null);

      expect(stderrOutput).toContain('Unknown error occurred');
      expect(mockExit).toHaveBeenCalledWith(1);
    });
  });

  describe('withErrorHandling', () => {
    it('should execute action successfully when no error', async () => {
      const mockAction = vi.fn().mockResolvedValue(undefined);
      const wrapped = withErrorHandling(mockAction);

      await wrapped('arg1', 'arg2');

      expect(mockAction).toHaveBeenCalledWith('arg1', 'arg2');
      expect(mockExit).not.toHaveBeenCalled();
    });

    it('should catch and handle errors from action', async () => {
      const mockAction = vi.fn().mockRejectedValue(new Error('Action failed'));
      const wrapped = withErrorHandling(mockAction);

      await wrapped('arg1');

      expect(stderrOutput).toContain('Action failed');
      expect(mockExit).toHaveBeenCalledWith(1);
    });

    it('should handle AnonymizerError from action', async () => {
      const error = new FileNotFoundError('/missing.csv');
      const mockAction = vi.fn().mockRejectedValue(error);
      const wrapped = withErrorHandling(mockAction);

      await wrapped();

      expect(stderrOutput).toContain('FILE_NOT_FOUND');
      expect(stderrOutput).toContain('/missing.csv');
      expect(mockExit).toHaveBeenCalledWith(1);
    });

    it('should preserve function arguments', async () => {
      const mockAction = vi.fn().mockResolvedValue(undefined);
      const wrapped = withErrorHandling(mockAction);

      await wrapped('file.csv', { force: true, quiet: false });

      expect(mockAction).toHaveBeenCalledWith('file.csv', { force: true, quiet: false });
    });
  });
});
