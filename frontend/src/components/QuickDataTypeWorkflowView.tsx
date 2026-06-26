import { AlertCircle, Check, Clipboard, Loader2, Wand2 } from 'lucide-react'
import { useState } from 'react'
import { dataTypes, quickGenerateStrategies, strategyLabel } from '../dataOptions'
import { generateQuickValues } from '../tauri'
import type { AnonymizationStrategy, AppSettings, DataType, QuickTransformData } from '../types'
import type { LocalAiState } from '../hooks/useLocalAi'
import { copyTextToClipboard } from '../utils/clipboard'
import { messageFrom } from '../utils/errors'
import { formatToken } from '../utils/format'
import { Alert } from './Alert'
import { Card } from './Card'
import { LocalAiSettingsBlock } from './LocalAiSettingsBlock'

type QuickBusyState = 'idle' | 'generating' | 'copying'
const MIN_COUNT = 1
const MAX_COUNT = 1000

export function QuickDataTypeWorkflowView({
  settings,
  localAi,
  onUpdateSetting,
  onError,
}: {
  settings: AppSettings
  localAi: LocalAiState
  onUpdateSetting: <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => void
  onError: (message: string | null) => void
}) {
  const [dataType, setDataType] = useState<DataType>('email')
  const [strategy, setStrategy] = useState<AnonymizationStrategy>('auto')
  const [count, setCount] = useState(1)
  const [result, setResult] = useState<QuickTransformData | null>(null)
  const [busy, setBusy] = useState<QuickBusyState>('idle')
  const [copyStatus, setCopyStatus] = useState<string | null>(null)

  const isBusy = busy !== 'idle'
  const usesLocalAi = strategy === 'localAi'
  const localAiBlocked = usesLocalAi && (!localAi.ready || localAi.downloadRunning)
  const canGenerate = count >= MIN_COUNT && count <= MAX_COUNT && !isBusy && !localAiBlocked

  async function handleGenerate() {
    if (count < MIN_COUNT || count > MAX_COUNT || isBusy) return
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
        settings.deterministicDefault,
        settings.seed,
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
    if (!result?.output || isBusy) return
    onError(null)
    setBusy('copying')
    try {
      await copyTextToClipboard(result.output)
      setCopyStatus('Copied')
    } catch (caught) {
      onError(messageFrom(caught))
    } finally {
      setBusy('idle')
    }
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

          {usesLocalAi ? (
            <div className="quick-local-ai">
              <LocalAiSettingsBlock
                settings={settings}
                localAi={localAi}
                disabled={isBusy}
                onUpdateSetting={onUpdateSetting}
              />
              {localAiBlocked ? (
                <Alert icon={<AlertCircle aria-hidden="true" />}>
                  Set up Local AI before generating Smart replacement values.
                </Alert>
              ) : null}
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
    </div>
  )
}
