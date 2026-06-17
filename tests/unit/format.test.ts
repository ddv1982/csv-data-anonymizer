/**
 * Unit Tests for CLI Output Formatting
 */

import { describe, it, expect } from 'vitest';
import {
  formatPiiRisk,
  formatDataType,
  formatColumnName,
  formatValue,
  formatError,
  formatSuccess,
  formatWarning,
  formatInfo,
  padString,
  formatColumnLine,
  formatColumnTable,
  formatPreviewTransform,
  formatColumnPreview,
  drawDivider,
  drawBox,
  formatFileSize,
  formatDuration,
  formatRowCount,
} from '../../src/cli/output/format.js';
import type { ColumnMetadata } from '../../src/types/column.js';

describe('formatPiiRisk', () => {
  it('should format high risk in red', () => {
    const result = formatPiiRisk('high');
    expect(result).toContain('HIGH');
  });

  it('should format medium risk in yellow', () => {
    const result = formatPiiRisk('medium');
    expect(result).toContain('MEDIUM');
  });

  it('should format low risk in green', () => {
    const result = formatPiiRisk('low');
    expect(result).toContain('LOW');
  });
});

describe('formatDataType', () => {
  it('should format single word types', () => {
    const result = formatDataType('email');
    expect(result).toContain('Email');
  });

  it('should format snake_case types', () => {
    const result = formatDataType('numeric_id');
    expect(result).toContain('Numeric');
    expect(result).toContain('Id');
  });
});

describe('formatColumnName', () => {
  it('should format column name with bold', () => {
    const result = formatColumnName('test_column');
    expect(result).toContain('test_column');
  });
});

describe('formatValue', () => {
  it('should format normal values', () => {
    const result = formatValue('test value');
    expect(result).toBe('test value');
  });

  it('should format empty values', () => {
    const result = formatValue('');
    expect(result).toContain('empty');
  });

  it('should format null values', () => {
    const result = formatValue('null');
    expect(result).toContain('empty');
  });

  it('should truncate long values', () => {
    const longValue = 'a'.repeat(50);
    const result = formatValue(longValue, 40);
    expect(result.length).toBeLessThan(50);
    expect(result).toContain('...');
  });

  it('should not truncate short values', () => {
    const shortValue = 'short';
    const result = formatValue(shortValue, 40);
    expect(result).toBe(shortValue);
  });
});

describe('formatError', () => {
  it('should format error message with cross mark', () => {
    const result = formatError('Something went wrong');
    expect(result).toContain('✖');
    expect(result).toContain('Something went wrong');
  });
});

describe('formatSuccess', () => {
  it('should format success message with check mark', () => {
    const result = formatSuccess('Operation completed');
    expect(result).toContain('✔');
    expect(result).toContain('Operation completed');
  });
});

describe('formatWarning', () => {
  it('should format warning message with warning sign', () => {
    const result = formatWarning('Be careful');
    expect(result).toContain('⚠');
    expect(result).toContain('Be careful');
  });
});

describe('formatInfo', () => {
  it('should format info message with info sign', () => {
    const result = formatInfo('FYI');
    expect(result).toContain('ℹ');
    expect(result).toContain('FYI');
  });
});

describe('padString', () => {
  it('should pad string to specified width (left align)', () => {
    const result = padString('test', 10, 'left');
    expect(result).toBe('test      ');
  });

  it('should pad string to specified width (right align)', () => {
    const result = padString('test', 10, 'right');
    expect(result).toBe('      test');
  });

  it('should handle string longer than width', () => {
    const result = padString('testing', 5, 'left');
    expect(result).toBe('testing');
  });

  it('should default to left alignment', () => {
    const result = padString('test', 10);
    expect(result).toBe('test      ');
  });
});

describe('formatColumnLine', () => {
  it('should format column metadata as a line', () => {
    const column: ColumnMetadata = {
      name: 'email',
      index: 0,
      detectedType: 'email',
      confidence: 'high',
      piiRisk: 'high',
      emptyFormat: 'empty_string',
      sampleValues: ['test@example.com'],
      selected: true,
    };

    const result = formatColumnLine(column, 1);
    expect(result).toContain('[1]');
    expect(result).toContain('email');
    expect(result).toContain('PII Risk');
    expect(result).toContain('HIGH');
  });
});

