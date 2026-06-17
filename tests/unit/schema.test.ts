import { describe, it, expect } from 'vitest';
import {
  DataTypeSchema,
  ColumnConfigSchema,
  ConfigSchema,
  validateConfig,
  safeValidateConfig,
} from '../../src/config/schema.js';

describe('DataTypeSchema', () => {
  it('should accept all valid data types', () => {
    const validTypes = [
      'email',
      'uuid',
      'timestamp',
      'numeric_id',
      'country_code',
      'phone',
      'first_name',
      'last_name',
      'full_name',
      'enum',
      'string',
      'unknown',
    ];

    for (const type of validTypes) {
      expect(DataTypeSchema.parse(type)).toBe(type);
    }
  });

  it('should reject invalid data types', () => {
    const invalidTypes = ['invalid', 'EMAIL', 'UUID', 'number', '', null, undefined, 123];

    for (const type of invalidTypes) {
      expect(() => DataTypeSchema.parse(type)).toThrow();
    }
  });
});

describe('ColumnConfigSchema', () => {
  it('should accept valid column config with name only', () => {
    const config = { name: 'email_address' };
    const result = ColumnConfigSchema.parse(config);
    expect(result.name).toBe('email_address');
    expect(result.type).toBeUndefined();
  });

  it('should accept valid column config with all fields', () => {
    const config = {
      name: 'email_address',
      type: 'email',
      strategy: 'custom',
      options: { preserveDomain: true },
    };
    const result = ColumnConfigSchema.parse(config);
    expect(result).toEqual(config);
  });

  it('should reject empty column name', () => {
    const config = { name: '' };
    expect(() => ColumnConfigSchema.parse(config)).toThrow('Column name cannot be empty');
  });

  it('should reject invalid type override', () => {
    const config = { name: 'col', type: 'invalid_type' };
    expect(() => ColumnConfigSchema.parse(config)).toThrow();
  });

  it('should reject unknown options', () => {
    const config = {
      name: 'col',
      options: { unknownOption: true },
    };
    expect(() => ColumnConfigSchema.parse(config)).toThrow();
  });
});

describe('ConfigSchema', () => {
  it('should accept valid config with minimal fields', () => {
    const config = {
      columns: [{ name: 'email' }],
    };
    const result = ConfigSchema.parse(config);
    expect(result.columns).toHaveLength(1);
    expect(result.deterministic).toBe(false);
  });

  it('should accept valid config with all fields', () => {
    const config = {
      columns: [
        { name: 'email', type: 'email', options: { preserveDomain: true } },
        { name: 'id', type: 'numeric_id' },
      ],
      output: 'output.csv',
      deterministic: true,
      seed: 'my-seed',
    };
    const result = ConfigSchema.parse(config);
    expect(result.columns).toHaveLength(2);
    expect(result.output).toBe('output.csv');
    expect(result.deterministic).toBe(true);
    expect(result.seed).toBe('my-seed');
  });

  it('should reject empty columns array', () => {
    const config = { columns: [] };
    expect(() => ConfigSchema.parse(config)).toThrow('At least one column must be configured');
  });

  it('should reject missing columns field', () => {
    const config = { output: 'out.csv' };
    expect(() => ConfigSchema.parse(config)).toThrow();
  });

  it('should set deterministic to false by default', () => {
    const config = { columns: [{ name: 'col' }] };
    const result = ConfigSchema.parse(config);
    expect(result.deterministic).toBe(false);
  });
});

describe('validateConfig', () => {
  it('should return validated config for valid input', () => {
    const config = {
      columns: [{ name: 'email', type: 'email' }],
      deterministic: true,
      seed: 'test-seed',
    };
    const result = validateConfig(config);
    expect(result.columns[0].name).toBe('email');
  });

  it('should throw for invalid input', () => {
    const config = { columns: [] };
    expect(() => validateConfig(config)).toThrow();
  });
});

describe('safeValidateConfig', () => {
  it('should return success result for valid config', () => {
    const config = {
      columns: [{ name: 'email' }],
    };
    const result = safeValidateConfig(config);
    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.columns[0].name).toBe('email');
    }
  });

  it('should return error result for invalid config', () => {
    const config = { columns: [] };
    const result = safeValidateConfig(config);
    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.error.issues.length).toBeGreaterThan(0);
    }
  });

  it('should provide meaningful error messages', () => {
    const config = {
      columns: [{ name: '', type: 'invalid' }],
    };
    const result = safeValidateConfig(config);
    expect(result.success).toBe(false);
    if (!result.success) {
      const messages = result.error.issues.map((i) => i.message);
      expect(messages.some((m) => m.includes('Column name cannot be empty'))).toBe(true);
    }
  });
});
