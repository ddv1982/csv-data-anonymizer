import { CheckCircle2, FolderOpen, RefreshCcw } from 'lucide-react'
import { openOutputLocation } from '../tauri'
import type { AnonymizeData } from '../types'
import { messageFrom } from '../utils/errors'
import { formatResultStats } from '../utils/format'
import { Alert } from './Alert'
import { PrivacyReportSummary } from './PrivacyReportSummary'

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
        <h2>Output created</h2>
        <div className="result-description">
          <p>Selected data was transformed in the protected CSV.</p>
          <p className="mono muted-text result-path">{result.outputPath}</p>
          <p className="muted-text text-sm">{formatResultStats(result)}</p>
        </div>
      </Alert>

      <PrivacyReportSummary privacyReport={result.privacyReport} />

      <div className="result-actions">
        <button type="button" className="button button-outline" onClick={() => void handleOpenFolder()}>
          <FolderOpen aria-hidden="true" />
          Open Folder
        </button>
        <button type="button" className="button button-primary" onClick={onReset}>
          <RefreshCcw aria-hidden="true" />
          Transform Another File
        </button>
      </div>
    </div>
  )
}
