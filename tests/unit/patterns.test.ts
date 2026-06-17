import { describe, it, expect } from 'vitest';
import {
  EMAIL_PATTERN,
  UUID_PATTERN,
  TIMESTAMP_PATTERN,
  NUMERIC_ID_PATTERN,
  COUNTRY_CODE_PATTERN,
  PHONE_PATTERN,
} from '../../src/utils/patterns.js';

describe('Detection Patterns', () => {
  describe('EMAIL_PATTERN', () => {
    it('matches valid email addresses', () => {
      const validEmails = [
        'user@example.com',
        'user.name@example.com',
        'user+tag@example.com',
        'user123@example.co.uk',
        'first.last@subdomain.example.org',
        'test%email@domain.io',
        'a@b.co',
      ];
      for (const email of validEmails) {
        expect(EMAIL_PATTERN.test(email), `Expected "${email}" to match`).toBe(true);
      }
    });

    it('rejects invalid email addresses', () => {
      const invalidEmails = [
        'not-an-email',
        '@example.com',
        'user@',
        'user@.com',
        'user@example',
        'user @example.com',
        'user@example .com',
        '',
        'user@exam ple.com',
        'user@@example.com',
      ];
      for (const email of invalidEmails) {
        expect(EMAIL_PATTERN.test(email), `Expected "${email}" to not match`).toBe(false);
      }
    });
  });

  describe('UUID_PATTERN', () => {
    it('matches valid UUID v4 format', () => {
      const validUuids = [
        '550e8400-e29b-41d4-a716-446655440000',
        'A550E840-E29B-41D4-A716-446655440000',
        '123e4567-e89b-12d3-a456-426614174000',
        'ffffffff-ffff-ffff-ffff-ffffffffffff',
        '00000000-0000-0000-0000-000000000000',
      ];
      for (const uuid of validUuids) {
        expect(UUID_PATTERN.test(uuid), `Expected "${uuid}" to match`).toBe(true);
      }
    });

    it('rejects invalid UUIDs', () => {
      const invalidUuids = [
        'not-a-uuid',
        '550e8400-e29b-41d4-a716',
        '550e8400e29b41d4a716446655440000',
        '550e8400-e29b-41d4-a716-44665544000',
        '550e8400-e29b-41d4-a716-4466554400000',
        'g50e8400-e29b-41d4-a716-446655440000',
        '',
        '550e8400-e29b-41d4-a716-44665544000g',
      ];
      for (const uuid of invalidUuids) {
        expect(UUID_PATTERN.test(uuid), `Expected "${uuid}" to not match`).toBe(false);
      }
    });
  });

  describe('TIMESTAMP_PATTERN', () => {
    it('matches valid timestamps', () => {
      const validTimestamps = [
        '2024-01-15',
        '2024-12-31',
        '2024-01-15T10:30:00',
        '2024-01-15 10:30:00',
        '2024-01-15T10:30:00.123',
        '2024-01-15T10:30:00.123456',
        '2024-01-15 10:30:00.123456789',
      ];
      for (const ts of validTimestamps) {
        expect(TIMESTAMP_PATTERN.test(ts), `Expected "${ts}" to match`).toBe(true);
      }
    });

    it('rejects invalid timestamps', () => {
      const invalidTimestamps = [
        '01-15-2024',
        '2024/01/15',
        '2024-1-15',
        '24-01-15',
        '2024-01-15T10:30',
        '10:30:00',
        'not-a-timestamp',
        '',
        '2024-01-15T',
        '2024-01-15 10:30',
      ];
      for (const ts of invalidTimestamps) {
        expect(TIMESTAMP_PATTERN.test(ts), `Expected "${ts}" to not match`).toBe(false);
      }
    });
  });

  describe('NUMERIC_ID_PATTERN', () => {
    it('matches valid numeric IDs (4+ digits)', () => {
      const validIds = [
        '1234',
        '12345',
        '1234567890',
        '0000',
        '9999999999999999',
      ];
      for (const id of validIds) {
        expect(NUMERIC_ID_PATTERN.test(id), `Expected "${id}" to match`).toBe(true);
      }
    });

    it('rejects invalid numeric IDs', () => {
      const invalidIds = [
        '123',
        '12',
        '1',
        '',
        'abc',
        '123a',
        'a123',
        '12.34',
        '-1234',
        '+1234',
        '1234 ',
        ' 1234',
      ];
      for (const id of invalidIds) {
        expect(NUMERIC_ID_PATTERN.test(id), `Expected "${id}" to not match`).toBe(false);
      }
    });
  });

  describe('COUNTRY_CODE_PATTERN', () => {
    it('matches valid ISO 3166-1 alpha-2 country codes', () => {
      const validCodes = [
        'US',
        'GB',
        'DE',
        'FR',
        'JP',
        'AU',
        'CA',
        'NZ',
        'ZZ',
      ];
      for (const code of validCodes) {
        expect(COUNTRY_CODE_PATTERN.test(code), `Expected "${code}" to match`).toBe(true);
      }
    });

    it('rejects invalid country codes', () => {
      const invalidCodes = [
        'us',
        'Us',
        'uS',
        'USA',
        'U',
        '',
        '12',
        'U1',
        ' US',
        'US ',
      ];
      for (const code of invalidCodes) {
        expect(COUNTRY_CODE_PATTERN.test(code), `Expected "${code}" to not match`).toBe(false);
      }
    });
  });

  describe('PHONE_PATTERN', () => {
    it('matches valid phone numbers', () => {
      const validPhones = [
        '+1234567890',
        '+1 234 567 8900',
        '+1 (234) 567-8900',
        '1234567890',
        '123-456-7890',
        '(123) 456-7890',
        '+44 20 7123 4567',
        '+49 (30) 1234567',
        '1 800 555 0123',
      ];
      for (const phone of validPhones) {
        expect(PHONE_PATTERN.test(phone), `Expected "${phone}" to match`).toBe(true);
      }
    });

    it('rejects invalid phone numbers', () => {
      const invalidPhones = [
        '123456789',
        '12345678',
        'abc',
        '',
        '+',
        '123',
        'phone-number',
        '+abc1234567',
      ];
      for (const phone of invalidPhones) {
        expect(PHONE_PATTERN.test(phone), `Expected "${phone}" to not match`).toBe(false);
      }
    });
  });
});
