import { render, screen, within } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { columnReportFixture, privacyReportFixture, rowUniquenessFixture } from '../test-utils/builders'
import { PrivacyReportSummary } from './PrivacyReportSummary'

/** A column an outsider can match cell for cell — the plain case the other `matchedOn` values are read against. */
const matchedWholeValue = (columnIndex: number) => ({
  columnIndex,
  matchedOn: 'wholeValue' as const,
  matchedEveryRow: true,
})

describe('PrivacyReportSummary', () => {
  it('shows a compact overview and omits zero-only advanced metrics', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          directIdentifiers: 1,
          pseudonymizedColumns: 1,
          passThroughColumns: 2,
          uniquePseudonymValues: 3,
        })}
      />,
    )

    expect(screen.getByText('Privacy Report')).toBeInTheDocument()
    expect(screen.getByText('Readiness')).toBeInTheDocument()
    expect(screen.getByText('Columns transformed')).toBeInTheDocument()
    expect(screen.getByText('1 pseudonymized')).toBeInTheDocument()
    expect(screen.getByText('Pass-through/no-op')).toBeInTheDocument()
    expect(screen.getByText('Advanced Counts')).toBeInTheDocument()
    expect(screen.queryByText('Pool exhaustions')).not.toBeInTheDocument()
    expect(screen.queryByText('Smart Replacement')).not.toBeInTheDocument()
  })

  it('prints a count Rust sent as a string exactly as Rust wrote it', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          utilityMetrics: [
            // What `release_report::build_utility_metrics` actually emits: one bare count via
            // `reused_pseudonym_values.to_string()`, and one ratio.
            { label: 'Repeat reuse', value: '999999', status: 'info', detail: null },
            { label: 'Selected columns', value: '3/7', status: 'review', detail: null },
          ],
        })}
      />,
    )

    // A plain literal, and it can be one now: nothing in this panel reformats a number, so
    // the assertion no longer has to be computed with `toLocaleString` to survive being run
    // on a machine with a different locale. That portability is the point of the rule.
    expect(screen.getByText('999999')).toBeInTheDocument()
    expect(screen.queryByText((999999).toLocaleString())).not.toBeInTheDocument()
    expect(screen.getByText('3/7')).toBeInTheDocument()
  })

  it('shows Smart replacement details only when Smart replacement was used', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          smartReplacementColumns: 1,
          smartReplacementValues: 4,
          smartReplacementRejections: 1,
          smartReplacementRejectionReasons: [{ reason: 'containsOriginal', count: 1 }],
        })}
      />,
    )

    expect(screen.getByText('Smart Replacement')).toBeInTheDocument()
    expect(screen.getByText('Smart replacement columns')).toBeInTheDocument()
    expect(screen.getByText('Source text included')).toBeInTheDocument()
  })

  it('keeps column decisions compact in a labelled table', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [
            columnReportFixture({
              columnIndex: 2,
              columnName: 'email',
              detectedType: 'email',
              action: 'Redacted values',
            }),
          ],
        })}
      />,
    )

    const table = screen.getByRole('table', { name: /privacy report column decisions/i })
    expect(within(table).getByText('email')).toBeInTheDocument()
    expect(within(table).getByText('#2 / Email')).toBeInTheDocument()
    expect(within(table).getByText('Redacted values')).toBeInTheDocument()
    expect(screen.getByText('Showing 1 of 1')).toBeInTheDocument()
  })

  it('renders every column decision instead of hiding later columns', () => {
    const columnReports = Array.from({ length: 13 }, (_, index) =>
      columnReportFixture({ columnIndex: index, columnName: `column-${index}` }),
    )

    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({ columnReports })}
      />,
    )

    expect(screen.getByText('Showing 13 of 13')).toBeInTheDocument()
    expect(screen.getByText('column-12')).toBeInTheDocument()
  })

  it('surfaces readiness review items above the details sections', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          readiness: {
            status: 'review',
            blockers: [],
            reviewItems: ['Check low confidence columns'],
            verifiedItems: [],
          },
        })}
      />,
    )

    expect(screen.getByText('Needs review')).toBeInTheDocument()
    expect(screen.getByText('Check low confidence columns')).toBeInTheDocument()
  })

  it('renders every readiness item that needs review', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          readiness: {
            status: 'review',
            blockers: [],
            reviewItems: ['Review 1', 'Review 2', 'Review 3', 'Review 4'],
            verifiedItems: [],
          },
        })}
      />,
    )

    expect(screen.getByText('Review 4')).toBeInTheDocument()
  })
})

