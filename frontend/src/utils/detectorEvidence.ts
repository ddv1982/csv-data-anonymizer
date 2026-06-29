import type {
  Confidence,
  PrivacyEvidenceSummary,
  PrivacyFinding,
  PrivacyFindingKind,
} from '../types'

export type DetectorStrictness = 'balanced' | 'strict'

export function isDetectorEvidenceVisible(
  item: Pick<PrivacyEvidenceSummary | PrivacyFinding, 'confidence'>,
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

export function visibleFindings(findings: PrivacyFinding[] | undefined, strictness: DetectorStrictness) {
  return (findings ?? []).filter((item) => isDetectorEvidenceVisible(item, strictness))
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

export function placeholderForFinding(finding: Pick<PrivacyFinding, 'kind' | 'dataType'>) {
  if (finding.kind === 'credentialOrSecret') return '[SECRET]'
  if (finding.kind === 'person') return '[PERSON]'
  if (finding.kind === 'contact' && finding.dataType === 'email') return '[EMAIL]'
  if (finding.kind === 'contact' && finding.dataType === 'phone') return '[PHONE]'
  if (finding.kind === 'privateAddress') return '[ADDRESS]'
  if (finding.kind === 'privateDate') return '[DATE]'
  if (finding.kind === 'accountOrFinancialId') return '[ACCOUNT_ID]'
  if (finding.kind === 'governmentId') return '[GOVERNMENT_ID]'
  if (finding.kind === 'networkOrDeviceId') return '[DEVICE_ID]'
  if (finding.kind === 'url') return '[URL]'
  return '[SENSITIVE]'
}
