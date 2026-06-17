/**
 * CLI Integration Tests
 * Tests CLI commands with various options and scenarios.
 * Updated for subcommand-based architecture (headers, preview, run, serve).
 */

import { describe, it, expect, afterEach } from 'vitest';
import { spawn, type ChildProcess } from 'node:child_process';
import { createServer, type Server } from 'node:net';
import { promises as fs, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.join(__dirname, '..', 'fixtures');
const projectRoot = path.join(__dirname, '..', '..');
const cliPath = path.join(projectRoot, 'dist', 'index.js');

/**
 * Get an available port for testing to avoid port conflicts
 */
async function getAvailablePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const server: Server = createServer();
    server.on('error', reject);
    server.listen(0, () => {
      const address = server.address();
      if (address && typeof address === 'object') {
        const port = address.port;
        server.close(() => resolve(port));
      } else {
        server.close(() => reject(new Error('Could not get port')));
      }
    });
  });
}

/**
 * Wait for a child process to output a specific pattern
 */
async function waitForOutput(
  child: ChildProcess,
  pattern: string,
  timeout: number = 5000
): Promise<string> {
  return new Promise((resolve, reject) => {
    let output = '';
    const timeoutId = setTimeout(() => {
      reject(new Error(`Timeout waiting for pattern: ${pattern}. Got: ${output}`));
    }, timeout);

    const onData = (data: Buffer) => {
      output += data.toString();
      if (output.includes(pattern)) {
        clearTimeout(timeoutId);
        resolve(output);
      }
    };

    child.stdout?.on('data', onData);
    child.stderr?.on('data', onData);

    child.on('exit', () => {
      clearTimeout(timeoutId);
      if (output.includes(pattern)) {
        resolve(output);
      } else {
        reject(new Error(`Process exited before pattern found. Got: ${output}`));
      }
    });

    child.on('error', (err) => {
      clearTimeout(timeoutId);
      reject(err);
    });
  });
}

