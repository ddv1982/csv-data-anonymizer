import type {
  Confidence,
  PrivacyEvidenceSummary,
  PrivacyFindingKind,
} from '../types'

export type DetectorStrictness = 'balanced' | 'strict'

export function isDetectorEvidenceVisible(
  item: Pick<PrivacyEvidenceSummary, 'confidence'>,
  strictness: DetectorStrictness,
) {
  return strictness === 'strict' || item.confidence !== 'low'
}

export function visibleEvidence(
  evidence: PrivacyEvidenceSummary[] | undefined,
  strictness: DetectorStrictness,
) {
  return (evidence ?? []).filter((item) => isDetectorEvidenceVisible(item, strictness))
}

export function privacyFindingKindLabel(kind: PrivacyFindingKind) {
  switch (kind) {
    case 'person':
      return 'Person'
    case 'contact':
      return 'Contact'
    case 'privateAddress':
      return 'Address'
    case 'addressRegion':
      return 'Address region'
    case 'privateDate':
      return 'Private date'
    case 'accountOrFinancialId':
      return 'Account ID'
    case 'recordIdentifier':
      return 'Record ID'
    case 'governmentId':
      return 'Government ID'
    case 'credentialOrSecret':
      return 'Secret'
    case 'networkOrDeviceId':
      return 'Network/device ID'
    case 'url':
      return 'URL'
    case 'mixedSensitiveText':
      return 'Mixed sensitive text'
  }
}

export function detectorConfidenceLabel(confidence: Confidence) {
  return confidence === 'high' ? 'High' : confidence === 'medium' ? 'Medium' : 'Low'
}

/**
 * Human label for a detector identifier emitted by the Rust detection pipeline.
 *
 * The exact-match arms below have to correspond to labels Rust actually
 * produces; `detectorEvidence.test.ts` pins the current set so a label renamed
 * on the Rust side shows up as a failing test rather than as a generic
 * "Detector" in the UI. The `startsWith` arms are deliberate fallbacks that also
 * cover identifiers Rust does not emit yet (`header:` for a future non-taxonomy
 * header rule, `pattern:tax-id` for a second country-specific tax pattern).
 */
export function detectorSourceLabel(detector: string | undefined) {
  if (!detector) return 'Detector'
  if (detector.startsWith('header:taxonomy')) return 'Header taxonomy'
  if (detector.startsWith('header:')) return 'Header rule'
  if (detector.startsWith('local-ner:')) return 'Local language model'
  if (detector === 'validator:iban') return 'IBAN validator'
  if (detector === 'validator:phone') return 'Phone validator'
  if (detector === 'validator:card') return 'Payment card validator'
  if (detector.startsWith('validator:vat')) return 'VAT validator'
  if (detector === 'validator:tax-id:us') return 'US tax ID validator'
  if (detector === 'pattern:tax-id:nl-btw-tax-number') return 'Dutch BTW pattern'
  if (detector.startsWith('pattern:tax-id')) return 'Tax ID pattern'
  if (detector.startsWith('validator:')) {
    const validator = detector.slice('validator:'.length).replaceAll('-', ' ')
    return `${validator.charAt(0).toUpperCase()}${validator.slice(1)} validator`
  }
  if (detector.startsWith('pattern:')) return 'Value pattern'
  if (detector.startsWith('detector:column-type')) return 'Column type'
  return 'Detector'
}

export function detectorSourceSummary(item: Pick<PrivacyEvidenceSummary, 'detector' | 'detectors'>) {
  const detectors = item.detectors?.length ? item.detectors : item.detector ? [item.detector] : []
  const labels = [...new Set(detectors.map((detector) => detectorSourceLabel(detector)))]

  if (labels.length === 0) return 'Detector'
  if (labels.length === 1) return labels[0]
  if (labels.length === 2) return `${labels[0]} + ${labels[1]}`
  return `${labels[0]} + ${labels.length - 1} more`
}
