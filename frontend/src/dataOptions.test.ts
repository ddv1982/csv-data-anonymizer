import { describe, expect, it } from 'vitest'
import { csvStrategies, directInputStrategies, quickGenerateStrategies, strategyLabel } from './dataOptions'

describe('strategy options', () => {
  it('offers redaction for column workflows without adding it to quick generation', () => {
    expect(csvStrategies).toContain('redact')
    expect(directInputStrategies).toContain('redact')
    expect(quickGenerateStrategies).not.toContain('redact')
    expect(strategyLabel('redact')).toBe('Redact')
  })
})
