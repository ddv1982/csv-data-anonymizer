import { AlertTriangle, ShieldCheck } from 'lucide-react'
import type { ColumnMetadata, PrivacyConfig, ReleaseMode } from '../types'
import {
  dpBudgetProjection,
  formatBudgetNumber,
  releaseModeGlossaryTerm,
  releaseModeHelp,
  releaseModeLabel,
  releaseModes,
} from '../utils/privacyDisplay'
import { Alert } from './Alert'
import { GlossaryLabel } from './GlossaryPopover'
import { DifferentialPrivacySettings } from './privacy-settings/DifferentialPrivacySettings'
import { FormalPrivacySettings } from './privacy-settings/FormalPrivacySettings'
import { SyntheticDataSettings } from './privacy-settings/SyntheticDataSettings'
import { SectionHelp } from './SectionHelp'

export function PrivacySettingsPanel({
  config,
  columns,
  disabled,
  onResetBudget,
  onChange,
}: {
  config: PrivacyConfig
  columns: ColumnMetadata[]
  disabled: boolean
  onResetBudget: () => void
  onChange: (config: PrivacyConfig) => void
}) {
  function update(next: Partial<PrivacyConfig>) {
    onChange({ ...config, ...next })
  }

  function updateFormal(next: Partial<PrivacyConfig['formal']>) {
    update({ formal: { ...config.formal, ...next } })
  }

  function updateDp(next: Partial<PrivacyConfig['differentialPrivacy']>) {
    update({ differentialPrivacy: { ...config.differentialPrivacy, ...next } })
  }

  function updateSynthetic(next: Partial<PrivacyConfig['synthetic']>) {
    update({ synthetic: { ...config.synthetic, ...next } })
  }

  const budgetProjection = dpBudgetProjection(config)

  return (
    <div className="privacy-config-panel">
      <div className="privacy-config-header">
        <div className="panel-title-row">
          <span className="privacy-config-title">
            <ShieldCheck aria-hidden="true" />
            <span>Privacy Release</span>
          </span>
          <SectionHelp topic="privacyRelease" label="How privacy release works" />
        </div>
        <select
          value={config.releaseMode}
          disabled={disabled}
          aria-label="Privacy release mode"
          onChange={(event) => update({ releaseMode: event.target.value as ReleaseMode })}
        >
          {releaseModes.map((mode) => (
            <option key={mode} value={mode}>
              {releaseModeLabel(mode)}
            </option>
          ))}
        </select>
      </div>
      <p className="privacy-mode-help muted-text text-sm">
        <GlossaryLabel term={releaseModeGlossaryTerm(config.releaseMode)}>
          {releaseModeLabel(config.releaseMode)}
        </GlossaryLabel>
        <span>{releaseModeHelp(config.releaseMode)}</span>
      </p>

      {config.releaseMode === 'formalTabular' ? (
        <FormalPrivacySettings config={config.formal} disabled={disabled} onChange={updateFormal} />
      ) : null}

      {config.releaseMode === 'differentialPrivacyAggregate' ? (
        <DifferentialPrivacySettings
          config={config.differentialPrivacy}
          columns={columns}
          disabled={disabled}
          onResetBudget={onResetBudget}
          onChange={updateDp}
        />
      ) : null}

      {budgetProjection && budgetProjection.overLimit ? (
        <Alert icon={<AlertTriangle aria-hidden="true" />}>
          This release would raise cumulative epsilon to {formatBudgetNumber(budgetProjection.spentAfter)}, above the
          configured budget limit of {formatBudgetNumber(budgetProjection.limit)}.{' '}
          {config.differentialPrivacy.budget.action === 'warn'
            ? 'It will be allowed because the budget action is Warn only.'
            : 'The local DP budget will block it unless the limit or epsilon changes.'}
        </Alert>
      ) : null}

      {config.releaseMode === 'syntheticData' ? (
        <SyntheticDataSettings config={config.synthetic} disabled={disabled} onChange={updateSynthetic} />
      ) : null}
    </div>
  )
}
