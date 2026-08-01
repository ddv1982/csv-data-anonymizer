use crate::report_notes::{
    push_detection_coverage_note, push_smart_replacement_leak_guard_note,
    push_unselected_column_note,
};
use crate::service::redaction_changes_structured_scalar_type;
use crate::strategies::{MASK_STRUCTURE_DISCLOSURE, STRUCTURED_SCALAR_REDACTION_WARNING};
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, ColumnReleaseReport, DataType, DetectionCoverage,
    FrequencyInversionRisk, GENERIC_STRING_STRUCTURE_DISCLOSURE, MatchedPart, ReleaseEvidenceItem,
    ReleaseEvidenceStatus, ReleaseReadiness, ReleaseReadinessStatus, RowUniquenessSummary,
    TransformReport, UtilityMetric,
};

/// Group size at or above which the joint measure is allowed to read as verified.
///
/// Five is the convention that k-anonymity is usually quoted at, and it is a convention
/// rather than a law — which is exactly why it sets the wording of a report and not the
/// success of a run. See the module's plan document for why this measure never blocks.
const VERIFIED_GROUP_FLOOR: usize = 5;

/// A column's name as the finding should print it.
///
/// Quoted when the name itself contains a comma, because the names are joined with commas: a
/// column genuinely called `city, state` would otherwise read as two columns, and a reader
/// counting the listed columns against the "N columns" label would find them disagreeing and
/// have no way to tell which was wrong.
fn column_name(columns: &[ColumnMetadata], index: usize) -> String {
    let name = columns
        .iter()
        .find(|column| column.index == index)
        .map(|column| column.name.clone())
        // A header is stored verbatim, so it can be empty or all spaces, and such a column
        // printed as nothing at all: "unique on , city" told a reader they were unique on a
        // comma. The positional fallback at least identifies it.
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| format!("column {index}"));
    if name.contains(',') || name.contains('"') {
        // Escaped, not merely wrapped. A header `he said "hi", ok` wrapped bare would close
        // its own quote halfway through, leaving it ambiguous under exactly the reading the
        // quoting was added to prevent.
        return format!("\"{}\"", name.replace('"', "\\\""));
    }
    name
}

/// How a group of columns matched on the same thing is described.
///
/// Only `WholeValue` returns `None`, and that is the whole point of the type: a bare column
/// name is a claim that the released cell *is* the value an outsider holds, so every other
/// variant has to say what it actually kept. Naming a pseudonymized email or a shifted birth
/// date bare, and then asserting that rows share "their combination of" them, is false on
/// every file holding either column.
fn matched_part_prefix(part: MatchedPart) -> Option<&'static str> {
    match part {
        MatchedPart::WholeValue => None,
        MatchedPart::EmailDomain => Some("the domain of"),
        // Named as a decade rather than as a date, because that is the resolution an
        // attacker actually gets: the ±365-day shift leaves them a two-year window, and a
        // decade is the coarsening that neither pretends the column is unmatchable nor
        // pretends every row is unique on it.
        MatchedPart::DateDecadeAndTime => Some("the decade and time of"),
        MatchedPart::SurvivingFormat => Some("the surviving format of"),
        MatchedPart::BlankPattern => Some("which cells are blank in"),
    }
}

/// Where a group sits in the finding, strongest claim first.
///
/// Exhaustive with no wildcard arm, which is the whole reason it is a function rather than an
/// array of variants: a `MatchedPart` added later cannot be silently left out of the sentence,
/// it has to be given a place here or the build fails.
fn group_order(part: MatchedPart) -> usize {
    match part {
        MatchedPart::WholeValue => 0,
        MatchedPart::EmailDomain => 1,
        MatchedPart::DateDecadeAndTime => 2,
        MatchedPart::SurvivingFormat => 3,
        // Last, because it is the weakest claim and the one a reader is least expecting: the
        // cell is not in the file at all, only the fact that it was empty.
        MatchedPart::BlankPattern => 4,
    }
}

fn render_group(part: MatchedPart, names: &[String]) -> String {
    let names = names.join(", ");
    match matched_part_prefix(part) {
        Some(prefix) => format!("{prefix} {names}"),
        None => names,
    }
}

/// The columns the measure counted, by name, with the shape-only ones marked as such.
///
/// Names rather than indices because the reader's first question on being told their rows
/// are unique is "on what?", and an index answers it only for someone holding the file
/// open beside the report.
///
/// The two groups are named apart because they answer that question differently. A
/// released postcode is what singles a row out; a customer id that survived only as a
/// five-digit width helped, but listing them together would credit it with far more than
/// it did, and a reader who then removes the wrong column has been misled by the report
/// that was supposed to help them.
fn counted_column_names(columns: &[ColumnMetadata], summary: &RowUniquenessSummary) -> String {
    // Grouped by what was matched, so a file with four format-only columns reads "the
    // surviving format of a, b, c, d" rather than repeating the phrase four times.
    //
    // Ordered by an exhaustive `match` (`group_order`) rather than by listing the variants in
    // an array, deliberately: a `MatchedPart` added later is then a compile error here instead
    // of a variant counted into the class arithmetic and named nowhere in the sentence.
    let mut ordered = summary
        .matched_columns
        .iter()
        .map(|matched| (group_order(matched.matched_on), matched))
        .collect::<Vec<_>>();
    // Stable, so columns keep their file order inside each group.
    ordered.sort_by_key(|(order, _)| *order);

    let mut groups: Vec<String> = Vec::new();
    let mut current: Option<(MatchedPart, Vec<String>)> = None;
    for (_, matched) in ordered {
        let name = column_name(columns, matched.column_index);
        match &mut current {
            Some((part, names)) if *part == matched.matched_on => names.push(name),
            _ => {
                if let Some((part, names)) = current.take() {
                    groups.push(render_group(part, &names));
                }
                current = Some((matched.matched_on, vec![name]));
            }
        }
    }
    if let Some((part, names)) = current {
        groups.push(render_group(part, &names));
    }

    // Semicolons between groups, commas within them. A comma join with an Oxford comma
    // marked only the *first* boundary: "postal_code, city, and the surviving format of
    // phone, customer_id" reads `customer_id` as a fourth column released as it stands, which
    // is exactly the misreading the grouping exists to prevent — the reader removes the wrong
    // column. A separator that cannot occur inside a group is the only kind that survives a
    // group holding more than one name.
    groups.join("; ")
}

