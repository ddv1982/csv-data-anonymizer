import { describe, it, expect, beforeEach } from 'vitest';
import type { TransformContext } from '../../src/types/index.js';
import { emailStrategy } from '../../src/strategies/email.js';
import { uuidStrategy } from '../../src/strategies/uuid.js';
import { timestampStrategy } from '../../src/strategies/timestamp.js';
import { numericIdStrategy } from '../../src/strategies/numericId.js';
import { genericStringStrategy } from '../../src/strategies/generic.js';
import {
  getStrategy,
  createPassThroughStrategy,
  isEmptyValue,
} from '../../src/strategies/index.js';

/**
 * Helper to create a TransformContext for testing.
 */
function createContext(overrides: Partial<TransformContext> = {}): TransformContext {
  return {
    columnName: 'test_column',
    columnIndex: 0,
    rowIndex: 0,
    seed: 'test-seed',
    deterministic: false,
    emptyFormat: 'empty_string',
    ...overrides,
  };
}

describe('Strategy Registry', () => {
  describe('isEmptyValue', () => {
    it('returns true for empty string', () => {
      expect(isEmptyValue('')).toBe(true);
    });

    it('returns true for "null" string (case insensitive)', () => {
      expect(isEmptyValue('null')).toBe(true);
      expect(isEmptyValue('NULL')).toBe(true);
      expect(isEmptyValue('Null')).toBe(true);
    });

    it('returns false for non-empty values', () => {
      expect(isEmptyValue('value')).toBe(false);
      expect(isEmptyValue(' ')).toBe(false);
      expect(isEmptyValue('0')).toBe(false);
    });
  });

  describe('getStrategy', () => {
    it('returns a strategy for known data types', () => {
      expect(getStrategy('email')).toBeDefined();
      expect(getStrategy('uuid')).toBeDefined();
      expect(getStrategy('timestamp')).toBeDefined();
      expect(getStrategy('numeric_id')).toBeDefined();
      expect(getStrategy('string')).toBeDefined();
    });

    it('returns pass-through strategy for country_code', () => {
      const strategy = getStrategy('country_code');
      const ctx = createContext({ deterministic: true });
      expect(strategy.transform('US', ctx)).toBe('US');
    });

    it('returns pass-through strategy for enum', () => {
      const strategy = getStrategy('enum');
      const ctx = createContext({ deterministic: true });
      expect(strategy.transform('active', ctx)).toBe('active');
    });
  });

  describe('createPassThroughStrategy', () => {
    it('returns values unchanged', () => {
      const strategy = createPassThroughStrategy();
      const ctx = createContext();
      expect(strategy.transform('any value', ctx)).toBe('any value');
    });

    it('preserves empty values', () => {
      const strategy = createPassThroughStrategy();
      const ctx = createContext();
      expect(strategy.transform('', ctx)).toBe('');
      expect(strategy.transform('null', ctx)).toBe('null');
    });
  });
});

describe('Email Strategy', () => {
  it('preserves domain', () => {
    const ctx = createContext({ deterministic: true });
    const result = emailStrategy.transform('john.doe@example.com', ctx);
    expect(result).toMatch(/@example\.com$/);
    expect(result).not.toBe('john.doe@example.com');
  });

  it('preserves domain for complex domains', () => {
    const ctx = createContext({ deterministic: true });
    const result = emailStrategy.transform('user@subdomain.example.co.uk', ctx);
    expect(result).toMatch(/@subdomain\.example\.co\.uk$/);
  });

  it('is deterministic with same seed', () => {
    const ctx = createContext({ deterministic: true, seed: 'test-seed' });
    const result1 = emailStrategy.transform('test@gmail.com', ctx);
    const result2 = emailStrategy.transform('test@gmail.com', ctx);
    expect(result1).toBe(result2);
  });

  it('produces different results with different seeds', () => {
    const ctx1 = createContext({ deterministic: true, seed: 'seed1' });
    const ctx2 = createContext({ deterministic: true, seed: 'seed2' });
    const result1 = emailStrategy.transform('test@gmail.com', ctx1);
    const result2 = emailStrategy.transform('test@gmail.com', ctx2);
    expect(result1).not.toBe(result2);
  });

  it('preserves empty values', () => {
    const ctx = createContext({ deterministic: true });
    expect(emailStrategy.transform('', ctx)).toBe('');
    expect(emailStrategy.transform('null', ctx)).toBe('null');
    expect(emailStrategy.transform('NULL', ctx)).toBe('NULL');
  });

  it('handles invalid email gracefully', () => {
    const ctx = createContext({ deterministic: true });
    const result = emailStrategy.transform('not-an-email', ctx);
    // Should return original if no @ found
    expect(result).toBe('not-an-email');
  });
});