describe('PrivacyReportSummary joint re-identifiability', () => {
  // Only the index-to-name mapping matters below: every assertion in this block is about a
  // sentence that names a column the joint measure counted.
  const columnReports = [
    columnReportFixture({ columnIndex: 1, columnName: 'postal_code' }),
    columnReportFixture({ columnIndex: 2, columnName: 'birth_date' }),
  ]

  it('names the columns the count was taken over rather than their indices', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 5000,
            matchedColumns: [matchedWholeValue(1), matchedWholeValue(2)],
            distinctClasses: 3200,
            uniqueRows: 412,
            distinctRowsAllColumns: 5000,
          }),
        })}
      />,
    )

    expect(screen.getByText('Joint re-identifiability')).toBeInTheDocument()
    expect(screen.getByText('Rows singled out')).toBeInTheDocument()
    expect(screen.getByText('412')).toBeInTheDocument()
    // An index answers "unique on what?" only for a reader holding the file open.
    expect(screen.getByText(/postal_code, birth_date/)).toBeInTheDocument()
  })

  it('renders nothing at all when there were no rows to measure', () => {
    render(<PrivacyReportSummary privacyReport={privacyReportFixture({ rowUniqueness: null })} />)

    // Absent, not empty. A blank table would read as a measurement that found nothing.
    expect(screen.queryByText('Joint re-identifiability')).not.toBeInTheDocument()
  })

  it('says an unmeasured file is unmeasured rather than showing zeroes', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 2000000,
            measurementIncomplete: true,
            // Everything the stopped pass never counted, spelled out: these are the zeroes
            // the panel must not present as findings.
            distinctClasses: 0,
            uniqueRows: 0,
            smallestClass: 0,
            fifthPercentileClassSize: 0,
            distinctRowsAllColumns: 0,
          }),
          columnReports,
        })}
      />,
    )

    expect(screen.getByText('not measured')).toBeInTheDocument()
    expect(screen.getByText(/not the same as measured clean/)).toBeInTheDocument()
    // The zeroed counts must not be presented as findings.
    expect(screen.queryByText('Rows singled out')).not.toBeInTheDocument()
  })

  it('reports an empty linkable subset as not applicable, not as a pass', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          rowUniqueness: rowUniquenessFixture({ rowsMeasured: 20, matchedColumns: [] }),
        })}
      />,
    )

    expect(screen.getByText('not applicable')).toBeInTheDocument()
    expect(screen.getByText(/not a finding that the rows cannot be re-identified/)).toBeInTheDocument()
  })

  it('says which single column to drop, and by how much it would help', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 5000,
            matchedColumns: [matchedWholeValue(1), matchedWholeValue(2)],
            distinctClasses: 3200,
            uniqueRows: 412,
            distinctRowsAllColumns: 5000,
            dropColumnEffects: [
              { columnIndex: 2, uniqueRowsWithout: 3 },
              { columnIndex: 1, uniqueRowsWithout: 400 },
            ],
            dropAttributionIncomplete: false,
          }),
        })}
      />,
    )

    // The only line here anyone can act on, and it names the best column rather than the
    // first: the head of the list is sorted by effect, not by column order.
    expect(
      screen.getByText(/Removing birth_date from the file would leave 3 of them unique instead of 412/),
    ).toBeInTheDocument()
    // The bound on that number. The panel mirrors `release_report::drop_column_advice`, and
    // both used to hand over the actionable figure without saying what it was counted over —
    // the scope was stated only on the verified arm, which a reviewed file never reaches.
    expect(
      screen.getByText(/counted over the same columns as the figures above and no others/),
    ).toBeInTheDocument()
  })


  it('trusts the flag over the list when the two disagree', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 5000,
            matchedColumns: [matchedWholeValue(2)],
            distinctClasses: 3200,
            uniqueRows: 412,
            distinctRowsAllColumns: 5000,
            // Not a shape the tracker produces — the flag and a populated list are meant to be
            // exclusive. It is asserted anyway because every other fixture pairs `true` with an
            // empty list, which lets an implementation branch on `length === 0` and pass all of
            // them, defeating the one distinction this field exists to make.
            dropColumnEffects: [{ columnIndex: 2, uniqueRowsWithout: 3 }],
            dropAttributionIncomplete: true,
          }),
        })}
      />,
    )

    expect(screen.getByText(/was not measured on this file/)).toBeInTheDocument()
    expect(screen.queryByText(/Removing/)).not.toBeInTheDocument()
  })

  it('renders an empty effect list without crashing when nothing was flagged', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 5000,
            matchedColumns: [matchedWholeValue(2)],
            distinctClasses: 3200,
            uniqueRows: 412,
            distinctRowsAllColumns: 5000,
            // The other inconsistent shape. `noUncheckedIndexedAccess` is off, so `[0]` is
            // typed as present and the guard against it is invisible to the compiler — this is
            // what stops it being deleted as dead code and taking the panel down with it.
            dropColumnEffects: [],
            dropAttributionIncomplete: false,
          }),
        })}
      />,
    )

    expect(screen.getByText('Joint re-identifiability')).toBeInTheDocument()
    expect(screen.queryByText(/Removing/)).not.toBeInTheDocument()
    expect(screen.queryByText(/No single column carries it/)).not.toBeInTheDocument()
  })

  it('prints both counts the way the Rust sentence beside them prints the same figures', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 20000,
            matchedColumns: [matchedWholeValue(2)],
            distinctClasses: 9000,
            uniqueRows: 4123,
            distinctRowsAllColumns: 20000,
            dropColumnEffects: [{ columnIndex: 2, uniqueRowsWithout: 1 }],
            dropAttributionIncomplete: false,
          }),
        })}
      />,
    )

    // Raw integers, matching `drop_column_advice` in `release_report.rs` character for
    // character. Rust builds that same sentence into the evidence detail and the readiness
    // review list, both rendered further down this panel, so the two are on screen together —
    // and while this side grouped, a reader saw "instead of 4,123" above "instead of 4123"
    // and had to work out whether those were one number or two. Rust has no locale here and
    // cannot be given one, so the raw integer is the only rendering both sides can agree on.
    //
    // The literal is also the regression test for the locale bug underneath it: this
    // assertion used to be built by calling `toLocaleString()` at test time purely so the
    // suite would survive being run on a Dutch machine, where the panel said "4.123".
    expect(
      screen.getByText(/Removing birth_date from the file would leave 1 of them unique instead of 4123/),
    ).toBeInTheDocument()
  })


  it('states that no single column carries it rather than falling silent', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 40,
            matchedColumns: [matchedWholeValue(1)],
            distinctClasses: 40,
            uniqueRows: 40,
            distinctRowsAllColumns: 40,
            // Dropping the column changes nothing: 40 still alone out of 40.
            dropColumnEffects: [{ columnIndex: 1, uniqueRowsWithout: 40 }],
            dropAttributionIncomplete: false,
          }),
        })}
      />,
    )

    expect(screen.getByText(/No single column carries it/)).toBeInTheDocument()
    expect(screen.queryByText(/Removing/)).not.toBeInTheDocument()
  })

  it('distinguishes an unmeasured attribution from no column helping', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 40,
            matchedColumns: [matchedWholeValue(1)],
            distinctClasses: 40,
            uniqueRows: 40,
            distinctRowsAllColumns: 40,
            dropColumnEffects: [],
            dropAttributionIncomplete: true,
          }),
        })}
      />,
    )

    // Silence here reads as "no column would help", which is the opposite finding.
    expect(screen.getByText(/was not measured on this file/)).toBeInTheDocument()
    expect(screen.queryByText(/No single column carries it/)).not.toBeInTheDocument()
  })

  it('offers no column to drop on a file with nothing to clear', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports,
          rowUniqueness: rowUniquenessFixture({
            matchedColumns: [matchedWholeValue(1)],
            distinctClasses: 10,
            // Nothing singled out, so there is no count for a dropped column to improve.
            uniqueRows: 0,
            dropColumnEffects: [{ columnIndex: 1, uniqueRowsWithout: 0 }],
            dropAttributionIncomplete: false,
          }),
        })}
      />,
    )

    expect(screen.queryByText(/Removing/)).not.toBeInTheDocument()
    expect(screen.queryByText(/No single column carries it/)).not.toBeInTheDocument()
    expect(screen.queryByText(/was not measured on this file/)).not.toBeInTheDocument()
  })
})

