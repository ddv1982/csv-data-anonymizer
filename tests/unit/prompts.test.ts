/**
 * Unit Tests for CLI Prompts
 */

import { describe, it, expect } from 'vitest';
import {
  getSuggestedColumns,
  validateSelection,
  formatSelectionSummary,
} from '../../src/cli/prompts/columnSelect.js';
import {
  generateColumnPreview,
  generatePreview,
  formatPreviewRow,
} from '../../src/cli/prompts/preview.js';
import type { ColumnMetadata } from '../../src/types/column.js';

const createMockColumn = (overrides: Partial<ColumnMetadata> = {}): ColumnMetadata => ({
  name: 'test',
  index: 0,
  detectedType: 'string',
  confidence: 'medium',
  piiRisk: 'low',
  emptyFormat: 'empty_string',
  sampleValues: ['value1', 'value2'],
  selected: false,
  ...overrides,
});

describe('getSuggestedColumns', () => {
  it('should return indices of high PII risk columns', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, piiRisk: 'low' }),
      createMockColumn({ index: 1, piiRisk: 'high' }),
      createMockColumn({ index: 2, piiRisk: 'low' }),
    ];

    const result = getSuggestedColumns(columns);
    expect(result).toEqual([1]);
  });

  it('should return indices of medium PII risk columns', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, piiRisk: 'low' }),
      createMockColumn({ index: 1, piiRisk: 'medium' }),
      createMockColumn({ index: 2, piiRisk: 'low' }),
    ];

    const result = getSuggestedColumns(columns);
    expect(result).toEqual([1]);
  });

  it('should return both high and medium PII risk columns', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, piiRisk: 'high' }),
      createMockColumn({ index: 1, piiRisk: 'medium' }),
      createMockColumn({ index: 2, piiRisk: 'low' }),
    ];

    const result = getSuggestedColumns(columns);
    expect(result).toEqual([0, 1]);
  });

  it('should return empty array when no high/medium risk columns', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, piiRisk: 'low' }),
      createMockColumn({ index: 1, piiRisk: 'low' }),
    ];

    const result = getSuggestedColumns(columns);
    expect(result).toEqual([]);
  });
});

describe('validateSelection', () => {
  it('should return valid indices', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0 }),
      createMockColumn({ index: 1 }),
      createMockColumn({ index: 2 }),
    ];

    const result = validateSelection([0, 1, 2], columns);
    expect(result).toEqual([0, 1, 2]);
  });

  it('should filter out invalid indices', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0 }),
      createMockColumn({ index: 1 }),
    ];

    const result = validateSelection([0, 1, 5, 10], columns);
    expect(result).toEqual([0, 1]);
  });

  it('should filter out negative indices', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0 }),
      createMockColumn({ index: 1 }),
    ];

    const result = validateSelection([-1, 0, 1], columns);
    expect(result).toEqual([0, 1]);
  });

  it('should return empty array for all invalid indices', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0 }),
    ];

    const result = validateSelection([5, 10, -1], columns);
    expect(result).toEqual([]);
  });
});

describe('formatSelectionSummary', () => {
  it('should format summary for selected columns', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, name: 'email' }),
      createMockColumn({ index: 1, name: 'phone' }),
      createMockColumn({ index: 2, name: 'id' }),
    ];

    const result = formatSelectionSummary(columns, [0, 2]);
    expect(result).toContain('2');
    expect(result).toContain('email');
    expect(result).toContain('id');
    expect(result).not.toContain('phone');
  });

  it('should handle empty selection', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, name: 'email' }),
    ];

    const result = formatSelectionSummary(columns, []);
    expect(result).toContain('No columns selected');
  });

  it('should handle single column selection', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, name: 'email' }),
      createMockColumn({ index: 1, name: 'phone' }),
    ];

    const result = formatSelectionSummary(columns, [0]);
    expect(result).toContain('1');
    expect(result).toContain('email');
  });
});

describe('generateColumnPreview', () => {
  it('should generate preview transformations', () => {
    const column = createMockColumn({
      name: 'test',
      detectedType: 'string',
      selected: true,
    });

    const sampleValues = ['value1', 'value2', 'value3'];
    const result = generateColumnPreview(column, sampleValues);

    expect(result.length).toBe(3);
    result.forEach((row, index) => {
      expect(row.original).toBe(sampleValues[index]);
      expect(row.anonymized).toBeDefined();
    });
  });

  it('should handle empty values', () => {
    const column = createMockColumn({
      name: 'test',
      detectedType: 'string',
      selected: true,
    });

    const sampleValues = ['', 'value', ''];
    const result = generateColumnPreview(column, sampleValues);

    expect(result.length).toBe(3);
    expect(result[0].original).toBe('');
    expect(result[0].anonymized).toBe(''); // Empty values preserved
  });

  it('should work with deterministic mode', () => {
    const column = createMockColumn({
      name: 'email',
      detectedType: 'email',
      selected: true,
    });

    const sampleValues = ['test@example.com'];
    const result1 = generateColumnPreview(column, sampleValues, 'seed123', true);
    const result2 = generateColumnPreview(column, sampleValues, 'seed123', true);

    expect(result1[0].anonymized).toBe(result2[0].anonymized);
  });
});

describe('generatePreview', () => {
  it('should generate preview for selected columns', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, name: 'id', detectedType: 'numeric_id' }),
      createMockColumn({ index: 1, name: 'email', detectedType: 'email' }),
      createMockColumn({ index: 2, name: 'country', detectedType: 'country_code' }),
    ];

    const sampleRows = [
      ['1001', 'test@example.com', 'US'],
      ['1002', 'user@test.com', 'GB'],
    ];

    const result = generatePreview(columns, [0, 1], sampleRows);

    expect(result.size).toBe(2);
    expect(result.has('id')).toBe(true);
    expect(result.has('email')).toBe(true);
    expect(result.has('country')).toBe(false);
  });

  it('should limit preview rows', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, name: 'email', detectedType: 'email' }),
    ];

    const sampleRows = [
      ['email1@test.com'],
      ['email2@test.com'],
      ['email3@test.com'],
      ['email4@test.com'],
      ['email5@test.com'],
      ['email6@test.com'],
      ['email7@test.com'],
      ['email8@test.com'],
      ['email9@test.com'],
      ['email10@test.com'],
    ];

    const result = generatePreview(columns, [0], sampleRows, { rowCount: 3 });
    const emailPreview = result.get('email');

    expect(emailPreview?.length).toBe(3);
  });

  it('should handle empty selected indices', () => {
    const columns: ColumnMetadata[] = [
      createMockColumn({ index: 0, name: 'email' }),
    ];

    const sampleRows = [['test@example.com']];

    const result = generatePreview(columns, [], sampleRows);
    expect(result.size).toBe(0);
  });
});

describe('formatPreviewRow', () => {
  it('should format preview row with arrow', () => {
    const row = {
      original: 'john@example.com',
      anonymized: 'jane@example.com',
    };

    const result = formatPreviewRow(row);
    expect(result).toContain('Original');
    expect(result).toContain('john@example.com');
    expect(result).toContain('→');
    expect(result).toContain('Anonymized');
    expect(result).toContain('jane@example.com');
  });
});
