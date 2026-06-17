import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { writeFileSync, mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { loadConfig } from '../../src/config/loader.js';
import { ConfigValidationError, FileNotFoundError } from '../../src/types/errors.js';

describe('loadConfig', () => {
  const testDir = join(process.cwd(), 'tests', 'fixtures', 'temp-loader');

  beforeAll(() => {
    mkdirSync(testDir, { recursive: true });
  });

  afterAll(() => {
    rmSync(testDir, { recursive: true, force: true });
  });

  describe('valid configuration', () => {
    it('should load a valid YAML config file', () => {
      const configPath = join(process.cwd(), 'tests', 'fixtures', 'config.yml');
      const config = loadConfig(configPath);

      expect(config.columns).toHaveLength(3);
      expect(config.columns[0].name).toBe('email_address');
      expect(config.columns[0].type).toBe('email');
      expect(config.columns[0].options?.preserveDomain).toBe(true);
      expect(config.columns[1].name).toBe('customer_id');
      expect(config.columns[1].type).toBe('uuid');
      expect(config.columns[2].name).toBe('id');
      expect(config.columns[2].type).toBe('numeric_id');
      expect(config.output).toBe('output_anonymized.csv');
      expect(config.deterministic).toBe(true);
      expect(config.seed).toBe('reproducible-seed-value');
    });

    it('should load config with minimal fields', () => {
      const configPath = join(testDir, 'minimal.yml');
      writeFileSync(
        configPath,
        `
columns:
  - name: email
`
      );

      const config = loadConfig(configPath);

      expect(config.columns).toHaveLength(1);
      expect(config.columns[0].name).toBe('email');
      expect(config.deterministic).toBe(false); // default value
    });

    it('should parse all valid data types', () => {
      const configPath = join(testDir, 'all-types.yml');
      writeFileSync(
        configPath,
        `
columns:
  - name: col1
    type: email
  - name: col2
    type: uuid
  - name: col3
    type: timestamp
  - name: col4
    type: numeric_id
  - name: col5
    type: country_code
  - name: col6
    type: phone
  - name: col7
    type: first_name
  - name: col8
    type: last_name
  - name: col9
    type: full_name
  - name: col10
    type: enum
  - name: col11
    type: string
  - name: col12
    type: unknown
`
      );

      const config = loadConfig(configPath);
      expect(config.columns).toHaveLength(12);
    });
  });

  describe('invalid configuration', () => {
    it('should throw ConfigValidationError for empty columns array', () => {
      const configPath = join(process.cwd(), 'tests', 'fixtures', 'invalid-config.yml');

      expect(() => loadConfig(configPath)).toThrow(ConfigValidationError);
      try {
        loadConfig(configPath);
      } catch (error) {
        expect(error).toBeInstanceOf(ConfigValidationError);
        expect((error as ConfigValidationError).message).toContain('At least one column');
      }
    });

    it('should throw ConfigValidationError for empty column name', () => {
      const configPath = join(testDir, 'empty-name.yml');
      writeFileSync(
        configPath,
        `
columns:
  - name: ""
`
      );

      expect(() => loadConfig(configPath)).toThrow(ConfigValidationError);
    });

    it('should throw ConfigValidationError for invalid type', () => {
      const configPath = join(testDir, 'invalid-type.yml');
      writeFileSync(
        configPath,
        `
columns:
  - name: col
    type: invalid_type
`
      );

      expect(() => loadConfig(configPath)).toThrow(ConfigValidationError);
    });

    it('should throw ConfigValidationError for unknown options', () => {
      const configPath = join(testDir, 'unknown-options.yml');
      writeFileSync(
        configPath,
        `
columns:
  - name: col
    options:
      unknownOption: true
`
      );

      expect(() => loadConfig(configPath)).toThrow(ConfigValidationError);
    });

    it('should throw ConfigValidationError for empty file', () => {
      const configPath = join(testDir, 'empty.yml');
      writeFileSync(configPath, '');

      expect(() => loadConfig(configPath)).toThrow(ConfigValidationError);
      try {
        loadConfig(configPath);
      } catch (error) {
        expect(error).toBeInstanceOf(ConfigValidationError);
        expect((error as ConfigValidationError).message).toContain('empty');
      }
    });

    it('should throw ConfigValidationError for invalid YAML syntax', () => {
      const configPath = join(testDir, 'invalid-yaml.yml');
      writeFileSync(configPath, 'columns:\n  - name: test\n    invalid yaml here');

      expect(() => loadConfig(configPath)).toThrow(ConfigValidationError);
    });
  });

  describe('missing file', () => {
    it('should throw FileNotFoundError for non-existent file', () => {
      const configPath = join(testDir, 'nonexistent.yml');

      expect(() => loadConfig(configPath)).toThrow(FileNotFoundError);
      try {
        loadConfig(configPath);
      } catch (error) {
        expect(error).toBeInstanceOf(FileNotFoundError);
        expect((error as FileNotFoundError).path).toBe(configPath);
      }
    });

    it('should include file path in error message', () => {
      const configPath = '/path/to/missing/config.yml';

      expect(() => loadConfig(configPath)).toThrow(FileNotFoundError);
      try {
        loadConfig(configPath);
      } catch (error) {
        expect((error as FileNotFoundError).message).toContain(configPath);
      }
    });
  });

  describe('error details', () => {
    it('should include detailed validation issues in ConfigValidationError', () => {
      const configPath = join(testDir, 'multiple-errors.yml');
      writeFileSync(
        configPath,
        `
columns:
  - name: ""
    type: invalid
`
      );

      try {
        loadConfig(configPath);
        expect.fail('Should have thrown');
      } catch (error) {
        expect(error).toBeInstanceOf(ConfigValidationError);
        const configError = error as ConfigValidationError;
        expect(configError.issues.length).toBeGreaterThan(0);
      }
    });
  });
});