describe('formatColumnTable', () => {
  it('should format array of columns as a table', () => {
    const columns: ColumnMetadata[] = [
      {
        name: 'email',
        index: 0,
        detectedType: 'email',
        confidence: 'high',
        piiRisk: 'high',
        emptyFormat: 'empty_string',
        sampleValues: ['test@example.com'],
        selected: true,
      },
      {
        name: 'id',
        index: 1,
        detectedType: 'numeric_id',
        confidence: 'high',
        piiRisk: 'medium',
        emptyFormat: 'empty_string',
        sampleValues: ['12345'],
        selected: false,
      },
    ];

    const result = formatColumnTable(columns);
    expect(result).toContain('Detected columns');
    expect(result).toContain('[1]');
    expect(result).toContain('[2]');
    expect(result).toContain('email');
    expect(result).toContain('id');
  });
});

describe('formatPreviewTransform', () => {
  it('should format original → anonymized transformation', () => {
    const result = formatPreviewTransform('john@example.com', 'jane@example.com');
    expect(result).toContain('Original');
    expect(result).toContain('john@example.com');
    expect(result).toContain('→');
    expect(result).toContain('Anonymized');
    expect(result).toContain('jane@example.com');
  });
});

describe('formatColumnPreview', () => {
  it('should format column preview section', () => {
    const transforms = [
      { original: 'john@example.com', anonymized: 'jane@example.com' },
      { original: 'bob@test.com', anonymized: 'alice@test.com' },
    ];

    const result = formatColumnPreview('email', transforms);
    expect(result).toContain('email');
    expect(result).toContain('john@example.com');
    expect(result).toContain('jane@example.com');
    expect(result).toContain('bob@test.com');
    expect(result).toContain('alice@test.com');
  });
});

describe('drawDivider', () => {
  it('should draw a divider of specified width', () => {
    const result = drawDivider(20);
    // Should contain 20 characters (ignoring ANSI codes)
    expect(result.replace(/\x1B\[[0-9;]*[mK]/g, '').length).toBe(20);
  });

  it('should use custom character', () => {
    const result = drawDivider(10, '=');
    expect(result.replace(/\x1B\[[0-9;]*[mK]/g, '')).toBe('==========');
  });

  it('should use default width and character', () => {
    const result = drawDivider();
    expect(result.replace(/\x1B\[[0-9;]*[mK]/g, '').length).toBe(60);
    expect(result.replace(/\x1B\[[0-9;]*[mK]/g, '')).toContain('─');
  });
});

describe('drawBox', () => {
  it('should draw a box around content', () => {
    const result = drawBox('Title', ['Line 1', 'Line 2']);
    expect(result).toContain('┌');
    expect(result).toContain('┐');
    expect(result).toContain('└');
    expect(result).toContain('┘');
    expect(result).toContain('Title');
    expect(result).toContain('Line 1');
    expect(result).toContain('Line 2');
  });
});

describe('formatFileSize', () => {
  it('should format bytes', () => {
    expect(formatFileSize(500)).toBe('500 B');
  });

  it('should format kilobytes', () => {
    expect(formatFileSize(1024)).toBe('1.0 KB');
    expect(formatFileSize(2048)).toBe('2.0 KB');
  });

  it('should format megabytes', () => {
    expect(formatFileSize(1024 * 1024)).toBe('1.0 MB');
    expect(formatFileSize(5 * 1024 * 1024)).toBe('5.0 MB');
  });

  it('should format gigabytes', () => {
    expect(formatFileSize(1024 * 1024 * 1024)).toBe('1.0 GB');
  });
});

describe('formatDuration', () => {
  it('should format milliseconds', () => {
    expect(formatDuration(500)).toBe('500ms');
  });

  it('should format seconds', () => {
    expect(formatDuration(2500)).toBe('2.5s');
  });

  it('should format minutes and seconds', () => {
    expect(formatDuration(125000)).toBe('2m 5s');
  });
});

describe('formatRowCount', () => {
  it('should format row count with comma separators', () => {
    expect(formatRowCount(1000)).toMatch(/1[,.]?000/);
    expect(formatRowCount(1000000)).toMatch(/1[,.]?000[,.]?000/);
  });

  it('should handle small numbers', () => {
    expect(formatRowCount(5)).toBe('5');
    expect(formatRowCount(100)).toBe('100');
  });
});
