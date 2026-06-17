/**
 * Unit Tests for Progress Indicator
 */

import { describe, it, expect, vi } from 'vitest';
import { ProgressTracker, createSpinner, createProgressCallback } from '../../src/cli/output/progress.js';

describe('ProgressTracker', () => {
  describe('constructor', () => {
    it('should create a tracker with default options', () => {
      const tracker = new ProgressTracker();
      expect(tracker.getRowCount()).toBe(0);
    });

    it('should create a quiet tracker', () => {
      const tracker = new ProgressTracker(true);
      // Quiet mode should not throw
      tracker.start('Testing');
      tracker.update(100);
      tracker.succeed();
    });
  });

  describe('start', () => {
    it('should start tracking', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Processing...');
      expect(tracker.getElapsed()).toBeGreaterThanOrEqual(0);
    });

    it('should reset row count on start', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('First');
      expect(tracker.getRowCount()).toBe(0);
    });
  });

  describe('update', () => {
    it('should update row count in non-quiet mode', () => {
      // Non-quiet mode to actually track updates
      const tracker = new ProgressTracker(false);
      tracker.start('Processing...');
      tracker.update(500);
      expect(tracker.getRowCount()).toBe(500);
      tracker.succeed();
    });

    it('should handle multiple updates in non-quiet mode', () => {
      const tracker = new ProgressTracker(false);
      tracker.start('Processing...');
      tracker.update(100);
      tracker.update(200);
      tracker.update(300);
      expect(tracker.getRowCount()).toBe(300);
      tracker.succeed();
    });

    it('should not throw in quiet mode', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Processing...');
      tracker.update(500);
      // Quiet mode doesn't update rowCount, just verify no error
      tracker.succeed();
    });
  });

  describe('succeed', () => {
    it('should stop the tracker on success', () => {
      const tracker = new ProgressTracker(false);
      tracker.start('Processing...');
      tracker.update(100);
      tracker.succeed();
      // Should not throw after succeed
      expect(tracker.getRowCount()).toBe(100);
    });

    it('should accept custom success message', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Processing...');
      tracker.succeed('Done!');
      // Should complete without errors
    });
  });

  describe('fail', () => {
    it('should stop the tracker on failure', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Processing...');
      tracker.fail('Something went wrong');
      // Should not throw after fail
    });
  });

  describe('stop', () => {
    it('should stop the spinner without status', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Processing...');
      tracker.stop();
      // Should complete without errors
    });
  });

  describe('setText', () => {
    it('should update spinner text', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Initial');
      tracker.setText('Updated');
      // Should complete without errors
    });
  });

  describe('info', () => {
    it('should show info message', () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Processing...');
      tracker.info('Information');
      // Should complete without errors
    });
  });

  describe('getElapsed', () => {
    it('should return elapsed time', async () => {
      const tracker = new ProgressTracker(true);
      tracker.start('Processing...');

      // Wait a bit
      await new Promise(resolve => setTimeout(resolve, 50));

      const elapsed = tracker.getElapsed();
      expect(elapsed).toBeGreaterThanOrEqual(40); // Allow some tolerance
    });
  });

  describe('getRowCount', () => {
    it('should return current row count', () => {
      const tracker = new ProgressTracker(false);
      tracker.start('Processing...');
      tracker.update(1234);
      expect(tracker.getRowCount()).toBe(1234);
      tracker.succeed();
    });
  });

  describe('non-quiet mode', () => {
    it('should work in non-quiet mode', () => {
      const tracker = new ProgressTracker(false);
      // This will actually create a spinner, but we just verify it doesn't throw
      tracker.start('Testing');
      tracker.update(100);
      tracker.succeed();
    });
  });
});

describe('createSpinner', () => {
  it('should create a spinner in quiet mode', () => {
    const spinner = createSpinner('Test', true);
    // Should return a no-op spinner
    expect(spinner.isSpinning).toBe(false);
    spinner.start();
    spinner.stop();
    spinner.succeed();
    spinner.fail();
  });

  it('should create a real spinner in non-quiet mode', () => {
    const spinner = createSpinner('Test', false);
    // Should return a real ora spinner
    expect(spinner).toBeDefined();
  });
});

describe('createProgressCallback', () => {
  it('should create a callback that updates tracker', () => {
    // Use non-quiet mode so updates actually work
    const tracker = new ProgressTracker(false);
    tracker.start('Processing...');

    const callback = createProgressCallback(tracker);

    callback(100);
    expect(tracker.getRowCount()).toBe(100);

    callback(200);
    expect(tracker.getRowCount()).toBe(200);

    tracker.succeed();
  });
});
