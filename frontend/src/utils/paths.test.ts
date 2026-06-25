import { describe, expect, it } from 'vitest'
import { defaultOutputPathWithSuffix, directoryOf } from './paths'

describe('paths', () => {
  it('extracts parent directories from unix and windows paths', () => {
    expect(directoryOf('/tmp/input.csv')).toBe('/tmp')
    expect(directoryOf('C:\\tmp\\input.csv')).toBe('C:\\tmp')
    expect(directoryOf('input.csv')).toBeNull()
  })

  it('builds default output paths with the configured suffix', () => {
    expect(defaultOutputPathWithSuffix('/tmp/input.csv', '_safe')).toBe('/tmp/input_safe.csv')
    expect(defaultOutputPathWithSuffix('C:\\tmp\\input.csv', '_safe')).toBe('C:\\tmp\\input_safe.csv')
    expect(defaultOutputPathWithSuffix('/tmp/input', '  ')).toBe('/tmp/input_private_output')
  })
})
