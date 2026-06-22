import { CheckCircle2, FolderOpen, RefreshCcw } from 'lucide-react'
import type { GlossaryKey } from '../glossary'
import { openOutputLocation } from '../tauri'
import type { AnonymizeData, PrivacyModel } from '../types'
import { messageFrom } from '../utils/errors'
import { formatResultStats, formatToken } from '../utils/format'
import { releaseModeGlossaryTerm, releaseModeLabel } from '../utils/privacyDisplay'
import { Alert } from './Alert'
import { GlossaryLabel } from './GlossaryPopover'
import { SectionHelp } from './SectionHelp'

export function ResultDisplay({
  result,
  onReset,
  onError,
}: {
  result: AnonymizeData
  onReset: () => void
  onError: (message: string) => void
}) {
  const privacyMetrics: Array<{ label: string; value: string | number; glossaryTerm: GlossaryKey }> = [
    {
      label: 'Release mode',
      value: releaseModeLabel(result.privacyReport.releaseMode),
      glossaryTerm: releaseModeGlossaryTerm(result.privacyReport.releaseMode),
    },
    { label: 'Direct identifiers', value: result.privacyReport.directIdentifiers, glossaryTerm: 'directIdentifier' },
    { label: 'Quasi-identifiers', value: result.privacyReport.quasiIdentifiers, glossaryTerm: 'quasiIdentifier' },
    { label: 'Sensitive columns', value: result.privacyReport.sensitiveColumns, glossaryTerm: 'sensitive' },
    { label: 'Pseudonymized columns', value: result.privacyReport.pseudonymizedColumns, glossaryTerm: 'pseudonymizedColumns' },
    {
      label: 'Smart replacement columns',
      value: result.privacyReport.smartReplacementColumns,
      glossaryTerm: 'smartReplacementColumns',
    },
    { label: 'Opaque token columns', value: result.privacyReport.opaqueTokenColumns, glossaryTerm: 'opaqueTokenColumns' },
    { label: 'Masked columns', value: result.privacyReport.maskedColumns, glossaryTerm: 'maskedColumns' },
    { label: 'Generalized columns', value: result.privacyReport.generalizedColumns, glossaryTerm: 'generalizedColumns' },
    { label: 'Pass-through/no-op', value: result.privacyReport.passThroughColumns, glossaryTerm: 'passThroughNoOp' },
    { label: 'Suppressed rows', value: result.privacyReport.suppressedRows, glossaryTerm: 'suppressedRows' },
    { label: 'Synthetic rows', value: result.privacyReport.syntheticRows, glossaryTerm: 'syntheticRows' },
    { label: 'DP epsilon', value: result.privacyReport.dpEpsilon ?? 'n/a', glossaryTerm: 'epsilon' },
    { label: 'Unique pseudonyms', value: result.privacyReport.uniquePseudonymValues, glossaryTerm: 'uniquePseudonyms' },
    { label: 'Opaque token values', value: result.privacyReport.opaqueTokenValues, glossaryTerm: 'opaqueTokenValues' },
    {
      label: 'Repeated source reuses',
      value: result.privacyReport.reusedPseudonymValues,
      glossaryTerm: 'repeatedSourceReuses',
    },
    { label: 'Collisions avoided', value: result.privacyReport.collisionsAvoided, glossaryTerm: 'collisionsAvoided' },
    { label: 'Pool exhaustions', value: result.privacyReport.exhaustedPseudonymPools, glossaryTerm: 'poolExhaustions' },
    {
      label: 'Smart replacement values',
      value: result.privacyReport.smartReplacementValues,
      glossaryTerm: 'smartReplacementValues',
    },
    { label: 'Smart fallbacks', value: result.privacyReport.smartReplacementFallbacks, glossaryTerm: 'smartFallbacks' },
  ]
  if (result.privacyReport.dpBudget) {
    privacyMetrics.splice(
      13,
      0,
      {
        label: 'DP budget status',
        value: budgetStatusLabel(result.privacyReport.dpBudget.status),
        glossaryTerm: 'dpBudgetStatus',
      },
      {
        label: 'DP spent after',
        value: result.privacyReport.dpBudget.spentEpsilonAfter,
        glossaryTerm: 'dpBudgetSpent',
      },
      {
        label: 'DP budget limit',
        value: result.privacyReport.dpBudget.limitEpsilon,
        glossaryTerm: 'dpBudgetLimit',
      },
      {
        label: 'DP remaining',
        value: result.privacyReport.dpBudget.remainingEpsilon,
        glossaryTerm: 'dpBudgetRemaining',
      },
    )
  }

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
          <p>Selected data was transformed or released according to the configured workflow.</p>
          <p className="mono muted-text result-path">{result.outputPath}</p>
          <p className="muted-text text-sm">{formatResultStats(result)}</p>
        </div>
      </Alert>

      <div className="preview-group">
        <div className="section-heading-row">
          <h3>Privacy Report</h3>
          <SectionHelp topic="privacyReport" label="How to read this report" />
        </div>
        <div className="preview-frame">
          <div className="privacy-metrics">
            {privacyMetrics.map(({ label, value, glossaryTerm }) => (
              <div className="privacy-metric" key={label}>
                <span className="muted-text text-sm">
                  <GlossaryLabel term={glossaryTerm}>{label}</GlossaryLabel>
                </span>
                <strong>{typeof value === 'number' ? value.toLocaleString() : value}</strong>
              </div>
            ))}
          </div>
          {result.privacyReport.formalModels.length > 0 ? (
            <div className="privacy-models">
              {result.privacyReport.formalModels.map((model) => (
                <div className="privacy-model-row" key={model.model}>
                  <span>
                    <strong>
                      <GlossaryLabel term={privacyModelGlossaryTerm(model.model)}>{formatToken(model.model)}</GlossaryLabel>
                    </strong>
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
          Transform Another File
        </button>
      </div>
    </div>
  )
}

function privacyModelGlossaryTerm(model: PrivacyModel): GlossaryKey {
  if (model === 'kAnonymity') return 'kAnonymity'
  if (model === 'lDiversity') return 'lDiversity'
  if (model === 'tCloseness') return 'tCloseness'
  if (model === 'differentialPrivacy') return 'epsilon'
  if (model === 'syntheticData') return 'syntheticData'
  return 'formalModel'
}

function budgetStatusLabel(status: string) {
  if (status === 'withinBudget') return 'Within budget'
  if (status === 'atBudget') return 'At budget'
  if (status === 'overBudget') return 'Over budget'
  return formatToken(status)
}
