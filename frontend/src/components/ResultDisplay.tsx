import { CheckCircle2, FolderOpen, RefreshCcw } from 'lucide-react'
import { openOutputLocation } from '../tauri'
import type { AnonymizeData } from '../types'
import { messageFrom } from '../utils/errors'
import { formatResultStats } from '../utils/format'
import { Alert } from './Alert'

export function ResultDisplay({
  result,
  onReset,
  onError,
}: {
  result: AnonymizeData
  onReset: () => void
  onError: (message: string) => void
}) {
  const privacyMetrics = [
    ['Direct identifiers', result.privacyReport.directIdentifiers],
    ['Quasi-identifiers', result.privacyReport.quasiIdentifiers],
    ['Pseudonymized columns', result.privacyReport.pseudonymizedColumns],
    ['Smart replacement columns', result.privacyReport.smartReplacementColumns],
    ['Opaque token columns', result.privacyReport.opaqueTokenColumns],
    ['Masked columns', result.privacyReport.maskedColumns],
    ['Generalized columns', result.privacyReport.generalizedColumns],
    ['Pass-through/no-op', result.privacyReport.passThroughColumns],
    ['Unique pseudonyms', result.privacyReport.uniquePseudonymValues],
    ['Opaque token values', result.privacyReport.opaqueTokenValues],
    ['Repeated source reuses', result.privacyReport.reusedPseudonymValues],
    ['Collisions avoided', result.privacyReport.collisionsAvoided],
    ['Pool exhaustions', result.privacyReport.exhaustedPseudonymPools],
    ['Smart replacement values', result.privacyReport.smartReplacementValues],
    ['Smart fallbacks', result.privacyReport.smartReplacementFallbacks],
  ] as const

  async function handleOpenFolder() {
    try {
      await openOutputLocation(result.outputPath)
    } catch (caught) {
      onError(messageFrom(caught))
    }
  }

  return (
    <div className="result-stack">
      <Alert variant="success" icon={<CheckCircle2 aria-hidden="true" />}>
        <h2>Success!</h2>
        <div className="result-description">
          <p>Your file has been successfully anonymized.</p>
          <p className="mono muted-text result-path">{result.outputPath}</p>
          <p className="muted-text text-sm">{formatResultStats(result)}</p>
        </div>
      </Alert>

      <div className="preview-group">
        <h3>Privacy Report</h3>
        <div className="preview-frame">
          <div className="privacy-metrics">
            {privacyMetrics.map(([label, value]) => (
              <div className="privacy-metric" key={label}>
                <span className="muted-text text-sm">{label}</span>
                <strong>{value.toLocaleString()}</strong>
              </div>
            ))}
          </div>
          {result.privacyReport.notes.map((note) => (
            <p className="muted-text text-sm" key={note}>
              {note}
            </p>
          ))}
        </div>
      </div>

      <div className="result-actions">
        <button type="button" className="button button-outline" onClick={() => void handleOpenFolder()}>
          <FolderOpen aria-hidden="true" />
          Open Folder
        </button>
        <button type="button" className="button button-primary" onClick={onReset}>
          <RefreshCcw aria-hidden="true" />
          Anonymize Another File
        </button>
      </div>
    </div>
  )
}