describe('PrivacyReportSummary shape-only columns', () => {
  it('names format-only contributors apart from value-carrying ones', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [
            columnReportFixture({ columnIndex: 1, columnName: 'postal_code' }),
            columnReportFixture({ columnIndex: 4, columnName: 'customer_id' }),
          ],
          rowUniqueness: rowUniquenessFixture({
            matchedColumns: [
              matchedWholeValue(1),
              { columnIndex: 4, matchedOn: 'survivingFormat', matchedEveryRow: true },
            ],
            distinctClasses: 40,
            uniqueRows: 3,
          }),
        })}
      />,
    )

    // Two columns counted, reported on two lines. Merging them would read as though the
    // customer id had singled the rows out, when all it contributed was its width.
    expect(screen.getByText('2 columns')).toBeInTheDocument()
    expect(screen.getByText(/Counted over postal_code —/)).toBeInTheDocument()
    expect(
      screen.getByText(/Counted by surviving format only: customer_id —/),
    ).toBeInTheDocument()
  })

  it('does not open on a conjunction when there is nothing for it to follow', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [columnReportFixture({ columnIndex: 4, columnName: 'customer_id' })],
          // No value-carrying column at all: the format-only line is the first one rendered.
          rowUniqueness: rowUniquenessFixture({
            matchedColumns: [{ columnIndex: 4, matchedOn: 'survivingFormat', matchedEveryRow: true }],
            distinctClasses: 40,
            uniqueRows: 3,
          }),
        })}
      />,
    )

    // Every line states what it counted and why on its own, so none can be left opening on
    // a conjunction whose antecedent was not rendered — which is what "Also counted, by
    // surviving format only…" did when there was no value-carrying column above it.
    expect(screen.queryByText(/^Also/)).not.toBeInTheDocument()
    expect(
      screen.getByText(/Counted by surviving format only: customer_id —/),
    ).toBeInTheDocument()
  })

  it('reports a suppressed all-column count as absent rather than as zero', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [columnReportFixture({ columnIndex: 1, columnName: 'postal_code' })],
          rowUniqueness: rowUniquenessFixture({
            rowsMeasured: 3000000,
            // The histogram over whole rows filled and was dropped; the joint measure did
            // not, and its figures are still the file's.
            distinctRowsAllColumns: null,
          }),
        })}
      />,
    )

    // The joint figures are still the file's, so they render...
    expect(screen.queryByText('not measured')).not.toBeInTheDocument()
    expect(screen.getByText('Rows singled out')).toBeInTheDocument()
    // ...and the suppressed figure is omitted rather than shown as a zero. That assertion is
    // the whole test: the component read this field nowhere at all, so the earlier version
    // passed just as happily with a number in the fixture.
    expect(screen.queryByText('Distinct released rows')).not.toBeInTheDocument()
  })

  it('shows the all-column count when it was measured', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [columnReportFixture({ columnIndex: 1, columnName: 'postal_code' })],
          rowUniqueness: rowUniquenessFixture({ distinctRowsAllColumns: 97 }),
        })}
      />,
    )

    expect(screen.getByText('Distinct released rows')).toBeInTheDocument()
    expect(screen.getByText('97')).toBeInTheDocument()
  })

  it('quotes a column name containing a comma, as the Rust finding does', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [columnReportFixture({ columnIndex: 1, columnName: 'city, state' })],
          rowUniqueness: rowUniquenessFixture(),
        })}
      />,
    )

    // Unquoted, this rendered as two names under a label reading "1 column", inventing a
    // column called `city` and one called `state`. Rust quotes it; this must agree.
    expect(screen.getByText(/Counted over "city, state" —/)).toBeInTheDocument()
    expect(screen.getByText('1 column')).toBeInTheDocument()
  })

  it('names a blank-cell pattern as what it is, not as the column', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [
            columnReportFixture({
              columnIndex: 3,
              columnName: 'address',
              detectedType: 'address',
              action: 'Redact',
            }),
          ],
          rowUniqueness: rowUniquenessFixture({
            matchedColumns: [{ columnIndex: 3, matchedOn: 'blankPattern', matchedEveryRow: true }],
            distinctClasses: 2,
          }),
        })}
      />,
    )

    // A redacted column still publishes which of its cells were blank. Naming it "address"
    // would say the addresses are in the file; they are not.
    expect(screen.queryByText(/Counted over/)).not.toBeInTheDocument()
    expect(screen.getByText(/Counted by which cells are blank: address —/)).toBeInTheDocument()
  })

  it('names a date column as matched on its decade, not on its date', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [columnReportFixture({ columnIndex: 2, columnName: 'birth_date' })],
          rowUniqueness: rowUniquenessFixture({
            matchedColumns: [{ columnIndex: 2, matchedOn: 'dateDecadeAndTime', matchedEveryRow: true }],
            distinctClasses: 4,
          }),
        })}
      />,
    )

    // Not "Counted over birth_date", which would claim the released date is the real one.
    expect(screen.queryByText(/Counted over/)).not.toBeInTheDocument()
    expect(
      screen.getByText(/Counted by decade and time of day only: birth_date —/),
    ).toBeInTheDocument()
    expect(screen.getByText(/can move between runs/)).toBeInTheDocument()
    // Every row carries it here, so the partial-match line must stay away — otherwise its
    // appearance elsewhere tells the reader nothing.
    expect(screen.queryByText(/Only some of the released rows carry/)).not.toBeInTheDocument()
  })

  it('says when only some rows carry what a column was matched on', () => {
    render(
      <PrivacyReportSummary
        privacyReport={privacyReportFixture({
          columnReports: [columnReportFixture({ columnIndex: 2, columnName: 'birth_date' })],
          rowUniqueness: rowUniquenessFixture({
            // The case that motivated the flag: `matchedOn` is fixed by the column's strategy
            // and detected type, so a timestamp column where one value in a hundred parses is
            // still `dateDecadeAndTime` — and the line above claimed the decade of all
            // hundred rows, ninety-nine of which carry no decade at all.
            matchedColumns: [{ columnIndex: 2, matchedOn: 'dateDecadeAndTime', matchedEveryRow: false }],
            distinctClasses: 4,
          }),
        })}
      />,
    )

    expect(
      screen.getByText(/Only some of the released rows carry what birth_date was matched on/),
    ).toBeInTheDocument()
    // Without this the line reads as doubt about the grid above it, and the grid is right:
    // those rows were counted as sharing nothing on that column.
    expect(
      screen.getByText(/already treat those as sharing nothing there/),
    ).toBeInTheDocument()
  })
})
