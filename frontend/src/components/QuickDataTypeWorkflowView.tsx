import { AlertCircle, Check, Clipboard, Loader2, Wand2 } from 'lucide-react'
import { useState } from 'react'
import { dataTypes, quickGenerateStrategies, strategyLabel } from '../dataOptions'
import { generateQuickValues } from '../tauri'
import { useCopyOutput } from '../hooks/useCopyOutput'
import type { AnonymizationStrategy, DataType, QuickTransformData } from '../types'
import type { LocalAiState } from '../hooks/useLocalAi'
import { messageFrom } from '../utils/errors'
import { formatToken } from '../utils/format'
import { Alert } from './Alert'
import { Card } from './Card'
import { PrivacyReportSummary } from './ResultDisplay'

type QuickBusyState = 'idle' | 'generating' | 'copying'
const MIN_COUNT = 1
const MAX_COUNT = 1000

export function QuickDataTypeWorkflowView({
  settingsLoaded,
  localAi,
  onOpenLocalAiSettings,
  onError,
}: {
  settingsLoaded: boolean
  localAi: LocalAiState
  onOpenLocalAiSettings: () => void
  onError: (message: string | null) => void
}) {
  const [dataType, setDataType] = useState<DataType>('email')
  const [strategy, setStrategy] = useState<AnonymizationStrategy>('auto')
  const [count, setCount] = useState(1)
  const [result, setResult] = useState<QuickTransformData | null>(null)
  const [busy, setBusy] = useState<QuickBusyState>('idle')

  const isBusy = busy !== 'idle'
  const { copyOutput, copyStatus, setCopyStatus } = useCopyOutput({ isBusy, onError, setBusy })
  const usesLocalAi = strategy === 'localAi'
  const localAiBlocked = usesLocalAi && (!localAi.ready || localAi.downloadRunning)
  const canGenerate = settingsLoaded && count >= MIN_COUNT && count <= MAX_COUNT && !isBusy && !localAiBlocked

  async function handleGenerate() {
    if (!settingsLoaded || count < MIN_COUNT || count > MAX_COUNT || isBusy) return
    if (localAiBlocked) {
      onError('Set up Local AI before generating Smart replacement values.')
      return
    }
    onError(null)
    setBusy('generating')
    setCopyStatus(null)
    try {
      const generated = await generateQuickValues(
        dataType,
        strategy,
        count,
        localAi.request,
      )
      setResult(generated)
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
  }

  async function handleCopy() {
    await copyOutput(result?.output)
  }

  return (
    <div className="workflow-stack">
      <Card title="Quick by Data Type">
        <div className="quick-grid">
          <div className="field">
            <label htmlFor="quick-data-type">Data Type</label>
            <select
              id="quick-data-type"
              value={dataType}
              disabled={isBusy}
              onChange={(event) => {
                setDataType(event.target.value as DataType)
                setResult(null)
                setCopyStatus(null)
              }}
            >
              {dataTypes.map((type) => (
                <option key={type} value={type}>
                  {formatToken(type)}
                </option>
              ))}
            </select>
          </div>

          <div className="field">
            <label htmlFor="quick-strategy">Strategy</label>
            <select
              id="quick-strategy"
              value={strategy}
              disabled={isBusy}
              onChange={(event) => {
                setStrategy(event.target.value as AnonymizationStrategy)
                setResult(null)
                setCopyStatus(null)
              }}
            >
              {quickGenerateStrategies.map((strategyOption) => (
                <option key={strategyOption} value={strategyOption}>
                  {strategyLabel(strategyOption)}
                </option>
              ))}
            </select>
          </div>

          <div className="field">
            <label htmlFor="quick-count">Quantity</label>
            <input
              id="quick-count"
              type="number"
              min={MIN_COUNT}
              max={MAX_COUNT}
              step={1}
              value={count}
              disabled={isBusy}
              onChange={(event) => {
                const nextCount = Number.parseInt(event.target.value, 10)
                setCount(Number.isNaN(nextCount) ? 0 : nextCount)
                setResult(null)
                setCopyStatus(null)
              }}
            />
            <span className="muted-text text-sm">Generate 1 to {MAX_COUNT.toLocaleString()} values.</span>
          </div>

          {usesLocalAi && localAiBlocked ? (
            <div className="quick-local-ai">
              <Alert icon={<AlertCircle aria-hidden="true" />}>
                <div className="alert-line">
                  <span>Set up Local AI before generating Smart replacement values.</span>
                  <button type="button" className="button button-outline button-sm" onClick={onOpenLocalAiSettings}>
                    Open Local AI settings
                  </button>
                </div>
              </Alert>
            </div>
          ) : null}

          <button type="button" className="button button-primary button-lg full-width" disabled={!canGenerate} onClick={handleGenerate}>
            {busy === 'generating' ? <Loader2 className="spin" aria-hidden="true" /> : <Wand2 aria-hidden="true" />}
            Generate values
          </button>
        </div>
      </Card>

      {result ? (
        <Card
          title="Generated Values"
          action={
            <button type="button" className="button button-outline button-sm" disabled={isBusy} onClick={handleCopy}>
              {busy === 'copying' ? <Loader2 className="spin" aria-hidden="true" /> : <Clipboard aria-hidden="true" />}
              Copy
            </button>
          }
        >
          <div className="direct-output-stack">
            <textarea className="direct-output" value={result.output} readOnly aria-label="Generated values" />
            <div className="direct-output-meta" aria-live="polite">
              <span className="muted-text text-sm">{result.rowCount.toLocaleString()} values generated</span>
              {copyStatus ? (
                <span className="status-pill success">
                  <Check aria-hidden="true" />
                  {copyStatus}
                </span>
              ) : null}
            </div>
          </div>
        </Card>
      ) : null}

      {result ? <PrivacyReportSummary privacyReport={result.privacyReport} /> : null}
    </div>
  )
}
