/**
 * Serve Command
 * Starts the web UI server with Express backend.
 */

import { Command } from 'commander';
import chalk from 'chalk';
import open from 'open';
import { existsSync } from 'node:fs';
import { execSync } from 'node:child_process';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

import { startServer } from '../../server/index.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = join(__dirname, '../../..');
const UI_DIST_PATH = join(PROJECT_ROOT, 'ui/dist');

/**
 * Options for the serve command.
 */
interface ServeCommandOptions {
  port?: string;
  host?: string;
  open?: boolean;
}

/**
 * Default server port.
 */
const DEFAULT_PORT = 3456;

/**
 * Default server host.
 */
const DEFAULT_HOST = 'localhost';

/**
 * Opens a URL in the default browser.
 * Uses the 'open' package for reliable cross-platform support.
 *
 * @param url - The URL to open
 * @returns Promise that resolves when browser opens or rejects on error
 */
async function openBrowser(url: string): Promise<void> {
  try {
    await open(url, { wait: false });
  } catch (error) {
    // Graceful fallback - log message but don't crash
    const errorMessage = error instanceof Error ? error.message : 'Unknown error';
    console.error(chalk.dim(`Note: Could not open browser automatically (${errorMessage}). Please visit ${url}`));
  }
}

/**
 * Ensures the UI is built before starting the server.
 * If the UI dist directory doesn't exist, triggers a build.
 */
function ensureUiBuilt(): void {
  if (existsSync(UI_DIST_PATH)) {
    return;
  }

  console.log(chalk.yellow('\nUI not built. Building now...\n'));

  try {
    execSync('npm run build:ui', {
      stdio: 'inherit',
      cwd: PROJECT_ROOT,
    });
    console.log(chalk.green('\nUI build complete.\n'));
  } catch {
    console.error(chalk.red('\nFailed to build UI. Run "npm run build:ui" manually.\n'));
    process.exit(1);
  }
}

/**
 * Run the serve command.
 */
async function runServe(options: ServeCommandOptions): Promise<void> {
  // Ensure UI is built before starting server
  ensureUiBuilt();

  const port = options.port ? parseInt(options.port, 10) : DEFAULT_PORT;
  const host = options.host ?? DEFAULT_HOST;
  const url = `http://${host}:${port}`;

  const write = (text: string) => process.stdout.write(text);

  write('\n');
  write(chalk.cyan('🚀 CSV Anonymizer UI'));
  write('\n\n');

  write(`  ${chalk.dim('Local:')}   ${chalk.cyan(url)}`);
  write('\n\n');

  write(chalk.dim('  Press Ctrl+C to stop the server.'));
  write('\n\n');

  // Start the Express server
  const server = await startServer(port, host);

  // Open browser if enabled
  if (options.open !== false) {
    // Small delay to ensure server is ready
    setTimeout(async () => {
      await openBrowser(url);
    }, 300);
  }

  // Handle graceful shutdown
  const shutdown = () => {
    write('\n');
    write(chalk.dim('Shutting down server...'));
    write('\n');

    server.close(() => {
      write(chalk.green('Server stopped.'));
      write('\n\n');
      process.exit(0);
    });

    // Force exit after 5 seconds if graceful shutdown fails
    setTimeout(() => {
      process.exit(1);
    }, 5000);
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}

/**
 * Create the serve command.
 */
export function createServeCommand(): Command {
  const command = new Command('serve');

  command
    .description('Start the web UI server')
    .option('-p, --port <number>', `Server port (default: ${DEFAULT_PORT})`, String(DEFAULT_PORT))
    .option('-H, --host <address>', `Host address (default: ${DEFAULT_HOST})`, DEFAULT_HOST)
    .option('--no-open', "Don't open browser automatically")
    .action(async (options: ServeCommandOptions) => {
      await runServe(options);
    });

  return command;
}

export { runServe };
