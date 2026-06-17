import { describe, it, expect } from 'vitest';
import {
  DEFAULT_CONFIG,
  generateRandomSeed,
  generateDefaultOutputPath,
  mergeConfig,
  applyDefaults,
} from '../../src/config/defaults.js';

describe('DEFAULT_CONFIG', () => {
  it('should have deterministic set to false', () => {
    expect(DEFAULT_CONFIG.deterministic).toBe(false);
  });

  it('should have output as undefined', () => {
    expect(DEFAULT_CONFIG.output).toBeUndefined();
  });
});

describe('generateRandomSeed', () => {
  it('should generate a 32-character hex string', () => {
    const seed = generateRandomSeed();
    expect(seed).toMatch(/^[0-9a-f]{32}$/);
  });

  it('should generate different seeds on each call', () => {
    const seed1 = generateRandomSeed();
    const seed2 = generateRandomSeed();
    expect(seed1).not.toBe(seed2);
  });

  it('should generate valid hex strings consistently', () => {
    for (let i = 0; i < 10; i++) {
      const seed = generateRandomSeed();
      expect(seed).toHaveLength(32);
      expect(/^[0-9a-f]+$/.test(seed)).toBe(true);
    }
  });
});

describe('generateDefaultOutputPath', () => {
  it('should add _anonymized before extension', () => {
    expect(generateDefaultOutputPath('data.csv')).toBe('data_anonymized.csv');
    expect(generateDefaultOutputPath('input.json')).toBe('input_anonymized.json');
  });

  it('should handle files with multiple dots', () => {
    expect(generateDefaultOutputPath('my.data.csv')).toBe('my.data_anonymized.csv');
    expect(generateDefaultOutputPath('file.name.test.csv')).toBe('file.name.test_anonymized.csv');
  });

  it('should handle files without extension', () => {
    expect(generateDefaultOutputPath('datafile')).toBe('datafile_anonymized');
  });

  it('should handle paths with directories', () => {
    expect(generateDefaultOutputPath('/path/to/data.csv')).toBe('/path/to/data_anonymized.csv');
    expect(generateDefaultOutputPath('./relative/path.csv')).toBe(
      './relative/path_anonymized.csv'
    );
  });

  it('should handle dotfiles', () => {
    // A dotfile like ".config" has no base name before the dot
    expect(generateDefaultOutputPath('.config')).toBe('_anonymized.config');
  });
});

describe('mergeConfig', () => {
  describe('deterministic precedence', () => {
    it('should use CLI value when provided', () => {
      const result = mergeConfig(
        { deterministic: true },
        { deterministic: false }
      );
      expect(result.deterministic).toBe(true);
    });

    it('should use file config when CLI not provided', () => {
      const result = mergeConfig({}, { deterministic: true });
      expect(result.deterministic).toBe(true);
    });

    it('should use default when neither CLI nor file provided', () => {
      const result = mergeConfig({}, {});
      expect(result.deterministic).toBe(false);
    });

    it('should use default when no config provided', () => {
      const result = mergeConfig({});
      expect(result.deterministic).toBe(false);
    });
  });

  describe('seed precedence', () => {
    it('should use CLI seed when provided', () => {
      const result = mergeConfig(
        { seed: 'cli-seed' },
        { seed: 'file-seed' }
      );
      expect(result.seed).toBe('cli-seed');
    });

    it('should use file seed when CLI not provided', () => {
      const result = mergeConfig({}, { seed: 'file-seed' });
      expect(result.seed).toBe('file-seed');
    });

    it('should generate random seed when neither provided', () => {
      const result = mergeConfig({}, {});
      expect(result.seed).toMatch(/^[0-9a-f]{32}$/);
    });
  });

  describe('output precedence', () => {
    it('should use CLI output when provided', () => {
      const result = mergeConfig(
        { output: 'cli-output.csv' },
        { output: 'file-output.csv' }
      );
      expect(result.output).toBe('cli-output.csv');
    });

    it('should use file output when CLI not provided', () => {
      const result = mergeConfig({}, { output: 'file-output.csv' });
      expect(result.output).toBe('file-output.csv');
    });

    it('should use undefined when neither provided', () => {
      const result = mergeConfig({}, {});
      expect(result.output).toBeUndefined();
    });
  });

  describe('combined precedence', () => {
    it('should correctly merge all options with CLI taking precedence', () => {
      const result = mergeConfig(
        { deterministic: true, output: 'cli.csv' },
        { deterministic: false, seed: 'file-seed', output: 'file.csv' }
      );
      expect(result.deterministic).toBe(true);
      expect(result.seed).toBe('file-seed'); // CLI didn't provide seed
      expect(result.output).toBe('cli.csv');
    });
  });
});

describe('applyDefaults', () => {
  it('should apply defaults to empty config', () => {
    const result = applyDefaults({});
    expect(result.columns).toEqual([]);
    expect(result.deterministic).toBe(false);
    expect(result.output).toBeUndefined();
    expect(result.seed).toMatch(/^[0-9a-f]{32}$/);
  });

  it('should preserve user-provided values', () => {
    const result = applyDefaults({
      columns: [{ name: 'email' }],
      deterministic: true,
      seed: 'my-seed',
      output: 'output.csv',
    });
    expect(result.columns).toHaveLength(1);
    expect(result.columns[0].name).toBe('email');
    expect(result.deterministic).toBe(true);
    expect(result.seed).toBe('my-seed');
    expect(result.output).toBe('output.csv');
  });

  it('should generate seed when not provided', () => {
    const result = applyDefaults({ columns: [{ name: 'col' }] });
    expect(result.seed).toBeDefined();
    expect(result.seed).toHaveLength(32);
  });
});