/// `unique` as a share of `total`, never rounded down to nothing.
///
/// One decimal place turns 1 row in 10,000 into "0.0%", so a report whose headline figure
/// reads as "none" was being issued about a file where one person is individually
/// identifiable. The interesting range of this number starts at a handful of rows in a
/// million, which is below any fixed precision worth printing, so the small case is worded
/// instead of rounded.
fn unique_share(unique: usize, total: usize) -> String {
    if total == 0 {
        // Not reachable: a summary exists only once a row has been measured. Guarded anyway,
        // because the alternative to a wrong percentage here is a NaN in a privacy report.
        return "0.0%".to_string();
    }
    let share = (unique as f64) * 100.0 / (total as f64);
    if unique > 0 && share < 0.05 {
        return "under 0.1%".to_string();
    }
    format!("{share:.1}%")
}

/// What has to be said about the subset before any count over it can be read.
///
/// Empty for most files. A date column is the exception, and it needs saying at the point of
/// the claim rather than in a doc comment: its match is at decade resolution, which is
/// coarser than the two-year window an attacker holding a real date actually gets, and which
/// moves between runs because the shift is redrawn per value. Both facts make the number
/// beside it mean less than it appears to, and neither is guessable from the number.
fn approximate_match_caveat(
    summary: &RowUniquenessSummary,
    quotes_group_size: bool,
) -> &'static str {
    let has_date = summary
        .matched_columns
        .iter()
        .any(|matched| matched.matched_on == MatchedPart::DateDecadeAndTime);
    if !has_date {
        return "";
    }
    // The same approximation, but its error points opposite ways depending on which figure
    // the sentence quotes, so the caveat cannot be one string. A decade is coarser than the
    // attacker's two-year window, so it merges rows: group sizes come out too *large* and the
    // count of singled-out rows too *small*. Appending "treat this group size as an upper
    // bound" to a sentence quoting only `unique_rows` told a reader "at most this many people
    // are identifiable" when the truth is "at least".
    if quotes_group_size {
        " A shifted date is matched at decade resolution: someone holding a real date narrows \
         to a two-year window inside that decade rather than to the decade itself, so treat \
         this group size as an upper bound, and expect it to move between runs when a shift \
         crosses a decade boundary."
    } else {
        " A shifted date is matched at decade resolution, which is coarser than the two-year \
         window someone holding a real date actually narrows to, so treat this count as a \
         lower bound, and expect it to move between runs when a shift crosses a decade \
         boundary."
    }
}

/// Names the columns whose matched part only some of the rows actually carry.
///
/// The sentence above this one describes a column by what its strategy and detected type make
/// reproducible, which is decided once per column and cannot be told otherwise by any cell.
/// The cells can still disagree: one that does not fit its column's detected shape is
/// pseudonymized generically and the projection returns nothing for that row. A `Timestamp`
/// column where one value in a hundred parses is still `DateDecadeAndTime`, so the finding
/// said the rows share "the decade and time of birth_date" — of ninety-nine rows carrying no
/// decade at all.
///
/// The counts were right throughout, and that is worth saying because it decides the wording:
/// those rows were hashed as sharing nothing on that column, which is what an outsider holding
/// the original also gets. So this qualifies the *phrase*, and says the arithmetic already
/// accounts for it, rather than casting doubt on a figure that is sound.
fn partial_match_caveat(columns: &[ColumnMetadata], summary: &RowUniquenessSummary) -> String {
    let partial = summary
        .matched_columns
        .iter()
        .filter(|matched| !matched.matched_every_row)
        .map(|matched| column_name(columns, matched.column_index))
        .collect::<Vec<_>>();
    if partial.is_empty() {
        return String::new();
    }

    format!(
        " Only some of the released rows carry what {} was matched on: a cell that did not fit \
         its column's detected shape was replaced generically, and the rows above already count \
         those as sharing nothing there.",
        partial.join(", ")
    )
}

