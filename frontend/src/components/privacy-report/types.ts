import type { GlossaryKey } from '../../glossary'
import type { ReleaseEvidenceStatus, ReleaseReadinessStatus } from '../../types'

export type ReportStatus = ReleaseEvidenceStatus | ReleaseReadinessStatus

export type PrivacyMetric = {
  label: string
  value: string | number
  glossaryTerm?: GlossaryKey
  detail?: string | null
  status?: ReportStatus
}
