/**
 * Progress Indicator
 * Provides a spinner and progress tracking for large file processing.
 */

import ora, { type Ora } from 'ora';
import { formatRowCount, formatDuration } from './format.js';

/**
 * Progress tracker for file processing.
 * Wraps ora spinner with row count and elapsed time display.
 */
export class ProgressTracker {
  private spinner: Ora | null = null;
  private startTime: number = 0;
  private rowCount: number = 0;
  private readonly quiet: boolean;
  private readonly updateInterval: number;
  private lastUpdate: number = 0;

  /**
   * Create a new progress tracker.
   * @param quiet - If true, suppress all output
   * @param updateInterval - Minimum ms between updates (default: 100ms)
   */
  constructor(quiet: boolean = false, updateInterval: number = 100) {
    this.quiet = quiet;
    this.updateInterval = updateInterval;
  }

  /**
   * Start the progress indicator.
   * @param message - Initial spinner text
   */
  start(message: string = 'Processing...'): void {
    if (this.quiet) {
      return;
    }

    this.startTime = Date.now();
    this.rowCount = 0;
    this.lastUpdate = 0;

    this.spinner = ora({
      text: message,
      spinner: 'dots',
    }).start();
  }

  /**
   * Update progress with current row count.
   * Throttled to avoid excessive updates.
   * @param rowCount - Current row count
   */
  update(rowCount: number): void {
    if (this.quiet || !this.spinner) {
      return;
    }

    this.rowCount = rowCount;

    // Throttle updates
    const now = Date.now();
    if (now - this.lastUpdate < this.updateInterval) {
      return;
    }
    this.lastUpdate = now;

    const elapsed = now - this.startTime;
    const rowsFormatted = formatRowCount(rowCount);
    const elapsedFormatted = formatDuration(elapsed);
    const rowsPerSecond = elapsed > 0 ? Math.round((rowCount / elapsed) * 1000) : 0;

    this.spinner.text = `Processing... ${rowsFormatted} rows (${elapsedFormatted}, ~${formatRowCount(rowsPerSecond)} rows/sec)`;
  }

  /**
   * Mark progress as successful and stop spinner.
   * @param message - Success message (optional, auto-generated if not provided)
   */
  succeed(message?: string): void {
    if (this.quiet || !this.spinner) {
      return;
    }

    const elapsed = Date.now() - this.startTime;
    const defaultMessage = `Processed ${formatRowCount(this.rowCount)} rows in ${formatDuration(elapsed)}`;

    this.spinner.succeed(message ?? defaultMessage);
    this.spinner = null;
  }

  /**
   * Mark progress as failed and stop spinner.
   * @param message - Error message
   */
  fail(message: string): void {
    if (this.quiet || !this.spinner) {
      return;
    }

    this.spinner.fail(message);
    this.spinner = null;
  }

  /**
   * Stop the spinner without success/fail status.
   */
  stop(): void {
    if (this.spinner) {
      this.spinner.stop();
      this.spinner = null;
    }
  }

  /**
   * Update spinner text without affecting progress state.
   * @param text - New spinner text
   */
  setText(text: string): void {
    if (this.quiet || !this.spinner) {
      return;
    }

    this.spinner.text = text;
  }

  /**
   * Show an informational message.
   * @param message - Info message
   */
  info(message: string): void {
    if (this.quiet || !this.spinner) {
      return;
    }

    this.spinner.info(message);
  }

  /**
   * Get elapsed time in milliseconds.
   * @returns Elapsed time since start
   */
  getElapsed(): number {
    return Date.now() - this.startTime;
  }

  /**
   * Get current row count.
   * @returns Current row count
   */
  getRowCount(): number {
    return this.rowCount;
  }
}

/**
 * Create a simple spinner for non-progress operations.
 * @param text - Spinner text
 * @param quiet - If true, return a no-op spinner
 * @returns Ora spinner instance or no-op object
 */
export function createSpinner(text: string, quiet: boolean = false): Ora {
  if (quiet) {
    // Return a no-op spinner-like object
    return {
      start: () => ({ stop: () => {}, succeed: () => {}, fail: () => {}, text: '' }) as unknown as Ora,
      stop: () => {},
      succeed: () => {},
      fail: () => {},
      warn: () => {},
      info: () => {},
      text: '',
      isSpinning: false,
    } as unknown as Ora;
  }

  return ora({ text, spinner: 'dots' });
}

/**
 * Create a progress callback for the processor.
 * @param tracker - Progress tracker instance
 * @returns Callback function for processor progress updates
 */
export function createProgressCallback(tracker: ProgressTracker): (rowCount: number) => void {
  return (rowCount: number) => {
    tracker.update(rowCount);
  };
}