/// Which single column to change, appended to a finding that has just quoted `unique_rows`.
///
/// The one sentence in this item a reader can act on, and the reason it is a sentence rather
/// than a number: "412 rows are unique" invites "so what do I do?", and the answer is not
/// guessable from the per-column risk levels beside it. The column that carries a joint
/// finding is routinely not the one that looks most dangerous alone — a low-risk postcode
/// column with a hundred distinct values will out-carry a high-risk name column that masking
/// already flattened to three distinct skeletons.
///
/// Leads with the empty string on the paths where nothing can honestly be said, so the caller
/// can append it unconditionally.
///
/// "Removing from the file", never "dropping". The verb has to name the measured action, and
/// the measured action is the column not being released at all — the count comes from a class
/// key built without it. The two things a reader can do in this app are both *not that*, and
/// one of them is actively worse: unticking a column writes it through unchanged
/// (`report_notes`), which `uniqueness::LinkableProjection::for_column` then reads as
/// `WholeValue`. A reader told to "drop birth_date" who unticks it releases the raw dates.
/// Redacting is closer but still short, because an empty cell is written through verbatim and
/// the blank-cell pattern survives it.
fn drop_column_advice(columns: &[ColumnMetadata], summary: &RowUniquenessSummary) -> String {
    if summary.drop_attribution_incomplete {
        // Said rather than omitted. Silence here is indistinguishable from "no column would
        // help", which is the opposite finding and the one that would stop a reader looking.
        return " Which single column carries this was not measured on this file.".to_string();
    }

    let Some(best) = summary.drop_column_effects.first() else {
        // Not reachable from the caller today: it returns early on an empty `matched_columns`,
        // and the two lists are filtered by one predicate. It is still guarded rather than
        // indexed, because `RowUniquenessSummary` is `Deserialize` and so can arrive from
        // outside the tracker that maintains that relationship, and the alternative to a guard
        // is an index panic inside a privacy report.
        return String::new();
    };

    if best.unique_rows_without >= summary.unique_rows {
        // Every matched column measured, none of them decisive. That is a real finding and a
        // different instruction: the uniqueness is spread across the combination, so changing
        // one column will not clear it however the strategies are set.
        return " No single column carries it: removing any one of the columns named above \
                would leave the same rows unique, so the combination has to be broken in more \
                than one place."
            .to_string();
    }

    // Quotes the count of rows still standing alone, and says so, rather than implying the
    // file would then be safe. Dropping the column can take the singletons to zero and still
    // leave groups of two, which this same item calls "not anonymity" three lines down —
    // `DropColumnEffect` carries no post-drop group size to check that against.
    //
    // The second sentence bounds the counterfactual to the columns the measure read, and has
    // to stay: this is the arm that hands the reader a number to act on, on the file where
    // somebody is actually deciding what to change. Without the bound, a reader who removes
    // the named column and expects the file to be clean is wrong for a reason nothing on the
    // page told them.
    format!(
        " Removing {} from the file would leave {} of them unique instead of {}. That is \
         counted over the same columns as the figures above and no others, and the group \
         sizes behind it are not re-measured.",
        column_name(columns, best.column_index),
        best.unique_rows_without,
        summary.unique_rows
    )
}

