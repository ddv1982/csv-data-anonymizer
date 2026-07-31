import { describe, expect, it } from 'vitest'
import { detectorSourceLabel, detectorSourceSummary, privacyFindingKindLabel } from './detectorEvidence'

describe('detector evidence labels', () => {
  it('uses specific labels for tax validators and country-specific patterns', () => {
    expect(detectorSourceLabel('validator:vat')).toBe('VAT validator')
    expect(detectorSourceLabel('validator:tax-id:us')).toBe('US tax ID validator')
    expect(detectorSourceLabel('pattern:tax-id:nl-btw-tax-number')).toBe('Dutch BTW pattern')
    expect(detectorSourceLabel('pattern:tax-id')).toBe('Tax ID pattern')
    expect(detectorSourceLabel('local-ner:presidio')).toBe('Local language model')
  })

  // Every detector identifier the Rust pipeline emits, so a rename on that side
  // surfaces here instead of quietly degrading to a generic "Detector" label.
  it.each([
    ['detector:column-type', 'Column type'],
    ['header:taxonomy:account-identifier', 'Header taxonomy'],
    ['header:taxonomy-fuzzy:phone', 'Header taxonomy'],
    ['pattern:bearer-token', 'Value pattern'],
    ['pattern:date', 'Value pattern'],
    ['pattern:ip', 'Value pattern'],
    ['pattern:mac', 'Value pattern'],
    ['pattern:phone', 'Value pattern'],
    ['pattern:phone-digits', 'Value pattern'],
    ['pattern:private-key', 'Value pattern'],
    ['pattern:secret-assignment', 'Value pattern'],
    ['pattern:tax-id:nl-btw-tax-number', 'Dutch BTW pattern'],
    ['pattern:uuid', 'Value pattern'],
    ['validator:card', 'Payment card validator'],
    ['validator:email', 'Email validator'],
    ['validator:iban', 'IBAN validator'],
    ['validator:idsmith:NL', 'Idsmith:NL validator'],
    ['validator:phone', 'Phone validator'],
    ['validator:tax-id:us', 'US tax ID validator'],
    ['validator:url', 'Url validator'],
    ['validator:vat', 'VAT validator'],
  ])('labels the Rust detector %s', (detector, expected) => {
    expect(detectorSourceLabel(detector)).toBe(expected)
  })

  // Every finding kind the Rust side can emit needs a label, or the switch falls
  // through and the UI renders nothing for it.
  it.each([
    ['person', 'Person'],
    ['contact', 'Contact'],
    ['privateAddress', 'Address'],
    ['addressRegion', 'Address region'],
    ['privateDate', 'Private date'],
    ['accountOrFinancialId', 'Account ID'],
    ['recordIdentifier', 'Record ID'],
    ['governmentId', 'Government ID'],
    ['credentialOrSecret', 'Secret'],
    ['networkOrDeviceId', 'Network/device ID'],
    ['url', 'URL'],
    ['mixedSensitiveText', 'Mixed sensitive text'],
  ] as const)('labels the %s finding kind', (kind, expected) => {
    expect(privacyFindingKindLabel(kind)).toBe(expected)
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
