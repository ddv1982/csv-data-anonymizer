import { Loader2, Wand2 } from 'lucide-react'
import { dataTypes, quickGenerateStrategies, strategyLabel } from '../dataOptions'
import {
  QUICK_MAX_COUNT,
  QUICK_MIN_COUNT,
  type QuickGenerateWorkflowState,
} from '../hooks/useQuickGenerateWorkflow'
import type { AnonymizationStrategy, DataType } from '../types'
import { formatToken } from '../utils/format'
import { Card } from './Card'
import { CopyableOutputCard } from './CopyableOutputCard'
import { LocalAiBlockedAlert } from './LocalAiBlockedAlert'
import { PrivacyReportSummary } from './PrivacyReportSummary'

export function QuickDataTypeWorkflowView({
  workflow,
  onOpenLocalAiSettings,
}: {
  workflow: QuickGenerateWorkflowState
  onOpenLocalAiSettings: () => void
}) {
  const { busy, count, dataType, isBusy, result, strategy } = workflow

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
                workflow.setDataType(event.target.value as DataType)
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
                workflow.setStrategy(event.target.value as AnonymizationStrategy)
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
              min={QUICK_MIN_COUNT}
              max={QUICK_MAX_COUNT}
              step={1}
              value={count}
              disabled={isBusy}
              onChange={(event) => {
                const nextCount = Number.parseInt(event.target.value, 10)
                workflow.setCount(Number.isNaN(nextCount) ? 0 : nextCount)
              }}
            />
            <span className="muted-text text-sm">Generate 1 to {QUICK_MAX_COUNT.toLocaleString()} values.</span>
          </div>

          {workflow.usesLocalAi && workflow.localAiBlocked ? (
            <div className="quick-local-ai">
              <LocalAiBlockedAlert
                message="Set up Local AI before generating Smart replacement values."
                onOpenSettings={onOpenLocalAiSettings}
              />
            </div>
          ) : null}

          <button
            type="button"
            className="button button-primary button-lg full-width"
            disabled={!workflow.canGenerate}
            onClick={workflow.generate}
          >
            {busy === 'generating' ? <Loader2 className="spin" aria-hidden="true" /> : <Wand2 aria-hidden="true" />}
            Generate values
          </button>
        </div>
      </Card>

      {result ? (
        <CopyableOutputCard
          title="Generated Values"
          outputLabel="Generated values"
          output={result.output}
          stats={`${result.rowCount.toLocaleString()} values generated`}
          copying={busy === 'copying'}
          disabled={isBusy}
          copyStatus={workflow.copyStatus}
          onCopy={workflow.copyOutput}
        />
      ) : null}

      {result ? <PrivacyReportSummary privacyReport={result.privacyReport} /> : null}
    </div>
  )
}
