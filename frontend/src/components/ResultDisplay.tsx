import { CheckCircle2, FolderOpen, RefreshCcw } from 'lucide-react'
import { openOutputLocation } from '../tauri'
import type { AnonymizeData } from '../types'
import { messageFrom } from '../utils/errors'
import { formatResultStats, formatToken } from '../utils/format'
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
    ['Release mode', releaseModeLabel(result.privacyReport.releaseMode)],
    ['Direct identifiers', result.privacyReport.directIdentifiers],
    ['Quasi-identifiers', result.privacyReport.quasiIdentifiers],
    ['Sensitive columns', result.privacyReport.sensitiveColumns],
    ['Pseudonymized columns', result.privacyReport.pseudonymizedColumns],
    ['Smart replacement columns', result.privacyReport.smartReplacementColumns],
    ['Opaque token columns', result.privacyReport.opaqueTokenColumns],
    ['Masked columns', result.privacyReport.maskedColumns],
    ['Generalized columns', result.privacyReport.generalizedColumns],
    ['Pass-through/no-op', result.privacyReport.passThroughColumns],
    ['Suppressed rows', result.privacyReport.suppressedRows],
    ['Synthetic rows', result.privacyReport.syntheticRows],
    ['DP epsilon', result.privacyReport.dpEpsilon ?? 'n/a'],
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
                <strong>{typeof value === 'number' ? value.toLocaleString() : value}</strong>
              </div>
            ))}
          </div>
          {result.privacyReport.formalModels.length > 0 ? (
            <div className="privacy-models">
              {result.privacyReport.formalModels.map((model) => (
                <div className="privacy-model-row" key={model.model}>
                  <span>
                    <strong>{formatToken(model.model)}</strong>
                    <span className="muted-text text-sm">
                      {model.actual} / {model.threshold}
                    </span>
                  </span>
                  <span className={model.satisfied ? 'status-pill success' : 'status-pill'}>
                    {model.satisfied ? 'Satisfied' : 'Review'}
                  </span>
                  <p className="muted-text text-sm">{model.message}</p>
                </div>
              ))}
            </div>
          ) : null}
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

function releaseModeLabel(mode: string) {
  if (mode === 'formalTabular') return 'k/l/t tabular'
  if (mode === 'differentialPrivacyAggregate') return 'DP aggregate'
  if (mode === 'syntheticData') return 'Synthetic data'
  return 'Standard masking'
}
