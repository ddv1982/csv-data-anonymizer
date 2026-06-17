import { describe, it, expect } from 'vitest';
import {
  buildColumnMetadata,
  applyColumnSelection,
  getSelectedColumns,
  getHighRiskColumns,
  autoSelectPiiColumns,
} from '../../src/core/metadataBuilder.js';
import type { ColumnMetadata } from '../../src/types/column.js';

describe('metadataBuilder', () => {
  describe('buildColumnMetadata', () => {
    it('should build metadata for all columns', () => {
      const headers = ['email', 'id', 'country'];
      const samples = [
        ['john@example.com', '1001', 'US'],
        ['jane@test.org', '1002', 'GB'],
        ['bob@company.io', '1003', 'CA'],
      ];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata.length).toBe(3);
      expect(metadata[0].name).toBe('email');
      expect(metadata[1].name).toBe('id');
      expect(metadata[2].name).toBe('country');
    });

    it('should detect email type', () => {
      const headers = ['email'];
      const samples = [
        ['john@example.com'],
        ['jane@test.org'],
        ['bob@company.io'],
      ];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata[0].detectedType).toBe('email');
      expect(metadata[0].piiRisk).toBe('high');
    });

    it('should detect UUID type', () => {
      const headers = ['uuid'];
      const samples = [
        ['550e8400-e29b-41d4-a716-446655440001'],
        ['550e8400-e29b-41d4-a716-446655440002'],
        ['550e8400-e29b-41d4-a716-446655440003'],
      ];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata[0].detectedType).toBe('uuid');
      expect(metadata[0].piiRisk).toBe('medium');
    });

    it('should detect numeric ID type', () => {
      const headers = ['id'];
      const samples = [
        ['1001'],
        ['1002'],
        ['1003'],
      ];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata[0].detectedType).toBe('numeric_id');
      expect(metadata[0].piiRisk).toBe('medium');
    });

    it('should detect timestamp type', () => {
      const headers = ['created_at'];
      const samples = [
        ['2024-01-15 10:30:00'],
        ['2024-02-20 14:45:30'],
        ['2024-03-10 09:00:00'],
      ];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata[0].detectedType).toBe('timestamp');
      expect(metadata[0].piiRisk).toBe('low');
    });

    it('should include correct column indices', () => {
      const headers = ['a', 'b', 'c'];
      const samples = [['1', '2', '3']];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata[0].index).toBe(0);
      expect(metadata[1].index).toBe(1);
      expect(metadata[2].index).toBe(2);
    });

    it('should include sample values', () => {
      const headers = ['email'];
      const samples = [
        ['john@example.com'],
        ['jane@test.org'],
        ['bob@company.io'],
      ];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata[0].sampleValues).toContain('john@example.com');
      expect(metadata[0].sampleValues.length).toBeLessThanOrEqual(5);
    });

    it('should detect empty format correctly', () => {
      const headers = ['col1', 'col2', 'col3'];
      const samples = [
        ['value1', '', 'null'],
        ['value2', '', 'value'],
        ['', '', 'null'],
      ];

      const metadata = buildColumnMetadata(headers, samples);

      // col1 has empty strings
      expect(metadata[0].emptyFormat).toBe('empty_string');
      // col2 has only empty strings
      expect(metadata[1].emptyFormat).toBe('empty_string');
      // col3 has 'null' strings
      expect(metadata[2].emptyFormat).toBe('null');
    });

    it('should default isSelected to false', () => {
      const headers = ['email'];
      const samples = [['test@example.com']];

      const metadata = buildColumnMetadata(headers, samples);

      expect(metadata[0].isSelected).toBe(false);
    });
  });

  describe('applyColumnSelection', () => {
    const metadata: ColumnMetadata[] = [
      {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        sampleValues: ['test@example.com'],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
      {
        name: 'id',
        index: 1,
        detectedType: 'numeric_id',
        confidence: 'high',
        piiRisk: 'medium',
        sampleValues: ['1001'],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
      {
        name: 'status',
        index: 2,
        detectedType: 'enum',
        confidence: 'high',
        piiRisk: 'low',
        sampleValues: ['active'],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
    ];

    it('should set isSelected for selected indices', () => {
      const result = applyColumnSelection(metadata, [0, 2]);

      expect(result[0].isSelected).toBe(true);
      expect(result[1].isSelected).toBe(false);
      expect(result[2].isSelected).toBe(true);
    });

    it('should not modify original metadata', () => {
      applyColumnSelection(metadata, [0]);

      expect(metadata[0].isSelected).toBe(false);
    });

    it('should handle empty selection', () => {
      const result = applyColumnSelection(metadata, []);

      result.forEach(col => {
        expect(col.isSelected).toBe(false);
      });
    });
  });

  describe('getSelectedColumns', () => {
    const metadata: ColumnMetadata[] = [
      {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: true,
      },
      {
        name: 'id',
        index: 1,
        detectedType: 'numeric_id',
        confidence: 'high',
        piiRisk: 'medium',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
    ];

    it('should return only selected columns', () => {
      const selected = getSelectedColumns(metadata);

      expect(selected.length).toBe(1);
      expect(selected[0].name).toBe('email');
    });
  });

  describe('getHighRiskColumns', () => {
    const metadata: ColumnMetadata[] = [
      {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
      {
        name: 'id',
        index: 1,
        detectedType: 'numeric_id',
        confidence: 'high',
        piiRisk: 'medium',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
    ];

    it('should return only high risk columns', () => {
      const highRisk = getHighRiskColumns(metadata);

      expect(highRisk.length).toBe(1);
      expect(highRisk[0].name).toBe('email');
    });
  });

  describe('autoSelectPiiColumns', () => {
    const metadata: ColumnMetadata[] = [
      {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
      {
        name: 'id',
        index: 1,
        detectedType: 'numeric_id',
        confidence: 'high',
        piiRisk: 'medium',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
      {
        name: 'status',
        index: 2,
        detectedType: 'enum',
        confidence: 'high',
        piiRisk: 'low',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
    ];

    it('should select high and medium risk columns', () => {
      const result = autoSelectPiiColumns(metadata);

      expect(result[0].isSelected).toBe(true); // high risk
      expect(result[1].isSelected).toBe(true); // medium risk
      expect(result[2].isSelected).toBe(false); // low risk
    });
  });
});