/// What the joint measure found, as one evidence item.
///
/// Sits next to the per-column detector-risk item deliberately: that item can read
/// verified on a file this one calls a review, and a reader who sees only the first would
/// draw the wrong conclusion. They are two answers to two different questions and belong
/// side by side.
pub(crate) fn row_uniqueness_evidence(
    columns: &[ColumnMetadata],
    summary: &RowUniquenessSummary,
) -> ReleaseEvidenceItem {
    let label = "Joint re-identifiability".to_string();
    let id = "row-uniqueness".to_string();

    if summary.measurement_incomplete {
        return ReleaseEvidenceItem {
            id,
            label,
            // Never verified on an unmeasured file. An absent measurement is not a clean
            // one, and this is the status that keeps the two from reading alike.
            status: ReleaseEvidenceStatus::Review,
            detail: format!(
                "Not measured: this file holds more distinct combinations than the check keeps. \
                 {} row(s) were read before it stopped.",
                summary.rows_measured
            ),
        };
    }

    if summary.matched_columns.is_empty() {
        return ReleaseEvidenceItem {
            id,
            label,
            // Info rather than Verified. Nothing was matchable *by strategy*, which is a
            // statement about how the columns were transformed and not a finding that the
            // data is anonymous — a column this check excludes can still be revealing.
            status: ReleaseEvidenceStatus::Info,
            detail: "No released column carries anything an outsider could match against data \
                     they already hold, so no joint measure applies. This describes how the \
                     columns were transformed, not a finding that the rows cannot be \
                     re-identified."
                .to_string(),
        };
    }

    let names = counted_column_names(columns, summary);

    if summary.unique_rows > 0 {
        return ReleaseEvidenceItem {
            id,
            label,
            status: ReleaseEvidenceStatus::Review,
            detail: format!(
                "{} of {} released row(s) ({}) are unique on {}. Anyone holding those fields for \
                 a person finds that person's row, however each column reads on its own.{}{}{}",
                summary.unique_rows,
                summary.rows_measured,
                unique_share(summary.unique_rows, summary.rows_measured),
                names,
                approximate_match_caveat(summary, false),
                partial_match_caveat(columns, summary),
                drop_column_advice(columns, summary)
            ),
        };
    }

    // The smallest class is the figure tested, and it has to be: a file whose smallest group
    // holds two rows and whose fifth percentile sits at ninety-nine must not reach the
    // verified arm below. This arm says in as many words that a group that small is not
    // anonymity.
    //
    // The percentile is deliberately *not* also tested. It is drawn from a list sorted
    // ascending whose first element is `smallest_class`, so it can never be the smaller of
    // the two, and a second disjunct for it would be unreachable code advertising a check
    // that does not exist. It is reported rather than gated, which is its job: the floor
    // says whether anyone is exposed, the percentile says whether many are.
    if summary.smallest_class < VERIFIED_GROUP_FLOOR {
        return ReleaseEvidenceItem {
            id,
            label,
            status: ReleaseEvidenceStatus::Review,
            detail: format!(
                "No released row stands alone, but the smallest group on {} holds {} row(s), under \
                 the floor of {}, and the most exposed 5% sit in groups of {} or fewer. A group \
                 that small is not anonymity.{}{}",
                names,
                summary.smallest_class,
                VERIFIED_GROUP_FLOOR,
                summary.fifth_percentile_class_size,
                approximate_match_caveat(summary, true),
                partial_match_caveat(columns, summary)
            ),
        };
    }

    ReleaseEvidenceItem {
        id,
        label,
        status: ReleaseEvidenceStatus::Verified,
        detail: format!(
            "Every released row shares {} with at least {} other(s), and the most exposed 5% sit \
             in groups of {} or fewer. Measured over those columns and no others — a column this \
             check excludes can still be revealing on its own.{}{}",
            names,
            summary.smallest_class.saturating_sub(1),
            summary.fifth_percentile_class_size,
            approximate_match_caveat(summary, true),
            partial_match_caveat(columns, summary)
        ),
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ReportContext<'a> {
    pub transform_report: Option<&'a TransformReport>,
}

pub(crate) fn build_readiness(
    columns: &[ColumnMetadata],
    context: &ReportContext<'_>,
) -> ReleaseReadiness {
    let mut review_items = Vec::new();
    let mut verified_items = Vec::new();

    verified_items.push(
        "Replacements are randomized per run; repeated source values stay consistent within the current output."
            .to_string(),
    );

    let unselected_risky = unselected_detector_risk_columns(columns);
    if unselected_risky.is_empty() {
        verified_items
            .push("No high/medium detector-risk columns were left unselected.".to_string());
    } else {
        review_items.push(format!(
            "{} high/medium detector-risk column(s) are outside this release: {}.",
            unselected_risky.len(),
            unselected_risky.join(", ")
        ));
    }

    // Pushed before the general disclaimer below, so a reader who stops at the first
    // concrete finding stops at a number about their own file rather than at a caveat
    // that is true of every file.
    if let Some(summary) = context
        .transform_report
        .and_then(|report| report.row_uniqueness.as_ref())
    {
        let item = row_uniqueness_evidence(columns, summary);
        match item.status {
            // The evidence item and the readiness list must not be able to disagree, so
            // the readiness entry is the evidence detail rather than a second wording of
            // the same finding.
            // `Blocked` cannot arrive from `row_uniqueness_evidence`, which never blocks, but
            // it is a live status elsewhere (`service::preflight` raises it for an unwritable
            // output path and for Local AI not being ready). Reviewing it is not an
            // under-statement: readiness blocks on `blockers`, which this function builds
            // empty, so anyone making this measure block has to add the blocker there too.
            ReleaseEvidenceStatus::Review | ReleaseEvidenceStatus::Blocked => {
                review_items.push(item.detail);
            }
            ReleaseEvidenceStatus::Verified => verified_items.push(item.detail),
            // An empty linkable subset is reported, and is deliberately neither a review
            // item nor a verified one: promoting it to verified would let "nothing was
            // matchable" be read as "these rows cannot be re-identified".
            ReleaseEvidenceStatus::Info => {}
        }
    }

    // Unconditional, and that is the whole stance: this tool does not get to certify a file
    // as anonymous. Because it always lands, `review_items` is never empty, which is what
    // makes the status below a constant rather than a decision. A test pins that.
    review_items.push(
        "CSV transforms reduce exposure but are not a formal anonymity guarantee.".to_string(),
    );
    if let Some(report) = context.transform_report
        && report.smart_replacement_rejections > 0
    {
        review_items.push(format!(
            "{} Local AI replacement candidate(s) were rejected before fallback handling: {}.",
            report.smart_replacement_rejections,
            smart_rejection_summary(report)
        ));
    }
    if let Some(report) = context.transform_report
        && report.shape_fallback_values > 0
    {
        review_items.push(format!(
            "{} value(s) did not match their column's detected format and were replaced with generic pseudonyms instead of format-preserving ones.",
            report.shape_fallback_values
        ));
    }
    if let Some(report) = context.transform_report
        && report.unchanged_sensitive_values > 0
    {
        review_items.push(format!(
            "{} selected high/medium-risk value(s) remained unchanged despite a transforming strategy; do not share this output.",
            report.unchanged_sensitive_values
        ));
    }
    if let Some(report) = context.transform_report
        && report.residual_audit_matches > 0
    {
        review_items.push(format!(
            "{} protected source value fingerprint(s) also occur somewhere in the released output; inspect it before sharing.",
            report.residual_audit_matches
        ));
    }
    if let Some(report) = context.transform_report
        && report.residual_audit_incomplete
    {
        review_items.push(
            "The broad residual-value audit reached its memory bound and is incomplete."
                .to_string(),
        );
    }
    // A review item rather than a blocker. Whether few distinct values matter depends
    // on what the column holds — a six-valued column may carry nothing sensitive — so
    // a measured heuristic should inform the reviewer, not refuse the release. Note
    // this cannot change the readiness status on its own: the "not a formal anonymity
    // guarantee" item below is unconditional, so the status is already Review.
    if let Some(report) = context.transform_report {
        let invertible = report
            .column_value_distributions
            .iter()
            .filter(|distribution| distribution.risks_frequency_inversion())
            .count();
        if invertible > 0 {
            // "Repeated few enough values" described only the distinct-count test. A
            // column flagged for one dominant value has not repeated few values — it
            // repeated *one* value often — so the summary states the property all three
            // tests establish and leaves the specific evidence to the per-column note.
            review_items.push(format!(
                "The value distribution of {invertible} pseudonymized column(s) is uneven enough that the mapping could be matched back by value frequency."
            ));
        }
    }

    // Always Review, never Verified, and written as a constant because it *is* one: the
    // caveat pushed above is unconditional, so `review_items.is_empty()` cannot hold. A
    // conditional here would read as a live decision that cannot go both ways, hiding the
    // stance it implements: this path does not certify, and a reader should not have to
    // prove that by tracing every push above. Blocked comes only from the preflight path in
    // service.rs.
    let status = ReleaseReadinessStatus::Review;

    ReleaseReadiness {
        status,
        blockers: Vec::new(),
        review_items,
        verified_items,
    }
}

pub(crate) fn build_evidence(
    columns: &[ColumnMetadata],
    context: &ReportContext<'_>,
) -> Vec<ReleaseEvidenceItem> {
    let mut evidence = Vec::new();
    let selected_count = columns.iter().filter(|column| column.is_selected).count();
    evidence.push(ReleaseEvidenceItem {
        id: "coverage".to_string(),
        label: "Column coverage".to_string(),
        status: if selected_count == columns.len() {
            ReleaseEvidenceStatus::Verified
        } else {
            ReleaseEvidenceStatus::Review
        },
        detail: format!(
            "{selected_count}/{} source column(s) selected for this workflow.",
            columns.len()
        ),
    });

    let unselected_risky = unselected_detector_risk_columns(columns);
    evidence.push(ReleaseEvidenceItem {
        id: "detector-risk".to_string(),
        label: "Detector risk review".to_string(),
        status: if unselected_risky.is_empty() {
            ReleaseEvidenceStatus::Verified
        } else {
            ReleaseEvidenceStatus::Review
        },
        detail: if unselected_risky.is_empty() {
            "No high/medium detector-risk column was left unchanged by selection.".to_string()
        } else {
            format!(
                "Review unselected high/medium detector-risk column(s): {}.",
                unselected_risky.join(", ")
            )
        },
    });

    // Immediately after the per-column verdict, because it is the one that qualifies it.
    if let Some(summary) = context
        .transform_report
        .and_then(|report| report.row_uniqueness.as_ref())
    {
        evidence.push(row_uniqueness_evidence(columns, summary));
    }

    if let Some(report) = context.transform_report
        && report.smart_replacement_requests > 0
    {
        evidence.push(ReleaseEvidenceItem {
            id: "local-ai-validation".to_string(),
            label: "Local AI validation".to_string(),
            status: if report.smart_replacement_rejections == 0
                && report.smart_replacement_fallbacks == 0
            {
                ReleaseEvidenceStatus::Verified
            } else {
                ReleaseEvidenceStatus::Review
            },
            detail: format!(
                "{} requested, {} accepted, {} rejected, {} fallback value(s).{}",
                report.smart_replacement_requests,
                report.smart_replacement_values,
                report.smart_replacement_rejections,
                report.smart_replacement_fallbacks,
                if report.smart_replacement_rejections > 0 {
                    format!(" Rejection reasons: {}.", smart_rejection_summary(report))
                } else {
                    String::new()
                }
            ),
        });
    }

    if let Some(report) = context.transform_report {
        if report.keyed_token_values > 0 {
            evidence.push(ReleaseEvidenceItem {
                id: "keyed-tokenization".to_string(),
                label: "Repeatable keyed tokenization".to_string(),
                status: ReleaseEvidenceStatus::Info,
                detail: format!(
                    "{} unique value(s) in column(s) {:?} used repeatable keyed tokens. The key is not included in this report. Reusing it makes those releases linkable; losing it prevents reproducing the tokens.",
                    report.keyed_token_values, report.keyed_token_columns
                ),
            });
        }
        evidence.push(ReleaseEvidenceItem {
            id: "residual-unchanged-values".to_string(),
            label: "Residual unchanged-value check".to_string(),
            status: if report.unchanged_sensitive_values == 0 {
                ReleaseEvidenceStatus::Verified
            } else {
                // The transform API still returns the artifact, so claiming it was
                // blocked would overstate enforcement. Readiness correctly remains
                // in review until a caller chooses not to publish it.
                ReleaseEvidenceStatus::Review
            },
            detail: if report.unchanged_sensitive_values == 0 {
                "No selected high/medium-risk value was returned unchanged by a strategy that claimed to transform it. This exact check does not detect unrelated or newly introduced identifiers."
                    .to_string()
            } else {
                format!(
                    "{} selected high/medium-risk value(s) were returned unchanged in column(s) {:?}.",
                    report.unchanged_sensitive_values, report.unchanged_sensitive_columns
                )
            },
        });

        evidence.push(ReleaseEvidenceItem {
            id: "broad-residual-value-audit".to_string(),
            label: "Broad residual-value audit".to_string(),
            status: if report.residual_audit_matches > 0 || report.residual_audit_incomplete {
                ReleaseEvidenceStatus::Review
            } else {
                ReleaseEvidenceStatus::Verified
            },
            detail: if report.residual_audit_incomplete {
                format!(
                    "The bounded audit reached its capacity after fingerprinting {} protected source value(s) and {} released value(s); its result is incomplete and must not be treated as a pass.",
                    report.residual_audit_source_values, report.residual_audit_output_values
                )
            } else if report.residual_audit_matches > 0 {
                format!(
                    "{} unique selected high/medium-risk source value fingerprint(s) also occur somewhere in the released output. Review the output before sharing.",
                    report.residual_audit_matches
                )
            } else {
                format!(
                    "No match was found between {} selected high/medium-risk source value fingerprint(s) and {} released value fingerprint(s). This broad comparison does not detect related, reformatted, or newly introduced identifiers.",
                    report.residual_audit_source_values, report.residual_audit_output_values
                )
            },
        });
    }

    evidence
}

pub(crate) fn build_column_reports(columns: &[ColumnMetadata]) -> Vec<ColumnReleaseReport> {
    columns
        .iter()
        .map(|column| {
            let (action, status, detail) = column_action(column);
            ColumnReleaseReport {
                column_index: column.index,
                column_name: column.name.clone(),
                selected: column.is_selected,
                detected_type: column.detected_type,
                pii_risk: column.pii_risk,
                strategy: column.strategy,
                action,
                status,
                detail,
            }
        })
        .collect()
}

pub(crate) fn build_utility_metrics(
    columns: &[ColumnMetadata],
    context: &ReportContext<'_>,
) -> Vec<UtilityMetric> {
    let mut metrics = Vec::new();
    let selected = columns.iter().filter(|column| column.is_selected).count();
    metrics.push(UtilityMetric {
        label: "Selected columns".to_string(),
        value: format!("{selected}/{}", columns.len()),
        status: if selected == columns.len() {
            ReleaseEvidenceStatus::Verified
        } else {
            ReleaseEvidenceStatus::Review
        },
        detail: Some("Columns outside selection are written unchanged.".to_string()),
    });

    if let Some(report) = context.transform_report {
        metrics.push(UtilityMetric {
            label: "Repeat reuse".to_string(),
            value: report.reused_pseudonym_values.to_string(),
            status: ReleaseEvidenceStatus::Info,
            detail: Some(
                "Repeated source values reused the same pseudonym/token within the run."
                    .to_string(),
            ),
        });
        metrics.push(UtilityMetric {
            label: "Local AI accepted".to_string(),
            value: format!(
                "{}/{}",
                report.smart_replacement_values, report.smart_replacement_requests
            ),
            status: if report.smart_replacement_rejections == 0 {
                ReleaseEvidenceStatus::Verified
            } else {
                ReleaseEvidenceStatus::Review
            },
            detail: Some(if report.smart_replacement_rejections > 0 {
                format!(
                    "Accepted structured Local AI replacements before rule-based fallback. Rejections: {}.",
                    smart_rejection_summary(report)
                )
            } else {
                "Accepted structured Local AI replacements before rule-based fallback.".to_string()
            }),
        });
    }

    metrics
}

pub(crate) fn standard_notes(
    columns: &[ColumnMetadata],
    transform_report: TransformReport,
    detection_coverage: DetectionCoverage,
) -> Vec<String> {
    let mut notes = vec![
        "Standard CSV transform changes selected cells in place with local strategies such as masking, redaction, tokenization, pseudonymization, pass-through, and optional Local AI replacement."
            .to_string(),
        "Treat this as risk reduction, not proof of anonymity; review the output against your sharing context and re-identification risk."
            .to_string(),
    ];
    push_detection_coverage_note(&mut notes, detection_coverage, columns);
    push_unselected_column_note(&mut notes, columns);

    if transform_report.unique_pseudonym_values > 0
        || transform_report.opaque_token_values > 0
        || columns.iter().any(|column| {
            column.is_selected
                && matches!(
                    column.strategy,
                    AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize
                )
                && !column.detected_type.uses_default_pass_through()
        })
    {
        notes.push(
            "Pseudonyms and tokens are tracked within each run so repeated source values stay consistent while distinct readable names avoid reuse while capacity remains."
                .to_string(),
        );
        // The sentence above describes consistency as the feature it is. It is also a
        // re-identification property, and saying only the first half is what the EDPB
        // anonymisation guidelines warn against: output that keeps records linkable is
        // pseudonymised, and pseudonymised data is still personal data. Naming that
        // costs nothing and stops the report implying more than it delivers.
        notes.push(
            "Because repeated values keep the same replacement, these columns are pseudonymized rather than anonymized: records stay linkable to each other, and the output remains personal data under GDPR. Redaction and masking do not preserve that link."
                .to_string(),
        );
    }

    let invertible: Vec<String> = transform_report
        .column_value_distributions
        .iter()
        .filter_map(|distribution| {
            let risk = distribution.frequency_inversion_risk()?;
            let name = columns
                .iter()
                .find(|column| column.index == distribution.column_index)
                .map(|column| column.name.as_str())
                .unwrap_or("(unnamed)");
            // These figures are exact rather than sampled — the ledger counted every
            // row — so unlike the pre-run warning this names no sample size.
            Some(match risk {
                // A distinct count would actively mislead here: a column of 101 values
                // where one covers half the rows renders as "101 distinct of 200
                // values", which is true, reads as reassuring, and describes a risk the
                // column does not have instead of the one it does.
                FrequencyInversionRisk::DominantValue { share } => format!(
                    "{name} (one value in {:.0}% of {} values)",
                    share * 100.0,
                    distribution.total_values
                ),
                FrequencyInversionRisk::FewDistinctValues
                | FrequencyInversionRisk::LargeGroups { .. } => format!(
                    "{name} ({} distinct of {} values)",
                    distribution.distinct_values, distribution.total_values
                ),
            })
        })
        .collect();
    if !invertible.is_empty() {
        notes.push(format!(
            "The replacement mapping for {} column(s) could be matched back by how often each value occurs: {}.",
            invertible.len(),
            invertible.join(", ")
        ));
    }
    if transform_report.collisions_avoided > 0 {
        notes.push(format!(
            "{} pseudonym candidate collision(s) were avoided by assigning unused alternatives.",
            transform_report.collisions_avoided
        ));
    }
    if transform_report.exhausted_pseudonym_pools > 0 {
        notes.push(format!(
            "{} pseudonym pool exhaustion event(s) used generated fallback values.",
            transform_report.exhausted_pseudonym_pools
        ));
    }
    if columns
        .iter()
        .any(|column| column.is_selected && column.strategy == AnonymizationStrategy::LocalAi)
    {
        notes.push(
            "Smart replacement used Local AI on this device to generate realistic replacement values; review outputs because this is not a formal anonymization guarantee."
                .to_string(),
        );
    }
    if columns
        .iter()
        .any(|column| column.is_selected && redaction_changes_structured_scalar_type(column))
    {
        notes.push(format!(
            "{STRUCTURED_SCALAR_REDACTION_WARNING} Use schema-preserving pseudonymization when downstream consumers require original scalar types."
        ));
    }
    if transform_report.smart_replacement_rejections > 0 {
        notes.push(format!(
            "{} smart replacement candidate(s) were rejected before fallback handling: {}.",
            transform_report.smart_replacement_rejections,
            smart_rejection_summary(&transform_report)
        ));
    }
    // Immediately after the aggregate count, which is the figure it explains.
    push_smart_replacement_leak_guard_note(&mut notes, columns, &transform_report);
    if transform_report.smart_replacement_fallbacks > 0 {
        notes.push(format!(
            "{} smart replacement value(s) fell back to rule-based pseudonymization after missing or invalid AI output.",
            transform_report.smart_replacement_fallbacks
        ));
    }
    if transform_report.shape_fallback_values > 0 {
        notes.push(format!(
            "{} value(s) did not match their column's detected format and were replaced with generic pseudonyms.",
            transform_report.shape_fallback_values
        ));
    }
    if columns.iter().any(|column| {
        column.is_selected
            && column.detected_type == DataType::Email
            && matches!(
                column.strategy,
                AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize
            )
    }) {
        notes.push(
            "Email pseudonymization keeps the original domain; use Redact or Tokenize when domains themselves are identifying (for example personal domains)."
                .to_string(),
        );
    }
    // The same disclosure the email note above makes, for the transform where the
    // surviving part is sharpest. A timestamp keeps its time of day byte for byte, so
    // in an event log the "anonymized" column joins back to the source on sub-second
    // precision alone — and unlike the email domain, nothing on screen makes that
    // visible unless the reader compares two microsecond suffixes by eye.
    if columns.iter().any(|column| {
        column.is_selected
            && column.detected_type == DataType::Timestamp
            && matches!(
                column.strategy,
                AnonymizationStrategy::Auto
                    | AnonymizationStrategy::Pseudonymize
                    | AnonymizationStrategy::LocalAi
            )
    }) {
        notes.push(
            "Timestamp pseudonymization shifts only the date, by at most a year, and copies the time of day through unchanged including any sub-second digits. Event times therefore remain a near-unique join key back to the source, and a date of birth keeps its year to within one. Use Redact or Tokenize when the time itself is identifying."
                .to_string(),
        );
    }
    if columns
        .iter()
        .any(|column| column.is_selected && column.strategy == AnonymizationStrategy::Mask)
    {
        notes.push(MASK_STRUCTURE_DISCLOSURE.to_string());
    }

    notes
}

fn column_action(column: &ColumnMetadata) -> (String, ReleaseEvidenceStatus, String) {
    if !column.is_selected {
        return (
            "Unselected".to_string(),
            if column.pii_risk.is_elevated() {
                ReleaseEvidenceStatus::Review
            } else {
                ReleaseEvidenceStatus::Info
            },
            "Column was outside the selected release set.".to_string(),
        );
    }

    match column.strategy {
        AnonymizationStrategy::Mask => (
            "Masked".to_string(),
            // Review, not Verified, for the same reason Label is: the output keeps a
            // structural fingerprint of the source, and whether that is acceptable for
            // a given release is a judgement a reader has to make. See
            // [`MASK_STRUCTURE_DISCLOSURE`].
            ReleaseEvidenceStatus::Review,
            MASK_STRUCTURE_DISCLOSURE.to_string(),
        ),
        AnonymizationStrategy::Redact => (
            "Redacted".to_string(),
            ReleaseEvidenceStatus::Verified,
            "Selected values are replaced with typed placeholders.".to_string(),
        ),
        AnonymizationStrategy::Tokenize => (
            "Tokenized".to_string(),
            ReleaseEvidenceStatus::Verified,
            "Selected values become opaque tokens that stay consistent within the run.".to_string(),
        ),
        AnonymizationStrategy::Label => (
            "Labelled".to_string(),
            // Review, not Verified: the output is readable and re-linkable by design,
            // so it is pseudonymised rather than anonymous. A reader has to decide
            // whether that is acceptable for the release, which is the definition of
            // a review item.
            ReleaseEvidenceStatus::Review,
            "Selected values become column-named placeholders that stay consistent within the run, which keeps repeated values linkable.".to_string(),
        ),
        AnonymizationStrategy::LocalAi => (
            "Smart replacement".to_string(),
            ReleaseEvidenceStatus::Review,
            // True only because of the pass-through exemption in
            // `strategies::transform_value_with_state`: without it, a rejected candidate
            // on a column whose detected type defaults to pass-through would be written
            // out unchanged and this sentence would promise a guarantee the code does not
            // provide.
            //
            // The structure clause names what the *fallback* keeps, and the fallback is
            // the rule-based transformer for the detected type — so a pass-through type,
            // which has no transformer of its own and lands on generic-string
            // pseudonymization, is described by that wording rather than by its own
            // (empty) one.
            format!(
                "Local AI generated realistic replacements; every rejected candidate fell back to rule-based replacement rather than the original value.{}",
                match local_ai_fallback_structure(column.detected_type) {
                    Some(structure) => format!(" Where the fallback applied, {structure}."),
                    None => String::new(),
                }
            ),
        ),
        AnonymizationStrategy::PassThrough => (
            "Pass-through".to_string(),
            ReleaseEvidenceStatus::Review,
            "Selected values are intentionally kept unchanged.".to_string(),
        ),
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            if column.detected_type.uses_default_pass_through() {
                (
                    "No-op/pass-through".to_string(),
                    ReleaseEvidenceStatus::Review,
                    "This detected type currently keeps values unchanged under Auto/Pseudonymize.".to_string(),
                )
            } else {
                match column.detected_type.pseudonymization_preserves_structure() {
                    // Review, not Verified. The transform is format-preserving by
                    // design — a pseudonymized timestamp still has to parse — but the
                    // preserved part is source data in the output, and a reader has to
                    // decide whether that is acceptable for the release. Calling it
                    // Verified while saying nothing about what came through intact is
                    // what let a microsecond time-of-day, a full email domain and an
                    // exact digit count be published under a green tick.
                    Some(structure) => (
                        "Pseudonymized".to_string(),
                        ReleaseEvidenceStatus::Review,
                        format!(
                            "Selected values use rule-based replacement, which preserves structure: {structure}."
                        ),
                    ),
                    None => (
                        "Pseudonymized".to_string(),
                        ReleaseEvidenceStatus::Verified,
                        "Selected values use rule-based replacement, which keeps nothing of the original value.".to_string(),
                    ),
                }
            }
        }
    }
}

