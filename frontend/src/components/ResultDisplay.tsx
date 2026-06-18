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
          <p className="muted-text text-sm">
            Direct identifiers: {result.privacyReport.directIdentifiers}; Quasi-identifiers:{' '}
            {result.privacyReport.quasiIdentifiers}; Pseudonymized: {result.privacyReport.pseudonymizedColumns};
            Masked: {result.privacyReport.maskedColumns}; Generalized:{' '}
            {result.privacyReport.generalizedColumns}; Pass-through/no-op:{' '}
            {result.privacyReport.passThroughColumns}
          </p>
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