describe('UUID Strategy', () => {
  const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

  it('produces valid UUID v4 format', () => {
    const ctx = createContext({ deterministic: true });
    const result = uuidStrategy.transform('550e8400-e29b-41d4-a716-446655440000', ctx);
    expect(result.toLowerCase()).toMatch(uuidRegex);
  });

  it('is deterministic with same seed', () => {
    const ctx = createContext({ deterministic: true, seed: 'test-seed' });
    const result1 = uuidStrategy.transform('550e8400-e29b-41d4-a716-446655440000', ctx);
    const result2 = uuidStrategy.transform('550e8400-e29b-41d4-a716-446655440000', ctx);
    expect(result1).toBe(result2);
  });

  it('produces different results for different inputs', () => {
    const ctx = createContext({ deterministic: true });
    const result1 = uuidStrategy.transform('550e8400-e29b-41d4-a716-446655440000', ctx);
    const result2 = uuidStrategy.transform('660e8400-e29b-41d4-a716-446655440000', ctx);
    expect(result1).not.toBe(result2);
  });

  it('produces different results with different seeds', () => {
    const ctx1 = createContext({ deterministic: true, seed: 'seed1' });
    const ctx2 = createContext({ deterministic: true, seed: 'seed2' });
    const result1 = uuidStrategy.transform('550e8400-e29b-41d4-a716-446655440000', ctx1);
    const result2 = uuidStrategy.transform('550e8400-e29b-41d4-a716-446655440000', ctx2);
    expect(result1).not.toBe(result2);
  });

  it('preserves uppercase format', () => {
    const ctx = createContext({ deterministic: true });
    const result = uuidStrategy.transform('550E8400-E29B-41D4-A716-446655440000', ctx);
    expect(result).toBe(result.toUpperCase());
  });

  it('preserves lowercase format', () => {
    const ctx = createContext({ deterministic: true });
    const result = uuidStrategy.transform('550e8400-e29b-41d4-a716-446655440000', ctx);
    expect(result).toBe(result.toLowerCase());
  });

  it('preserves empty values', () => {
    const ctx = createContext({ deterministic: true });
    expect(uuidStrategy.transform('', ctx)).toBe('');
    expect(uuidStrategy.transform('null', ctx)).toBe('null');
  });
});

describe('Timestamp Strategy', () => {
  describe('format preservation', () => {
    it('preserves date-only format', () => {
      const ctx = createContext({ deterministic: true });
      const result = timestampStrategy.transform('2024-06-15', ctx);
      expect(result).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    });

    it('preserves datetime format with T separator', () => {
      const ctx = createContext({ deterministic: true });
      const result = timestampStrategy.transform('2024-06-15T10:30:45', ctx);
      expect(result).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}$/);
    });

    it('preserves datetime format with space separator', () => {
      const ctx = createContext({ deterministic: true });
      const result = timestampStrategy.transform('2024-06-15 10:30:45', ctx);
      expect(result).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}$/);
    });

    it('preserves microseconds precision', () => {
      const ctx = createContext({ deterministic: true });
      const result = timestampStrategy.transform('2024-06-15 10:30:45.123456', ctx);
      expect(result).toMatch(/^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d+$/);
    });

    it('preserves time portion unchanged', () => {
      const ctx = createContext({ deterministic: true });
      const result = timestampStrategy.transform('2024-06-15 10:30:45', ctx);
      // Time should remain the same, only date changes
      expect(result).toMatch(/ 10:30:45$/);
    });
  });

  describe('determinism', () => {
    it('is deterministic with same seed', () => {
      const ctx = createContext({ deterministic: true, seed: 'test-seed' });
      const result1 = timestampStrategy.transform('2024-06-15', ctx);
      const result2 = timestampStrategy.transform('2024-06-15', ctx);
      expect(result1).toBe(result2);
    });

    it('produces different results with different seeds', () => {
      const ctx1 = createContext({ deterministic: true, seed: 'seed1' });
      const ctx2 = createContext({ deterministic: true, seed: 'seed2' });
      const result1 = timestampStrategy.transform('2024-06-15', ctx1);
      const result2 = timestampStrategy.transform('2024-06-15', ctx2);
      expect(result1).not.toBe(result2);
    });
  });

  describe('empty handling', () => {
    it('preserves empty values', () => {
      const ctx = createContext({ deterministic: true });
      expect(timestampStrategy.transform('', ctx)).toBe('');
      expect(timestampStrategy.transform('null', ctx)).toBe('null');
    });
  });

  describe('date offset', () => {
    it('offsets date within reasonable range', () => {
      const ctx = createContext({ deterministic: true });
      const originalDate = new Date('2024-06-15');
      const result = timestampStrategy.transform('2024-06-15', ctx);
      const resultDate = new Date(result);

      // Check that offset is within ±365 days
      const diffDays = Math.abs((resultDate.getTime() - originalDate.getTime()) / (1000 * 60 * 60 * 24));
      expect(diffDays).toBeLessThanOrEqual(365);
    });
  });
});

