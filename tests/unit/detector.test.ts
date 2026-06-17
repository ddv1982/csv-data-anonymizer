import { describe, it, expect } from 'vitest';
import {
  detectColumnType,
  classifyPiiRisk,
  detectEmptyFormat,
} from '../../src/core/detector.js';
import type { DataType } from '../../src/types/index.js';

describe('detectColumnType', () => {
  describe('email detection', () => {
    it('detects email with high confidence when ≥80% match', () => {
      const values = [
        'user1@example.com',
        'user2@example.com',
        'user3@example.com',
        'user4@example.com',
        'user5@example.com',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('email');
      expect(result.confidence).toBe('high');
      expect(result.sampleMatches).toBe(5);
      expect(result.totalSamples).toBe(5);
    });

    it('detects email with medium confidence when 50-80% match', () => {
      const values = [
        'user1@example.com',
        'user2@example.com',
        'user3@example.com',
        'not-email',
        'also-not-email',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('email');
      expect(result.confidence).toBe('medium');
    });

    it('skips empty values when calculating match percentage', () => {
      const values = [
        'user1@example.com',
        '',
        'user2@example.com',
        'null',
        'user3@example.com',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('email');
      expect(result.confidence).toBe('high');
      expect(result.sampleMatches).toBe(3);
    });
  });

  describe('UUID detection', () => {
    it('detects UUID with high confidence', () => {
      const values = [
        '550e8400-e29b-41d4-a716-446655440000',
        '6ba7b810-9dad-11d1-80b4-00c04fd430c8',
        '6ba7b811-9dad-11d1-80b4-00c04fd430c8',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('uuid');
      expect(result.confidence).toBe('high');
    });

    it('detects mixed case UUIDs', () => {
      const values = [
        '550E8400-E29B-41D4-A716-446655440000',
        '6BA7B810-9DAD-11D1-80B4-00C04FD430C8',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('uuid');
      expect(result.confidence).toBe('high');
    });
  });

  describe('timestamp detection', () => {
    it('detects date-only timestamps', () => {
      const values = [
        '2024-01-15',
        '2024-02-20',
        '2024-03-25',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('timestamp');
      expect(result.confidence).toBe('high');
    });

    it('detects datetime timestamps with T separator', () => {
      const values = [
        '2024-01-15T10:30:00',
        '2024-02-20T14:45:30',
        '2024-03-25T08:15:45',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('timestamp');
      expect(result.confidence).toBe('high');
    });

    it('detects datetime timestamps with space separator', () => {
      const values = [
        '2024-01-15 10:30:00',
        '2024-02-20 14:45:30',
        '2024-03-25 08:15:45',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('timestamp');
      expect(result.confidence).toBe('high');
    });

    it('detects timestamps with microseconds', () => {
      const values = [
        '2024-01-15T10:30:00.123456',
        '2024-02-20T14:45:30.789012',
        '2024-03-25T08:15:45.000000',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('timestamp');
      expect(result.confidence).toBe('high');
    });
  });

  describe('phone detection', () => {
    it('detects international phone numbers', () => {
      const values = [
        '+1 234 567 8900',
        '+44 20 7123 4567',
        '+49 30 12345678',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('phone');
      expect(result.confidence).toBe('high');
    });

    it('detects various phone formats', () => {
      const values = [
        '(123) 456-7890',
        '123-456-7890',
        '1234567890',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('phone');
      expect(result.confidence).toBe('high');
    });
  });

  describe('numeric_id detection', () => {
    it('detects numeric IDs with 4+ digits', () => {
      const values = [
        '123456',
        '789012',
        '345678',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('numeric_id');
      expect(result.confidence).toBe('high');
    });

    it('detects 9-digit numeric IDs (below phone threshold)', () => {
      const values = [
        '123456789',
        '987654321',
        '111122223',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('numeric_id');
      expect(result.confidence).toBe('high');
    });

    it('notes that 10+ digit pure numbers match phone pattern first', () => {
      // This is expected behavior: phone pattern matches digits of 10+ length
      // The priority order is designed this way since phone numbers are
      // more commonly 10+ digits, while shorter IDs are more likely numeric_id
      const values = [
        '1234567890123',
        '9876543210987',
        '1111222233334',
      ];
      const result = detectColumnType(values);
      // These long numeric strings match phone pattern due to priority
      expect(result.type).toBe('phone');
    });
  });

  describe('country_code detection', () => {
    it('detects ISO country codes', () => {
      const values = [
        'US',
        'GB',
        'DE',
        'FR',
        'JP',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('country_code');
      expect(result.confidence).toBe('high');
    });
  });

  describe('enum detection', () => {
    it('detects enum when unique values ≤20 and total >10', () => {
      const values = [
        'active', 'inactive', 'pending',
        'active', 'inactive', 'pending',
        'active', 'inactive', 'pending',
        'active', 'inactive', 'pending',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('enum');
      expect(result.confidence).toBe('high');
    });

    it('does not detect enum when total samples ≤10', () => {
      const values = [
        'active', 'inactive', 'pending',
        'active', 'inactive',
      ];
      const result = detectColumnType(values);
      expect(result.type).not.toBe('enum');
    });

    it('does not detect enum when unique values >20', () => {
      const values: string[] = [];
      for (let i = 0; i < 25; i++) {
        values.push(`value${i}`);
      }
      const result = detectColumnType(values);
      expect(result.type).not.toBe('enum');
    });
  });

  describe('detection priority', () => {
    it('prioritizes email over numeric patterns for email addresses', () => {
      const values = [
        'user123@example.com',
        'admin456@test.org',
        'info789@domain.net',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('email');
    });

    it('prioritizes UUID over other patterns', () => {
      const values = [
        '550e8400-e29b-41d4-a716-446655440000',
        '6ba7b810-9dad-11d1-80b4-00c04fd430c8',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('uuid');
    });
  });

  describe('fallback behavior', () => {
    it('falls back to string type for unrecognized patterns', () => {
      const values = [
        'Some random text',
        'Another value here',
        'More text content',
      ];
      const result = detectColumnType(values);
      expect(result.type).toBe('string');
      expect(result.confidence).toBe('low');
    });

    it('returns unknown for all empty values', () => {
      const values = ['', '', 'null', 'NULL'];
      const result = detectColumnType(values);
      expect(result.type).toBe('unknown');
      expect(result.confidence).toBe('low');
      expect(result.sampleMatches).toBe(0);
    });

    it('returns unknown for empty array', () => {
      const result = detectColumnType([]);
      expect(result.type).toBe('unknown');
      expect(result.confidence).toBe('low');
    });
  });

  describe('confidence calculation', () => {
    it('returns high confidence for 100% match', () => {
      const values = ['user@example.com', 'test@test.com', 'admin@admin.org'];
      const result = detectColumnType(values);
      expect(result.confidence).toBe('high');
    });

    it('returns high confidence for exactly 80% match', () => {
      const values = [
        'user1@example.com',
        'user2@example.com',
        'user3@example.com',
        'user4@example.com',
        'not-email',
      ];
      const result = detectColumnType(values);
      expect(result.confidence).toBe('high');
    });

    it('returns medium confidence for 79% match', () => {
      // Need approximately 79% match - 11 out of 14 = 78.5%
      const values = [
        'user1@example.com',
        'user2@example.com',
        'user3@example.com',
        'user4@example.com',
        'user5@example.com',
        'user6@example.com',
        'user7@example.com',
        'user8@example.com',
        'user9@example.com',
        'user10@example.com',
        'user11@example.com',
        'not-email-1',
        'not-email-2',
        'not-email-3',
      ];
      const result = detectColumnType(values);
      expect(result.confidence).toBe('medium');
    });

    it('returns medium confidence for exactly 50% match', () => {
      const values = [
        'user1@example.com',
        'not-email-1',
      ];
      const result = detectColumnType(values);
      expect(result.confidence).toBe('medium');
    });
  });
});

describe('classifyPiiRisk', () => {
  describe('high risk types', () => {
    it('classifies email as high risk', () => {
      expect(classifyPiiRisk('email')).toBe('high');
    });

    it('classifies phone as high risk', () => {
      expect(classifyPiiRisk('phone')).toBe('high');
    });

    it('classifies full_name as high risk', () => {
      expect(classifyPiiRisk('full_name')).toBe('high');
    });
  });

  describe('medium risk types', () => {
    it('classifies first_name as medium risk', () => {
      expect(classifyPiiRisk('first_name')).toBe('medium');
    });

    it('classifies last_name as medium risk', () => {
      expect(classifyPiiRisk('last_name')).toBe('medium');
    });

    it('classifies uuid as medium risk', () => {
      expect(classifyPiiRisk('uuid')).toBe('medium');
    });

    it('classifies numeric_id as medium risk', () => {
      expect(classifyPiiRisk('numeric_id')).toBe('medium');
    });
  });

  describe('low risk types', () => {
    it('classifies timestamp as low risk', () => {
      expect(classifyPiiRisk('timestamp')).toBe('low');
    });

    it('classifies country_code as low risk', () => {
      expect(classifyPiiRisk('country_code')).toBe('low');
    });

    it('classifies enum as low risk', () => {
      expect(classifyPiiRisk('enum')).toBe('low');
    });

    it('classifies string as low risk', () => {
      expect(classifyPiiRisk('string')).toBe('low');
    });

    it('classifies unknown as low risk', () => {
      expect(classifyPiiRisk('unknown')).toBe('low');
    });
  });

  describe('all types coverage', () => {
    it('handles all DataType values', () => {
      const allTypes: DataType[] = [
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

      for (const type of allTypes) {
        const risk = classifyPiiRisk(type);
        expect(['high', 'medium', 'low']).toContain(risk);
      }
    });
  });
});

describe('detectEmptyFormat', () => {
  describe('empty_string format', () => {
    it('detects empty_string when only empty strings present', () => {
      const values = ['value1', '', 'value2', '', 'value3'];
      expect(detectEmptyFormat(values)).toBe('empty_string');
    });

    it('returns empty_string when no empty values exist', () => {
      const values = ['value1', 'value2', 'value3'];
      expect(detectEmptyFormat(values)).toBe('empty_string');
    });

    it('returns empty_string for empty array', () => {
      expect(detectEmptyFormat([])).toBe('empty_string');
    });
  });

  describe('null format', () => {
    it('detects null when only "null" strings present', () => {
      const values = ['value1', 'null', 'value2', 'null'];
      expect(detectEmptyFormat(values)).toBe('null');
    });

    it('detects null case-insensitively', () => {
      const values = ['value1', 'NULL', 'value2', 'Null', 'value3'];
      expect(detectEmptyFormat(values)).toBe('null');
    });
  });

  describe('mixed format', () => {
    it('detects mixed when both empty strings and null present', () => {
      const values = ['value1', '', 'value2', 'null'];
      expect(detectEmptyFormat(values)).toBe('mixed');
    });

    it('detects mixed with various null casings', () => {
      const values = ['', 'NULL', 'value'];
      expect(detectEmptyFormat(values)).toBe('mixed');
    });

    it('detects mixed immediately when both found', () => {
      const values = ['', 'null', 'value1', '', 'null'];
      expect(detectEmptyFormat(values)).toBe('mixed');
    });
  });

  describe('edge cases', () => {
    it('handles all empty strings', () => {
      const values = ['', '', ''];
      expect(detectEmptyFormat(values)).toBe('empty_string');
    });

    it('handles all null strings', () => {
      const values = ['null', 'NULL', 'Null'];
      expect(detectEmptyFormat(values)).toBe('null');
    });

    it('does not treat "null" as empty string', () => {
      const values = ['null'];
      expect(detectEmptyFormat(values)).toBe('null');
    });

    it('handles whitespace-only values as non-empty', () => {
      const values = ['  ', '\t', '\n'];
      expect(detectEmptyFormat(values)).toBe('empty_string');
    });
  });
});
