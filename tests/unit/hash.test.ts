import { describe, it, expect } from 'vitest';
import {
  deterministicHash,
  deterministicNumber,
  deterministicString,
  deterministicUuid,
} from '../../src/utils/hash.js';

describe('Hash Utilities', () => {
  describe('deterministicHash', () => {
    it('returns consistent output for same input and seed', () => {
      const result1 = deterministicHash('test-value', 'test-seed');
      const result2 = deterministicHash('test-value', 'test-seed');
      expect(result1).toBe(result2);
    });

    it('returns different output for different values', () => {
      const result1 = deterministicHash('value1', 'seed');
      const result2 = deterministicHash('value2', 'seed');
      expect(result1).not.toBe(result2);
    });

    it('returns different output for different seeds', () => {
      const result1 = deterministicHash('value', 'seed1');
      const result2 = deterministicHash('value', 'seed2');
      expect(result1).not.toBe(result2);
    });

    it('returns a 64-character hex string', () => {
      const result = deterministicHash('test', 'seed');
      expect(result).toHaveLength(64);
      expect(result).toMatch(/^[0-9a-f]{64}$/);
    });
  });

  describe('deterministicNumber', () => {
    it('returns consistent output for same input and seed', () => {
      const result1 = deterministicNumber('test', 'seed', 0, 100);
      const result2 = deterministicNumber('test', 'seed', 0, 100);
      expect(result1).toBe(result2);
    });

    it('returns number within specified range', () => {
      for (let i = 0; i < 100; i++) {
        const result = deterministicNumber(`value-${i}`, 'seed', 10, 50);
        expect(result).toBeGreaterThanOrEqual(10);
        expect(result).toBeLessThanOrEqual(50);
      }
    });

    it('returns different numbers for different values', () => {
      const results = new Set<number>();
      for (let i = 0; i < 50; i++) {
        results.add(deterministicNumber(`value-${i}`, 'seed', 0, 1000));
      }
      // Should have some variety in results
      expect(results.size).toBeGreaterThan(20);
    });
  });

  describe('deterministicString', () => {
    it('returns consistent output for same input and seed', () => {
      const result1 = deterministicString('test', 'seed', 10);
      const result2 = deterministicString('test', 'seed', 10);
      expect(result1).toBe(result2);
    });

    it('returns string of specified length', () => {
      const result = deterministicString('test', 'seed', 15);
      expect(result).toHaveLength(15);
    });

    it('uses default charset correctly', () => {
      const result = deterministicString('test', 'seed', 50);
      expect(result).toMatch(/^[a-z0-9]{50}$/);
    });

    it('uses custom charset correctly', () => {
      const result = deterministicString('test', 'seed', 20, 'ABC123');
      expect(result).toMatch(/^[ABC123]{20}$/);
    });

    it('returns different strings for different values', () => {
      const result1 = deterministicString('value1', 'seed', 10);
      const result2 = deterministicString('value2', 'seed', 10);
      expect(result1).not.toBe(result2);
    });
  });

  describe('deterministicUuid', () => {
    it('returns consistent output for same input and seed', () => {
      const result1 = deterministicUuid('test', 'seed');
      const result2 = deterministicUuid('test', 'seed');
      expect(result1).toBe(result2);
    });

    it('returns valid UUID v4 format', () => {
      const result = deterministicUuid('test', 'seed');
      const uuidRegex = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
      expect(result).toMatch(uuidRegex);
    });

    it('sets version to 4', () => {
      const result = deterministicUuid('test', 'seed');
      // 13th character should be '4'
      expect(result[14]).toBe('4');
    });

    it('sets variant bits correctly', () => {
      const result = deterministicUuid('test', 'seed');
      // 17th character (19th with dashes) should be 8, 9, a, or b
      expect(['8', '9', 'a', 'b']).toContain(result[19]);
    });

    it('returns different UUIDs for different values', () => {
      const result1 = deterministicUuid('value1', 'seed');
      const result2 = deterministicUuid('value2', 'seed');
      expect(result1).not.toBe(result2);
    });

    it('returns different UUIDs for different seeds', () => {
      const result1 = deterministicUuid('value', 'seed1');
      const result2 = deterministicUuid('value', 'seed2');
      expect(result1).not.toBe(result2);
    });
  });
});
