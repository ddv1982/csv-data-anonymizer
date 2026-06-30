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
    case 'privateDate':
      return 'Private date'
    case 'accountOrFinancialId':
      return 'Account ID'
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