// Helper to run CLI and capture output
async function runCli(args: string[], options: { timeout?: number; stdin?: string } = {}): Promise<{
  stdout: string;
  stderr: string;
  exitCode: number | null;
}> {
  return new Promise((resolve) => {
    const timeout = options.timeout ?? 30000;
    const child: ChildProcess = spawn('node', [cliPath, ...args], {
      cwd: projectRoot,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    let stdout = '';
    let stderr = '';
    let resolved = false;

    child.stdout?.on('data', (data) => {
      stdout += data.toString();
    });

    child.stderr?.on('data', (data) => {
      stderr += data.toString();
    });

    // Write stdin if provided
    if (options.stdin) {
      child.stdin?.write(options.stdin);
      child.stdin?.end();
    } else {
      child.stdin?.end();
    }

    const timeoutId = setTimeout(() => {
      if (!resolved) {
        child.kill('SIGTERM');
        resolved = true;
        resolve({ stdout, stderr, exitCode: null });
      }
    }, timeout);

    child.on('close', (code) => {
      if (!resolved) {
        clearTimeout(timeoutId);
        resolved = true;
        resolve({ stdout, stderr, exitCode: code });
      }
    });

    child.on('error', (err) => {
      if (!resolved) {
        clearTimeout(timeoutId);
        resolved = true;
        resolve({ stdout, stderr: err.message, exitCode: 1 });
      }
    });
  });
}

describe('CLI Integration Tests', () => {
  const sampleCsvPath = path.join(fixturesDir, 'sample.csv');
  const configPath = path.join(fixturesDir, 'config.yml');
  const invalidConfigPath = path.join(fixturesDir, 'invalid-config.yml');
  const outputPath = path.join(fixturesDir, 'cli-output-test.csv');

  // Clean up output files after each test
  afterEach(async () => {
    const filesToClean = [
      outputPath,
      path.join(fixturesDir, 'sample_anonymized.csv'),
    ];

    for (const file of filesToClean) {
      try {
        await fs.unlink(file);
      } catch {
        // Ignore if not created
      }
    }
  });

  describe('--help flag', () => {
    it('should display help text with subcommands', async () => {
      const result = await runCli(['--help']);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('csv-anonymizer');
      expect(result.stdout).toContain('Interactive CSV Anonymizer');
      // Check for subcommands
      expect(result.stdout).toContain('headers');
      expect(result.stdout).toContain('preview');
      expect(result.stdout).toContain('run');
      expect(result.stdout).toContain('serve');
    });

    it('should display run command help', async () => {
      const result = await runCli(['run', '--help']);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('--output');
      expect(result.stdout).toContain('--config');
      expect(result.stdout).toContain('--preview');
      expect(result.stdout).toContain('--deterministic');
      expect(result.stdout).toContain('--yes');
    });
  });

  describe('--version flag', () => {
    it('should display version', async () => {
      const result = await runCli(['--version']);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toMatch(/\d+\.\d+\.\d+/);
    });
  });

  describe('headers command', () => {
    it('should display column headers with types and risk levels', async () => {
      const result = await runCli(['headers', sampleCsvPath]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('CSV Headers');
      expect(result.stdout).toContain('Column Name');
      expect(result.stdout).toContain('Type');
      expect(result.stdout).toContain('PII Risk');
      expect(result.stdout).toContain('email');
      expect(result.stdout).toContain('high');
    });

    it('should output JSON with --quiet flag', async () => {
      const result = await runCli(['headers', sampleCsvPath, '--quiet']);

      expect(result.exitCode).toBe(0);
      const json = JSON.parse(result.stdout);
      expect(json.columns).toBeDefined();
      expect(Array.isArray(json.columns)).toBe(true);
      expect(json.columns.length).toBe(7);
      expect(json.columns[1].type).toBe('email');
      expect(json.columns[1].piiRisk).toBe('high');
    });

    it('should show usage hint in footer', async () => {
      const result = await runCli(['headers', sampleCsvPath]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("csv-anonymizer run");
    });

    it('should handle missing file', async () => {
      const result = await runCli(['headers', '/nonexistent/file.csv']);

      expect(result.exitCode).toBe(1);
      expect(result.stderr.toLowerCase()).toMatch(/not found|does not exist|no such file/);
    });
  });

  describe('preview command', () => {
    it('should show preview for high-risk columns by default', async () => {
      const result = await runCli(['preview', sampleCsvPath]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('Preview');
      expect(result.stdout).toContain('email');
      expect(result.stdout).toContain('→');
    });

    it('should preview specific columns with -C flag', async () => {
      const result = await runCli(['preview', sampleCsvPath, '-C', '2,3']);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain('email');
      expect(result.stdout).toContain('user_uuid');
    });

    it('should control sample count with -n flag', async () => {
      const result = await runCli(['preview', sampleCsvPath, '-C', '2', '-n', '2']);

      expect(result.exitCode).toBe(0);
      // Count arrow symbols (→) to verify sample count
      const arrowCount = (result.stdout.match(/→/g) || []).length;
      expect(arrowCount).toBe(2);
    });

    it('should support deterministic mode', async () => {
      const result1 = await runCli(['preview', sampleCsvPath, '-C', '2', '-d', '-s', 'test-seed']);
      const result2 = await runCli(['preview', sampleCsvPath, '-C', '2', '-d', '-s', 'test-seed']);

      expect(result1.exitCode).toBe(0);
      expect(result2.exitCode).toBe(0);
      expect(result1.stdout).toBe(result2.stdout);
    });

    it('should show usage hint for run command', async () => {
      const result = await runCli(['preview', sampleCsvPath]);

      expect(result.exitCode).toBe(0);
      expect(result.stdout).toContain("csv-anonymizer run");
    });
  });

  describe('run command', () => {
    describe('--preview flag', () => {
      it('should show preview without processing the file', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--preview',
          '--columns', '2',
          '-y',
          '--quiet',
        ]);

        expect(result.exitCode).toBe(0);
        expect(result.stdout).toContain('Preview');

        // Verify no output file was created
        expect(existsSync(outputPath)).toBe(false);
        expect(existsSync(path.join(fixturesDir, 'sample_anonymized.csv'))).toBe(false);
      });
    });

    describe('--config flag', () => {
      it('should load and apply config from YAML file', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--config', configPath,
          '--preview',
          '-y',
        ]);

        expect(result.exitCode).toBe(0);
        expect(result.stdout).toContain('Loading config');
      });

      it('should handle invalid config file', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--config', invalidConfigPath,
          '-y',
        ]);

        expect(result.exitCode).toBe(1);
        expect(result.stderr).toContain('Error');
      });

      it('should handle missing config file', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--config', '/nonexistent/config.yml',
          '-y',
        ]);

        expect(result.exitCode).toBe(1);
      });
    });

    describe('--columns flag', () => {
      it('should select correct columns by index', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--columns', '2',
          '--preview',
          '-y',
        ]);

        expect(result.exitCode).toBe(0);
        expect(result.stdout).toContain('email');
      });

      it('should select multiple columns', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--columns', '2,3',
          '--preview',
          '-y',
        ]);

        expect(result.exitCode).toBe(0);
        expect(result.stdout).toContain('email');
        expect(result.stdout).toContain('user_uuid');
      });

      it('should handle invalid column indices', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--columns', '999',
          '-y',
        ]);

        expect(result.exitCode).toBe(1);
      });
    });

    describe('--deterministic flag', () => {
      it('should produce consistent output with same seed', async () => {
        await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '--deterministic',
          '--seed', 'test-seed-123',
          '-y',
          '--force',
          '--quiet',
        ]);
        const output1 = await fs.readFile(outputPath, 'utf-8');

        await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '--deterministic',
          '--seed', 'test-seed-123',
          '-y',
          '--force',
          '--quiet',
        ]);
        const output2 = await fs.readFile(outputPath, 'utf-8');

        expect(output1).toBe(output2);
      });

      it('should produce different output with different seeds', async () => {
        await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '--deterministic',
          '--seed', 'seed-1',
          '-y',
          '--force',
          '--quiet',
        ]);
        const output1 = await fs.readFile(outputPath, 'utf-8');

        await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '--deterministic',
          '--seed', 'seed-2',
          '-y',
          '--force',
          '--quiet',
        ]);
        const output2 = await fs.readFile(outputPath, 'utf-8');

        expect(output1).not.toBe(output2);
      });
    });

    describe('--output flag', () => {
      it('should create output file at specified path', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '-y',
          '--quiet',
        ]);

        expect(result.exitCode).toBe(0);
        expect(existsSync(outputPath)).toBe(true);
      });

      it('should fail if output exists without --force', async () => {
        await fs.writeFile(outputPath, 'existing content');

        const result = await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '-y',
        ]);

        expect(result.exitCode).toBe(1);
        expect(result.stderr).toContain('exists');
      });
    });

    describe('--force flag', () => {
      it('should overwrite existing output file', async () => {
        await fs.writeFile(outputPath, 'existing content');

        const result = await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '-y',
          '--force',
          '--quiet',
        ]);

        expect(result.exitCode).toBe(0);
        const content = await fs.readFile(outputPath, 'utf-8');
        expect(content).not.toBe('existing content');
      });
    });

    describe('--quiet flag', () => {
      it('should suppress progress output', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2',
          '-y',
          '--quiet',
        ]);

        expect(result.exitCode).toBe(0);
        expect(result.stdout.length).toBeLessThan(100);
      });
    });

    describe('-y (--yes) flag', () => {
      it('should auto-select PII columns when no columns specified', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--preview',
          '-y',
        ]);

        expect(result.exitCode).toBe(0);
        expect(result.stdout).toContain('Auto-selected');
      });
    });

    describe('error handling', () => {
      it('should handle missing input file', async () => {
        const result = await runCli([
          'run',
          '/nonexistent/file.csv',
          '-y',
        ]);

        expect(result.exitCode).toBe(1);
        expect(result.stderr.toLowerCase()).toMatch(/not found|does not exist|no such file/);
      });
    });

    describe('complete workflow', () => {
      it('should process file end-to-end', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--columns', '2,3',
          '--deterministic',
          '--seed', 'workflow-test',
          '-y',
        ]);

        expect(result.exitCode).toBe(0);
        expect(existsSync(outputPath)).toBe(true);

        const content = await fs.readFile(outputPath, 'utf-8');
        const lines = content.split('\n').filter(l => l.trim());

        expect(lines.length).toBe(6);
        expect(lines[0]).toBe('id,email,user_uuid,created_at,country,status,name');
        expect(lines[1]).toContain('@example.com');
        expect(lines[1]).not.toContain('john.doe');
      });

      it('should process with config file', async () => {
        const result = await runCli([
          'run',
          sampleCsvPath,
          '--output', outputPath,
          '--config', configPath,
          '-y',
          '--force',
        ]);

        expect(result.exitCode).toBe(0);
        expect(existsSync(outputPath)).toBe(true);
      });
    });
  });

  describe('serve command', () => {
    let serveProcess: ChildProcess | null = null;

    afterEach(async () => {
      // Clean up any running serve process
      if (serveProcess && !serveProcess.killed) {
        serveProcess.kill('SIGKILL');
        await new Promise(resolve => setTimeout(resolve, 200));
      }
      serveProcess = null;
    });

    it('should start server and display startup message', async () => {
      // Get a dynamic port to avoid conflicts
      const port = await getAvailablePort();

      // Start the serve command in a background process
      serveProcess = spawn('node', [cliPath, 'serve', '-p', String(port), '--no-open'], {
        cwd: projectRoot,
        stdio: ['pipe', 'pipe', 'pipe'],
      });

      try {
        // Wait for the startup message pattern
        const output = await waitForOutput(serveProcess, 'http://localhost', 8000);

        expect(output).toContain('CSV Anonymizer UI');
        expect(output).toContain(`http://localhost:${port}`);
      } finally {
        // Ensure cleanup
        serveProcess?.kill('SIGTERM');
      }
    }, 15000);

    it('should show port in URL with custom port', async () => {
      // Get a dynamic port to avoid conflicts
      const port = await getAvailablePort();

      // Start the serve command with custom port
      serveProcess = spawn('node', [cliPath, 'serve', '-p', String(port), '--no-open'], {
        cwd: projectRoot,
        stdio: ['pipe', 'pipe', 'pipe'],
      });

      try {
        // Wait for the URL pattern in output
        const output = await waitForOutput(serveProcess, `http://localhost:${port}`, 8000);

        expect(output).toContain(`http://localhost:${port}`);
      } finally {
        // Ensure cleanup
        serveProcess?.kill('SIGTERM');
      }
    }, 15000);
  });

  describe('no arguments', () => {
    it('should show help when no arguments provided', async () => {
      const result = await runCli([], { timeout: 10000 });

      // Help output may go to stdout (Commander.js behavior)
      const combined = result.stdout + result.stderr;
      expect(combined).toContain('csv-anonymizer');
      expect(combined).toContain('headers');
      expect(combined).toContain('preview');
      expect(combined).toContain('run');
      expect(combined).toContain('serve');
    }, 15000);
  });
});