/// What a rejected Local AI candidate's rule-based fallback keeps of the original.
///
/// Differs from [`DataType::pseudonymization_preserves_structure`] on exactly the
/// pass-through types. Those answer `None` there because Auto and Pseudonymize return
/// them unchanged and there is no transform to describe — but the Local AI fallback
/// deliberately bypasses that gate (see `strategies::transform_value_with_state`), so
/// for this strategy they do get transformed, by the generic-string path.
fn local_ai_fallback_structure(detected_type: DataType) -> Option<&'static str> {
    if detected_type.uses_default_pass_through() {
        return Some(GENERIC_STRING_STRUCTURE_DISCLOSURE);
    }
    detected_type.pseudonymization_preserves_structure()
}

fn smart_rejection_summary(report: &TransformReport) -> String {
    if report.smart_replacement_rejection_reasons.is_empty() {
        return "reason details unavailable".to_string();
    }

    report
        .smart_replacement_rejection_reasons
        .iter()
        .map(|entry| {
            format!(
                "{} {}",
                entry.count,
                smart_rejection_reason_label(entry.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn smart_rejection_reason_label(
    reason: crate::types::SmartReplacementRejectionReason,
) -> &'static str {
    match reason {
        crate::types::SmartReplacementRejectionReason::UnexpectedOriginal => "unexpected source",
        crate::types::SmartReplacementRejectionReason::MissingOutput => "missing output",
        crate::types::SmartReplacementRejectionReason::EmptyOutput => "empty output",
        crate::types::SmartReplacementRejectionReason::SameAsOriginal => "copied source",
        crate::types::SmartReplacementRejectionReason::ContainsOriginal => "source text included",
        crate::types::SmartReplacementRejectionReason::MatchesOtherOriginal => {
            "another row's source value"
        }
        crate::types::SmartReplacementRejectionReason::ControlCharacter => "control character",
        crate::types::SmartReplacementRejectionReason::DuplicateOriginal => "duplicate source",
        crate::types::SmartReplacementRejectionReason::DuplicateOutput => "duplicate output",
    }
}

fn unselected_detector_risk_columns(columns: &[ColumnMetadata]) -> Vec<String> {
    columns
        .iter()
        .filter(|column| !column.is_selected && column.pii_risk.is_elevated())
        .map(|column| column.name.clone())
        .collect()
}

#[cfg(test)]
mod tests;
