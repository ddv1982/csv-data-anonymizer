import { useState } from 'react'
import type { PrivacyReport } from '../../types'
import { copyTextToClipboard } from '../../utils/clipboard'
import { ReportDisclosure } from './ReportDisclosure'

type ReleaseType = 'unassessed' | 'internal' | 'namedRecipient' | 'definedGroup' | 'public'
type ReleaseContext = {
  releaseType: ReleaseType
  auxiliaryData: boolean
  knownPeople: boolean
  relatedReleases: boolean
  accessControls: boolean
}

const EMPTY_CONTEXT: ReleaseContext = {
  releaseType: 'unassessed',
  auxiliaryData: false,
  knownPeople: false,
  relatedReleases: false,
  accessControls: false,
}

export function ReleaseContextReview({ privacyReport }: { privacyReport: PrivacyReport }) {
  const [review, setReview] = useState(() => ({
    privacyReport,
    context: EMPTY_CONTEXT,
    copyStatus: null as string | null,
  }))
  // A prop change immediately presents a blank review without needing an effect
  // or one render in which the previous artifact's answers are visible.
  const isCurrentReport = review.privacyReport === privacyReport
  const context = isCurrentReport ? review.context : EMPTY_CONTEXT
  const copyStatus = isCurrentReport ? review.copyStatus : null
  const assessed = context.releaseType !== 'unassessed'

  function update(patch: Partial<ReleaseContext>) {
    setReview({
      privacyReport,
      context: { ...context, ...patch },
      copyStatus: null,
    })
  }

  async function copyReport() {
    try {
      await copyTextToClipboard(JSON.stringify({ schemaVersion: 1, privacyReport, releaseContext: context }, null, 2))
      setReview({
        privacyReport,
        context,
        copyStatus: 'Report and release context copied as JSON.',
      })
    } catch {
      setReview({
        privacyReport,
        context,
        copyStatus: 'Could not copy the report. Try again or use the system clipboard controls.',
      })
    }
  }

  return (
    <ReportDisclosure title="Release Context Review" countLabel={assessed ? 'Human review started' : 'Context not assessed'}>
      <p className="muted-text text-sm">
        These answers do not certify anonymity or legal compliance. They help a human reviewer consider information
        that cannot be inferred from the output file itself.
      </p>
      <div className="settings-stack">
        <label>
          Intended release
          <select value={context.releaseType} onChange={(event) => update({ releaseType: event.target.value as ReleaseType })}>
            <option value="unassessed">Not assessed</option>
            <option value="internal">Internal use</option>
            <option value="namedRecipient">Named recipient</option>
            <option value="definedGroup">Defined group</option>
            <option value="public">Public release</option>
          </select>
        </label>
        <ContextCheck checked={context.auxiliaryData} onChange={(value) => update({ auxiliaryData: value })} label="Recipients may hold related or auxiliary data" />
        <ContextCheck checked={context.knownPeople} onChange={(value) => update({ knownPeople: value })} label="Recipients may know specific people are included" />
        <ContextCheck checked={context.relatedReleases} onChange={(value) => update({ relatedReleases: value })} label="Earlier or related releases exist" />
        <ContextCheck checked={context.accessControls} onChange={(value) => update({ accessControls: value })} label="Access or contractual controls are in place" />
      </div>
      {context.releaseType === 'public' || context.auxiliaryData || context.knownPeople || context.relatedReleases ? (
        <p className="danger-text text-sm">
          Linkage or singling-out risk needs explicit human review before sharing this output.
        </p>
      ) : assessed ? (
        <p className="muted-text text-sm">
          Human release decision still required; technical checks and this checklist are supporting evidence only.
        </p>
      ) : null}
      <button type="button" className="button button-outline button-sm" onClick={() => void copyReport()}>
        Copy report with context
      </button>
      {copyStatus ? (
        <p className={copyStatus.startsWith('Could not') ? 'danger-text text-sm' : 'muted-text text-sm'} role="status">
          {copyStatus}
        </p>
      ) : null}
    </ReportDisclosure>
  )
}

function ContextCheck({
  checked,
  onChange,
  label,
}: {
  checked: boolean
  onChange: (checked: boolean) => void
  label: string
}) {
  return (
    <label className="switch-row">
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
      <span>{label}</span>
    </label>
  )
}