describe('Numeric ID Strategy', () => {
  describe('digit count preservation', () => {
    it('preserves exact digit count for 4-digit number', () => {
      const ctx = createContext({ deterministic: true });
      const result = numericIdStrategy.transform('1234', ctx);
      expect(result).toHaveLength(4);
      expect(result).toMatch(/^\d{4}$/);
    });

    it('preserves exact digit count for 7-digit number', () => {
      const ctx = createContext({ deterministic: true });
      const result = numericIdStrategy.transform('1234567', ctx);
      expect(result).toHaveLength(7);
      expect(result).toMatch(/^\d{7}$/);
    });

    it('preserves exact digit count for 10-digit number', () => {
      const ctx = createContext({ deterministic: true });
      const result = numericIdStrategy.transform('1234567890', ctx);
      expect(result).toHaveLength(10);
      expect(result).toMatch(/^\d{10}$/);
    });

    it('preserves leading zeros', () => {
      const ctx = createContext({ deterministic: true });
      const result = numericIdStrategy.transform('00012345', ctx);
      expect(result).toHaveLength(8);
      expect(result).toMatch(/^000\d{5}$/);
    });
  });

  describe('determinism', () => {
    it('is deterministic with same seed', () => {
      const ctx = createContext({ deterministic: true, seed: 'test-seed' });
      const result1 = numericIdStrategy.transform('1234567', ctx);
      const result2 = numericIdStrategy.transform('1234567', ctx);
      expect(result1).toBe(result2);
    });

    it('produces different results for different inputs', () => {
      const ctx = createContext({ deterministic: true });
      const result1 = numericIdStrategy.transform('1234567', ctx);
      const result2 = numericIdStrategy.transform('7654321', ctx);
      expect(result1).not.toBe(result2);
    });

    it('produces different results with different seeds', () => {
      const ctx1 = createContext({ deterministic: true, seed: 'seed1' });
      const ctx2 = createContext({ deterministic: true, seed: 'seed2' });
      const result1 = numericIdStrategy.transform('1234567', ctx1);
      const result2 = numericIdStrategy.transform('1234567', ctx2);
      expect(result1).not.toBe(result2);
    });
  });

  describe('empty handling', () => {
    it('preserves empty values', () => {
      const ctx = createContext({ deterministic: true });
      expect(numericIdStrategy.transform('', ctx)).toBe('');
      expect(numericIdStrategy.transform('null', ctx)).toBe('null');
    });
  });
});

describe('Generic String Strategy', () => {
  describe('length preservation', () => {
    it('generates string within ±20% of original length', () => {
      const ctx = createContext({ deterministic: true });
      const original = 'This is a test string';
      const result = genericStringStrategy.transform(original, ctx);

      const minLength = Math.floor(original.length * 0.8);
      const maxLength = Math.ceil(original.length * 1.2);

      expect(result.length).toBeGreaterThanOrEqual(minLength);
      expect(result.length).toBeLessThanOrEqual(maxLength);
    });

    it('handles short strings', () => {
      const ctx = createContext({ deterministic: true });
      const result = genericStringStrategy.transform('test', ctx);
      expect(result.length).toBeGreaterThanOrEqual(3); // 4 * 0.8 = 3.2
      expect(result.length).toBeLessThanOrEqual(5); // 4 * 1.2 = 4.8
    });

    it('handles long strings', () => {
      const ctx = createContext({ deterministic: true });
      const original = 'A'.repeat(100);
      const result = genericStringStrategy.transform(original, ctx);

      expect(result.length).toBeGreaterThanOrEqual(80);
      expect(result.length).toBeLessThanOrEqual(120);
    });
  });

  describe('determinism', () => {
    it('is deterministic with same seed', () => {
      const ctx = createContext({ deterministic: true, seed: 'test-seed' });
      const result1 = genericStringStrategy.transform('test string', ctx);
      const result2 = genericStringStrategy.transform('test string', ctx);
      expect(result1).toBe(result2);
    });

    it('produces different results for different inputs', () => {
      const ctx = createContext({ deterministic: true });
      const result1 = genericStringStrategy.transform('string one', ctx);
      const result2 = genericStringStrategy.transform('string two', ctx);
      expect(result1).not.toBe(result2);
    });
  });

  describe('empty handling', () => {
    it('preserves empty values', () => {
      const ctx = createContext({ deterministic: true });
      expect(genericStringStrategy.transform('', ctx)).toBe('');
      expect(genericStringStrategy.transform('null', ctx)).toBe('null');
    });
  });
});
