import { describe, expect, it } from 'vitest'
import { columnStrategies, quickGenerateStrategies, strategyLabel } from './dataOptions'

describe('strategy options', () => {
  it('offers redaction for column workflows without adding it to quick generation', () => {
    expect(columnStrategies).toContain('redact')
    expect(quickGenerateStrategies).not.toContain('redact')
    expect(strategyLabel('redact')).toBe('Redact')
  })
})
