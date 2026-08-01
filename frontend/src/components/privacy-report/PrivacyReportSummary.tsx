import type { MatchedPart, PrivacyReport, RowUniquenessSummary } from '../../types'
import { SectionHelp } from '../SectionHelp'
import {
  nonZeroMetrics,
  pluralize,
  readinessSummary,
  sensitiveSummary,
  smartRejectionReasonLabel,
  statusLabel,
  statusPillClass,
  transformationSummary,
} from './helpers'
import { PrivacyMetricGrid } from './PrivacyMetricGrid'
import { PrivacyReportColumnDecisions } from './PrivacyReportColumnDecisions'
import { ReportDisclosure } from './ReportDisclosure'
import { ReleaseContextReview } from './ReleaseContextReview'
import type { PrivacyMetric } from './types'

export function PrivacyReportSummary({ privacyReport }: { privacyReport: PrivacyReport }) {
  const transformedColumns =
    privacyReport.pseudonymizedColumns +
    privacyReport.smartReplacementColumns +
    privacyReport.opaqueTokenColumns +
    privacyReport.maskedColumns +
    privacyReport.redactedColumns
  const sensitiveColumnTotal = privacyReport.directIdentifiers + privacyReport.quasiIdentifiers
  const advancedMetrics = nonZeroMetrics([
    { label: 'Direct identifiers', value: privacyReport.directIdentifiers, glossaryTerm: 'directIdentifier' },
    { label: 'Quasi-identifiers', value: privacyReport.quasiIdentifiers, glossaryTerm: 'quasiIdentifier' },
    { label: 'Pseudonymized columns', value: privacyReport.pseudonymizedColumns, glossaryTerm: 'pseudonymizedColumns' },
    { label: 'Opaque token columns', value: privacyReport.opaqueTokenColumns, glossaryTerm: 'opaqueTokenColumns' },
    { label: 'Masked columns', value: privacyReport.maskedColumns, glossaryTerm: 'maskedColumns' },
    { label: 'Redacted columns', value: privacyReport.redactedColumns, glossaryTerm: 'redactedColumns' },
    { label: 'Unique pseudonyms', value: privacyReport.uniquePseudonymValues, glossaryTerm: 'uniquePseudonyms' },
    { label: 'Opaque token values', value: privacyReport.opaqueTokenValues, glossaryTerm: 'opaqueTokenValues' },
    { label: 'Repeatable keyed token values', value: privacyReport.keyedTokenValues },
    {
      label: 'Repeated source reuses',
      value: privacyReport.reusedPseudonymValues,
      glossaryTerm: 'repeatedSourceReuses',
    },
    { label: 'Collisions avoided', value: privacyReport.collisionsAvoided, glossaryTerm: 'collisionsAvoided' },
    { label: 'Pool exhaustions', value: privacyReport.exhaustedPseudonymPools, glossaryTerm: 'poolExhaustions' },
    { label: 'Format fallbacks', value: privacyReport.shapeFallbackValues, glossaryTerm: 'formatFallbacks' },
  ])
  const smartMetrics = nonZeroMetrics([
    {
      label: 'Smart replacement columns',
      value: privacyReport.smartReplacementColumns,
      glossaryTerm: 'smartReplacementColumns',
    },
    {
      label: 'Smart replacement values',
      value: privacyReport.smartReplacementValues,
      glossaryTerm: 'smartReplacementValues',
    },
    {
      label: 'Smart rejections',
      value: privacyReport.smartReplacementRejections,
      glossaryTerm: 'smartRejections',
    },
    { label: 'Smart fallbacks', value: privacyReport.smartReplacementFallbacks, glossaryTerm: 'smartFallbacks' },
  ])
  const hasSmartReplacementActivity =
    smartMetrics.length > 0 || privacyReport.smartReplacementRejectionReasons.length > 0
  const overviewMetrics: PrivacyMetric[] = [
    {
      label: 'Technical checks',
      value: statusLabel(privacyReport.readiness.status),
      detail: readinessSummary(privacyReport),
    },
    {
      label: 'Columns transformed',
      value: transformedColumns,
      detail: transformationSummary(privacyReport),
    },
    {
      label: 'Sensitive columns',
      value: sensitiveColumnTotal,
      detail: sensitiveSummary(privacyReport),
    },
    {
      label: 'Pass-through/no-op',
      value: privacyReport.passThroughColumns,
      glossaryTerm: 'passThroughNoOp',
      detail: 'Left unchanged by the selected strategy.',
    },
  ]

  return (
    <div className="preview-group">
      <div className="section-heading-row">
        <h3>Privacy Report</h3>
        <SectionHelp topic="privacyReport" label="How to read this report" />
      </div>
      <div className="preview-frame privacy-report-frame">
        <p className="muted-text text-sm">
          These checks inspect this output only. They do not assess its recipient, other available data,
          access controls, or related releases. A human release decision is still required.
        </p>
        <PrivacyMetricGrid metrics={overviewMetrics} variant="overview" />
        <ReadinessNotes privacyReport={privacyReport} />
        <ReleaseContextReview privacyReport={privacyReport} />

        {hasSmartReplacementActivity ? (
          <ReportDisclosure title="Smart Replacement" countLabel={pluralize(smartMetrics.length, 'metric')}>
            {smartMetrics.length > 0 ? <PrivacyMetricGrid metrics={smartMetrics} /> : null}
            {privacyReport.smartReplacementRejectionReasons.length > 0 ? (
              <div className="privacy-metrics">
                {privacyReport.smartReplacementRejectionReasons.map((item) => (
                  <div className="privacy-metric" key={item.reason}>
                    <span className="privacy-metric-label muted-text text-sm">
                      {smartRejectionReasonLabel(item.reason)}
                    </span>
                    <strong>{item.count}</strong>
                  </div>
                ))}
              </div>
            ) : null}
          </ReportDisclosure>
        ) : null}

        <JointReIdentifiability privacyReport={privacyReport} />

        {privacyReport.columnReports.length > 0 ? (
          <PrivacyReportColumnDecisions columns={privacyReport.columnReports} />
        ) : null}

        {advancedMetrics.length > 0 ? (
          <ReportDisclosure title="Advanced Counts" countLabel={pluralize(advancedMetrics.length, 'metric')}>
            <PrivacyMetricGrid metrics={advancedMetrics} />
          </ReportDisclosure>
        ) : null}

        {privacyReport.utilityMetrics.length > 0 ? (
          <ReportDisclosure title="Utility" countLabel={pluralize(privacyReport.utilityMetrics.length, 'check')}>
            <PrivacyMetricGrid
              metrics={privacyReport.utilityMetrics.map((metric) => ({
                label: metric.label,
                value: metric.value,
                status: metric.status,
                detail: metric.detail,
              }))}
            />
          </ReportDisclosure>
        ) : null}

        {privacyReport.evidence.length > 0 ? (
          <ReportDisclosure title="Evidence" countLabel={pluralize(privacyReport.evidence.length, 'item')}>
            <div className="privacy-models">
              {privacyReport.evidence.map((item) => (
                <div className="privacy-model-row" key={item.id}>
                  <span>
                    <strong>{item.label}</strong>
                    <span className="muted-text text-sm">{item.detail}</span>
                  </span>
                  <span className={statusPillClass(item.status)}>{statusLabel(item.status)}</span>
                </div>
              ))}
            </div>
          </ReportDisclosure>
        ) : null}

        {privacyReport.notes.length > 0 ? (
          <div className="report-note-list">
            {privacyReport.notes.map((note) => (
              <p className="muted-text text-sm" key={note}>
                {note}
              </p>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  )
}

/**
 * The figures behind the joint re-identifiability finding.
 *
 * The finding itself is worded in Rust, as an evidence row and — when it is a review or a
 * pass — as a readiness item too. This block adds only what prose cannot carry compactly:
 * the group sizes the count came from, and what each column was matched on.
 *
 * No thresholds are evaluated here. Whether a number is good or bad is decided once, in
 * `release_report.rs`, so the table and the wording beside it cannot come to different
 * conclusions about the same file. What each `matchedOn` *means* is likewise decided in
 * `uniqueness.rs`; this file only chooses the phrasing, which is why a new variant of the
 * union shows up here as a missing line rather than as a wrong claim.
 */
function JointReIdentifiability({ privacyReport }: { privacyReport: PrivacyReport }) {
  const summary = privacyReport.rowUniqueness
  // Absent on the paths with no rows to measure — unstructured text, a single pasted
  // value. Rendering nothing is the point: an empty table would read as a measurement.
  if (!summary) return null

  if (summary.measurementIncomplete) {
    return (
      <ReportDisclosure title="Joint re-identifiability" countLabel="not measured">
        <p className="muted-text text-sm">
          This file holds more distinct combinations than the check keeps, so it stopped after{' '}
          {summary.rowsMeasured} rows. The release is not measured, which is not the
          same as measured clean.
        </p>
      </ReportDisclosure>
    )
  }

  // Mirrors `column_name` in `release_report.rs`, including its quoting: the names below are
  // comma-joined, so a column genuinely called `city, state` read as two, and a reader
  // counting names against the "N columns" label found them disagreeing with nothing to say
  // which was wrong. Lower-case fallback for the same reason — one column must not read as
  // `Column 3` here and `column 3` in the finding beside it.
  const nameFor = (index: number) => {
    const found = privacyReport.columnReports.find((column) => column.columnIndex === index)
    const name = found?.columnName?.trim() ? found.columnName : `column ${index}`
    return name.includes(',') || name.includes('"') ? `"${name.replaceAll('"', '\\"')}"` : name
  }
  const namesMatchedOn = (part: MatchedPart) =>
    summary.matchedColumns.filter((matched) => matched.matchedOn === part).map((matched) => nameFor(matched.columnIndex))
  const countedCount = summary.matchedColumns.length

  if (countedCount === 0) {
    return (
      <ReportDisclosure title="Joint re-identifiability" countLabel="not applicable">
        <p className="muted-text text-sm">
          No released column carries anything an outsider could match against data they already
          hold, so there is no combination to measure. That describes how the columns were
          transformed, not a finding that the rows cannot be re-identified.
        </p>
      </ReportDisclosure>
    )
  }

  const metrics: PrivacyMetric[] = [
    {
      label: 'Rows singled out',
      value: summary.uniqueRows,
      detail: `Of ${summary.rowsMeasured} released rows, alone in their combination.`,
    },
    {
      label: 'Distinct combinations',
      value: summary.distinctClasses,
      detail: 'Groups of rows that look identical on the columns below.',
    },
    {
      label: 'Smallest group',
      value: summary.smallestClass,
      detail: 'One record can set this, so read it beside the 5% figure.',
    },
    {
      label: 'Most exposed 5%',
      value: summary.fifthPercentileClassSize,
      detail: 'The group size the most exposed twentieth of rows sit in, or smaller.',
    },
  ]

  // The one figure not taken over the matched subset, so it is labelled as the different
  // question it answers. Absent rather than zero when its own histogram outgrew the check,
  // and omitted here in that case: the alternative was computing it, budgeting memory for it
  // and then rendering it nowhere, which is what it did before.
  if (summary.distinctRowsAllColumns !== null) {
    metrics.push({
      label: 'Distinct released rows',
      value: summary.distinctRowsAllColumns,
      detail: 'Over every column, matchable or not — whether the file could be aggregated at all.',
    })
  }

  return (
    <ReportDisclosure title="Joint re-identifiability" countLabel={pluralize(countedCount, 'column')}>
      <PrivacyMetricGrid metrics={metrics} />
      {/* One self-contained line per kind of match, rather than one list with the partial
          matches folded into it. A pseudonymized email and a shifted date used to be named
          alongside a released postcode, which reads as a claim that all three cells were
          published as they stand — and a reader told their rows are unique "on customer_id"
          would remove the wrong column. Each line also stands alone, so none of them opens on
          a conjunction whose antecedent was not rendered. */}
      <MatchedColumnLine
        names={namesMatchedOn('wholeValue')}
        lead="Counted over"
        note="released as they stand, so anyone holding these fields can match them directly"
      />
      <MatchedColumnLine
        names={namesMatchedOn('emailDomain')}
        lead="Counted by domain only:"
        note="the local part is replaced, the employer is not"
      />
      <MatchedColumnLine
        names={namesMatchedOn('dateDecadeAndTime')}
        lead="Counted by decade and time of day only:"
        note="the date is shifted by up to a year, so an outsider narrows to a window rather than a day, and this group size can move between runs"
      />
      <MatchedColumnLine
        names={namesMatchedOn('survivingFormat')}
        lead="Counted by surviving format only:"
        note="a digit count, a separator layout, a masked value's word and letter counts — each weak alone and counted because this measure is about what they add up to"
      />
      <MatchedColumnLine
        names={namesMatchedOn('blankPattern')}
        lead="Counted by which cells are blank:"
        note="an empty cell is written through untouched whatever the strategy, so the pattern of what was left unanswered survives every transform"
      />
      <PartialMatchLine summary={summary} nameFor={nameFor} />
      <DropColumnAdvice summary={summary} nameFor={nameFor} />
    </ReportDisclosure>
  )
}

/**
 * Names the columns whose line above is true of only some of the rows.
 *
 * The five lines above describe a column by what its strategy and detected type make
 * reproducible, which is fixed per column — no cell value can change it. Cells can still fail
 * to carry it: one that does not fit its column's detected shape is pseudonymized generically
 * and projects to nothing. So "Counted by decade and time of day only: birth_date" was
 * rendered over a column where one value in a hundred parsed.
 *
 * Says the counts already account for it, because otherwise the line reads as doubt about the
 * figures in the grid — and those are right. Mirrors `partial_match_caveat` in
 * `release_report.rs`, which puts the same sentence into the evidence and readiness
 * lists rendered further down this same panel.
 */
function PartialMatchLine({
  summary,
  nameFor,
}: {
  summary: RowUniquenessSummary
  nameFor: (index: number) => string
}) {
  const partial = summary.matchedColumns
    .filter((matched) => !matched.matchedEveryRow)
    .map((matched) => nameFor(matched.columnIndex))
  if (partial.length === 0) return null

  return (
    <p className="muted-text text-sm">
      Only some of the released rows carry what {partial.join(', ')} was matched on: a cell that
      did not fit its column's detected shape was replaced generically, and the counts above
      already treat those as sharing nothing there.
    </p>
  )
}

/**
 * Which single column to change, under the lines that say what was counted.
 *
 * Sits last on purpose: it is the answer to the question the four metrics above provoke, and
 * it is the only line in this panel that asks the reader to do something. Worded to match
 * `drop_column_advice` in `release_report.rs`, which builds the same sentence into the evidence
 * detail and the readiness review list — both rendered further down this same panel, so the two
 * are on screen together. That is why the numbers here are printed raw: grouping them on this
 * side put "instead of 4,123" a few centimetres above "instead of 4123".
 */
function DropColumnAdvice({
  summary,
  nameFor,
}: {
  summary: RowUniquenessSummary
  nameFor: (index: number) => string
}) {
  // Advice about clearing uniqueness, on a file with none to clear. The measure still ran and
  // its numbers are still above; there is just no action to recommend.
  if (summary.uniqueRows === 0) return null

  if (summary.dropAttributionIncomplete) {
    // Stated rather than omitted, because silence here reads as "no column would help" —
    // the opposite finding, and the one that would stop a reader looking further.
    return (
      <p className="muted-text text-sm">
        Which single column carries this was not measured on this file.
      </p>
    )
  }

  const best = summary.dropColumnEffects[0]
  if (!best) return null

  if (best.uniqueRowsWithout >= summary.uniqueRows) {
    return (
      <p className="muted-text text-sm">
        No single column carries it: removing any one of the columns named above would leave the
        same rows unique, so the combination has to be broken in more than one place.
      </p>
    )
  }

  // "Removing from the file", not "dropping", and the difference is not stylistic. The measured
  // counterfactual is the column not being released at all. Unticking it here does the opposite
  // — an unselected column is written through unchanged — so a reader who read "drop" as "untick"
  // would release the raw values. The trailing clause is there because `uniqueRowsWithout` counts
  // only rows standing alone: it can reach zero while the remaining groups are still pairs, which
  // this same report calls "not anonymity", and nothing here re-measures them. The scope
  // sentence bounds the counterfactual to the columns the measure read: this is the one place
  // the panel hands over a number to act on, and without it a reader who removes the named
  // column expects a clean file and is wrong for a reason nothing on the page told them.
  return (
    <p className="muted-text text-sm">
      Removing {nameFor(best.columnIndex)} from the file would leave{' '}
      {best.uniqueRowsWithout} of them unique instead of{' '}
      {summary.uniqueRows}. That is counted over the same columns as the
      figures above and no others, and the group sizes behind it are not re-measured.
    </p>
  )
}

function MatchedColumnLine({ names, lead, note }: { names: string[]; lead: string; note: string }) {
  if (names.length === 0) return null

  return (
    <p className="muted-text text-sm">
      {lead} {names.join(', ')} — {note}.
    </p>
  )
}

function ReadinessNotes({ privacyReport }: { privacyReport: PrivacyReport }) {
  const readiness = privacyReport.readiness
  const items = readiness.status === 'blocked'
    ? readiness.blockers
    : readiness.status === 'review'
      ? readiness.reviewItems
      : []

  if (items.length === 0) return null

  return (
    <div className="report-readiness-notes">
      <strong>{readiness.status === 'blocked' ? 'Blocked by' : 'Needs review'}</strong>
      <ul>
        {items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  )
}
