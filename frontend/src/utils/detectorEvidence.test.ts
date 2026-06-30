import { describe, expect, it } from 'vitest'
import { detectorSourceLabel, detectorSourceSummary } from './detectorEvidence'

describe('detector evidence labels', () => {
  it('uses specific labels for tax validators and country-specific patterns', () => {
    expect(detectorSourceLabel('validator:vat')).toBe('VAT validator')
    expect(detectorSourceLabel('validator:tax-id:us')).toBe('US tax ID validator')
    expect(detectorSourceLabel('pattern:tax-id:nl-btw-tax-number')).toBe('Dutch BTW pattern')
    expect(detectorSourceLabel('pattern:tax-id')).toBe('Tax ID pattern')
  })

  it('summarizes multiple validator sources without duplicate labels', () => {
    expect(
      detectorSourceSummary({
        detector: 'validator:vat',
        detectors: ['validator:vat', 'validator:tax-id:us'],
      }),
    ).toBe('VAT validator + US tax ID validator')
  })
})
