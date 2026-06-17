import { describe, it, expect } from 'vitest';
import { parseColumnSelection } from '../../src/config/selection.js';
import { InvalidSelectionError } from '../../src/types/errors.js';

describe('parseColumnSelection', () => {
  const totalColumns = 10;

  describe('single numbers', () => {
    it('should parse single column number', () => {
      expect(parseColumnSelection('1', totalColumns)).toEqual([0]);
      expect(parseColumnSelection('5', totalColumns)).toEqual([4]);
      expect(parseColumnSelection('10', totalColumns)).toEqual([9]);
    });

    it('should handle whitespace around number', () => {
      expect(parseColumnSelection('  3  ', totalColumns)).toEqual([2]);
    });
  });

  describe('comma-separated numbers', () => {
    it('should parse comma-separated numbers', () => {
      expect(parseColumnSelection('1,3,5', totalColumns)).toEqual([0, 2, 4]);
    });

    it('should sort results in ascending order', () => {
      expect(parseColumnSelection('5,1,3', totalColumns)).toEqual([0, 2, 4]);
    });

    it('should remove duplicates', () => {
      expect(parseColumnSelection('1,1,3,3,5', totalColumns)).toEqual([0, 2, 4]);
    });

    it('should handle whitespace around numbers', () => {
      expect(parseColumnSelection(' 1 , 3 , 5 ', totalColumns)).toEqual([0, 2, 4]);
    });
  });

  describe('ranges', () => {
    it('should parse simple range', () => {
      expect(parseColumnSelection('1-5', totalColumns)).toEqual([0, 1, 2, 3, 4]);
    });

    it('should parse single-element range', () => {
      expect(parseColumnSelection('3-3', totalColumns)).toEqual([2]);
    });

    it('should handle whitespace in range', () => {
      expect(parseColumnSelection(' 1 - 5 ', totalColumns)).toEqual([0, 1, 2, 3, 4]);
    });

    it('should parse range at end of columns', () => {
      expect(parseColumnSelection('8-10', totalColumns)).toEqual([7, 8, 9]);
    });
  });

  describe('mixed format', () => {
    it('should parse mixed numbers and ranges', () => {
      expect(parseColumnSelection('1,3-5,7', totalColumns)).toEqual([0, 2, 3, 4, 6]);
    });

    it('should handle overlapping selections', () => {
      expect(parseColumnSelection('1-5,3-7', totalColumns)).toEqual([0, 1, 2, 3, 4, 5, 6]);
    });

    it('should handle multiple ranges', () => {
      expect(parseColumnSelection('1-3,7-9', totalColumns)).toEqual([0, 1, 2, 6, 7, 8]);
    });

    it('should handle complex mixed selection', () => {
      expect(parseColumnSelection('1,3-5,7,9-10', totalColumns)).toEqual([0, 2, 3, 4, 6, 8, 9]);
    });
  });

  describe('special values', () => {
    it('should parse "all" keyword', () => {
      expect(parseColumnSelection('all', totalColumns)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    });

    it('should parse "ALL" (case insensitive)', () => {
      expect(parseColumnSelection('ALL', totalColumns)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    });

    it('should parse "All" (case insensitive)', () => {
      expect(parseColumnSelection('All', totalColumns)).toEqual([0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    });

    it('should parse "none" keyword', () => {
      expect(parseColumnSelection('none', totalColumns)).toEqual([]);
    });

    it('should parse "NONE" (case insensitive)', () => {
      expect(parseColumnSelection('NONE', totalColumns)).toEqual([]);
    });

    it('should parse "None" (case insensitive)', () => {
      expect(parseColumnSelection('None', totalColumns)).toEqual([]);
    });

    it('should handle whitespace around special values', () => {
      expect(parseColumnSelection('  all  ', totalColumns)).toEqual([
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
      ]);
      expect(parseColumnSelection('  none  ', totalColumns)).toEqual([]);
    });
  });

  describe('edge cases with totalColumns', () => {
    it('should work with totalColumns = 1', () => {
      expect(parseColumnSelection('1', 1)).toEqual([0]);
      expect(parseColumnSelection('all', 1)).toEqual([0]);
      expect(parseColumnSelection('none', 1)).toEqual([]);
    });

    it('should work with large totalColumns', () => {
      const result = parseColumnSelection('all', 100);
      expect(result).toHaveLength(100);
      expect(result[0]).toBe(0);
      expect(result[99]).toBe(99);
    });
  });

  describe('invalid input', () => {
    it('should throw for empty input', () => {
      expect(() => parseColumnSelection('', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for whitespace-only input', () => {
      expect(() => parseColumnSelection('   ', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for column number out of range', () => {
      expect(() => parseColumnSelection('11', totalColumns)).toThrow(InvalidSelectionError);
      expect(() => parseColumnSelection('100', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for zero', () => {
      expect(() => parseColumnSelection('0', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for negative numbers', () => {
      expect(() => parseColumnSelection('-1', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for non-numeric input', () => {
      expect(() => parseColumnSelection('abc', totalColumns)).toThrow(InvalidSelectionError);
      expect(() => parseColumnSelection('1,abc,3', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for float numbers', () => {
      expect(() => parseColumnSelection('1.5', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for invalid range (start > end)', () => {
      expect(() => parseColumnSelection('5-1', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for range with out-of-bound start', () => {
      expect(() => parseColumnSelection('15-20', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for range with out-of-bound end', () => {
      expect(() => parseColumnSelection('5-15', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for consecutive commas', () => {
      expect(() => parseColumnSelection('1,,3', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for trailing comma', () => {
      expect(() => parseColumnSelection('1,3,', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for leading comma', () => {
      expect(() => parseColumnSelection(',1,3', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for malformed range (missing start)', () => {
      expect(() => parseColumnSelection('-5', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for malformed range (missing end)', () => {
      expect(() => parseColumnSelection('5-', totalColumns)).toThrow(InvalidSelectionError);
    });

    it('should throw for multiple hyphens in range', () => {
      expect(() => parseColumnSelection('1-3-5', totalColumns)).toThrow(InvalidSelectionError);
    });
  });

  describe('error messages', () => {
    it('should include original input in error message', () => {
      try {
        parseColumnSelection('invalid', totalColumns);
        expect.fail('Should have thrown');
      } catch (error) {
        expect(error).toBeInstanceOf(InvalidSelectionError);
        expect((error as InvalidSelectionError).input).toBe('invalid');
      }
    });

    it('should provide helpful message for out-of-range', () => {
      try {
        parseColumnSelection('15', totalColumns);
        expect.fail('Should have thrown');
      } catch (error) {
        expect(error).toBeInstanceOf(InvalidSelectionError);
        expect((error as InvalidSelectionError).message).toContain('out of range');
        expect((error as InvalidSelectionError).message).toContain('10');
      }
    });

    it('should provide helpful message for invalid format', () => {
      try {
        parseColumnSelection('abc', totalColumns);
        expect.fail('Should have thrown');
      } catch (error) {
        expect(error).toBeInstanceOf(InvalidSelectionError);
        expect((error as InvalidSelectionError).message).toContain('not a valid');
      }
    });
  });
});
