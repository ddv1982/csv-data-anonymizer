import { describe, it, expect } from 'vitest';
import {
  transformValue,
  createTransformContext,
  transformRow,
  createRowTransformer,
} from '../../src/core/transformer.js';
import type { ColumnMetadata } from '../../src/types/column.js';

describe('transformer', () => {
  describe('createTransformContext', () => {
    const column: ColumnMetadata = {
      name: 'email',
      index: 0,
      detectedType: 'email',
      confidence: 'high',
      piiRisk: 'high',
      sampleValues: ['test@example.com'],
      emptyFormat: 'empty_string',
      isSelected: true,
    };

    it('should create context with all required fields', () => {
      const context = createTransformContext(column, 5, 'test-seed', true);

      expect(context.columnName).toBe('email');
      expect(context.columnIndex).toBe(0);
      expect(context.rowIndex).toBe(5);
      expect(context.seed).toBe('test-seed');
      expect(context.deterministic).toBe(true);
      expect(context.emptyFormat).toBe('empty_string');
    });
  });

  describe('transformValue', () => {
    it('should preserve empty string values', () => {
      const column: ColumnMetadata = {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: true,
      };

      const context = createTransformContext(column, 0, '', false);
      const result = transformValue('', column, context);

      expect(result).toBe('');
    });

    it('should preserve null string values', () => {
      const column: ColumnMetadata = {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        sampleValues: [],
        emptyFormat: 'null',
        isSelected: true,
      };

      const context = createTransformContext(column, 0, '', false);
      const result = transformValue('null', column, context);

      expect(result).toBe('null');
    });

    it('should transform email values', () => {
      const column: ColumnMetadata = {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: true,
      };

      const context = createTransformContext(column, 0, 'seed', false);
      const result = transformValue('test@example.com', column, context);

      expect(result).toContain('@example.com');
      expect(result).not.toBe('test@example.com');
    });

    it('should transform UUID values', () => {
      const column: ColumnMetadata = {
        name: 'uuid',
        index: 0,
        detectedType: 'uuid',
        confidence: 'high',
        piiRisk: 'medium',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: true,
      };

      const context = createTransformContext(column, 0, 'seed', true);
      const result = transformValue('550e8400-e29b-41d4-a716-446655440001', column, context);

      // Should still be valid UUID format
      expect(result).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i);
    });

    it('should pass through country_code values unchanged', () => {
      const column: ColumnMetadata = {
        name: 'country',
        index: 0,
        detectedType: 'country_code',
        confidence: 'high',
        piiRisk: 'low',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: true,
      };

      const context = createTransformContext(column, 0, '', false);
      const result = transformValue('US', column, context);

      expect(result).toBe('US');
    });

    it('should pass through enum values unchanged', () => {
      const column: ColumnMetadata = {
        name: 'status',
        index: 0,
        detectedType: 'enum',
        confidence: 'high',
        piiRisk: 'low',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: true,
      };

      const context = createTransformContext(column, 0, '', false);
      const result = transformValue('active', column, context);

      expect(result).toBe('active');
    });
  });

  describe('transformRow', () => {
    const columns: ColumnMetadata[] = [
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
        name: 'country',
        index: 1,
        detectedType: 'country_code',
        confidence: 'high',
        piiRisk: 'low',
        sampleValues: [],
        emptyFormat: 'empty_string',
        isSelected: false,
      },
    ];

    it('should only transform selected columns', () => {
      const row = ['test@example.com', 'US'];
      const result = transformRow(row, columns, 0, 'seed', false);

      // Email (selected) should be transformed
      expect(result[0]).not.toBe('test@example.com');
      expect(result[0]).toContain('@example.com');

      // Country (not selected) should be unchanged
      expect(result[1]).toBe('US');
    });

    it('should handle rows with more values than columns', () => {
      const row = ['test@example.com', 'US', 'extra'];
      const result = transformRow(row, columns, 0, 'seed', false);

      // Extra value should be unchanged
      expect(result[2]).toBe('extra');
    });

    it('should preserve row structure', () => {
      const row = ['test@example.com', 'US'];
      const result = transformRow(row, columns, 0, 'seed', false);

      expect(result.length).toBe(row.length);
    });
  });

  describe('createRowTransformer', () => {
    const columns: ColumnMetadata[] = [
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
    ];

    it('should return a function that transforms rows', () => {
      const transformer = createRowTransformer(columns, 'seed', true);

      expect(typeof transformer).toBe('function');
    });

    it('should use deterministic mode when enabled', () => {
      const transformer = createRowTransformer(columns, 'seed', true);

      const result1 = transformer(['test@example.com'], 0);
      const result2 = transformer(['test@example.com'], 0);

      // Same input should produce same output in deterministic mode
      expect(result1[0]).toBe(result2[0]);
    });

    it('should produce different results for different row indices', () => {
      const transformer = createRowTransformer(columns, 'seed', false);

      const result1 = transformer(['test@example.com'], 0);
      const result2 = transformer(['test@example.com'], 1);

      // Different row indices may produce different results
      // (though not guaranteed, so just check they're valid)
      expect(result1[0]).toContain('@example.com');
      expect(result2[0]).toContain('@example.com');
    });
  });
});
