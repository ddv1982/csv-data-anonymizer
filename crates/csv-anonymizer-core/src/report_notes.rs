use crate::types::{
    AnonymizationStrategy, ColumnMetadata, DetectionCoverage, FrequencyInversionRisk,
    SmartReplacementRejectionReason, TransformReport,
};

/// Says the detected types rest on a sample, when they do — `None` when they do not.
///
/// The one wording of this disclosure, said before the run by
/// `service::preflight::add_detection_coverage_review` and after it by
/// [`push_detection_coverage_note`], about the same evidence. Two wordings existed and
/// stated the same limitation with different force, so a reader who acted on the
/// milder one acted on less than the app knew; either could also be softened alone,
/// leaving the run's own report claiming a firmer basis than the check that cleared
/// it. Same reason [`crate::strategies::MASK_STRUCTURE_DISCLOSURE`] is one constant
/// for its two call sites.
///
/// Present tense throughout, because the same sentence has to be true in both places:
/// it describes how detection behaves, not what one run did.
///
/// Silent on a fully classified input, so a small file's report says nothing about
/// sampling — a caveat that never applies is noise that trains people to skip the
/// notes that do.
///
/// The consequence sentence is chosen from `columns`, not asserted. "Stays
/// unselected" describes a column the user did not select, so it would be false on
/// a run where every column *was* selected; the residual risk there is a misread type
/// choosing the wrong strategy, which is the other branch. Gated the same way
/// [`push_unselected_column_note`] is, on unselected columns existing.
pub(crate) fn detection_coverage_disclosure(
    coverage: DetectionCoverage,
    columns: &[ColumnMetadata],
) -> Option<String> {
    if !coverage.is_partial() {
        return None;
    }

    let examined = coverage.examined();
    let total = coverage.total();
    let noun = coverage.unit().plural_noun();
    let has_unselected = columns.iter().any(|column| !column.is_selected);

    Some(if has_unselected {
        format!(
            "Detection examined {examined} of {total} {noun}. A column whose sensitive values all sit in unexamined {noun} shows low risk and stays unselected on evidence that never saw them. Raise \"Sample rows\" to widen detection, or select such columns manually."
        )
    } else {
        format!(
            "Detection examined {examined} of {total} {noun}. Every column is selected, so none is left out on this evidence, but a column whose type is misread from the examined {noun} is transformed with the strategy that wrong type implies. Raise \"Sample rows\" to widen detection."
        )
    })
}

/// The coverage disclosure as a note on a finished run's report.
///
/// Placed with the unselected-column note because the two describe the same risk
/// from opposite ends: that one says which columns the user chose to leave
/// unchanged, this one says the choice was offered on partial evidence. A column
/// whose only sensitive values sat in unexamined rows is reported as low risk and
/// left unselected, and neither note alone would show it.
pub(crate) fn push_detection_coverage_note(
    notes: &mut Vec<String>,
    coverage: DetectionCoverage,
    columns: &[ColumnMetadata],
) {
    notes.extend(detection_coverage_disclosure(coverage, columns));
}

pub(crate) fn push_unselected_column_note(notes: &mut Vec<String>, columns: &[ColumnMetadata]) {
    let unselected_columns = columns.iter().filter(|column| !column.is_selected).count();
    if unselected_columns == 0 {
        return;
    }

    let unselected_detector_risk_columns = columns
        .iter()
        .filter(|column| !column.is_selected && column.pii_risk.is_elevated())
        .count();
    if unselected_detector_risk_columns > 0 {
        notes.push(format!(
            "{} unselected high/medium detector-risk {} written unchanged.",
            unselected_detector_risk_columns,
            plural(
                unselected_detector_risk_columns,
                "column was",
                "columns were"
            )
        ));
    } else {
        notes.push(format!(
            "{} unselected {} written unchanged.",
            unselected_columns,
            plural(unselected_columns, "column was", "columns were")
        ));
    }
}

/// Share of a run's Local AI candidates the cross-value leak guard must have
/// rejected before the run is described as having fallen back wholesale.
///
/// Not a calibrated statistic — there is no corpus of rejection rates to calibrate
/// against — but a reading of the sentence it gates. The note tells the user that
/// Local AI "appeared to do nothing", so it should only appear when that is what the
/// output looks like: at half the candidates rejected, half the column is still the
/// model's readable output and the user can see Local AI working. Set at the point
/// where the fallback, not the model, is what the reader is looking at.
const WHOLESALE_SMART_REJECTION_SHARE: f64 = 0.5;

/// Explains a Local AI column whose replacements the cross-value leak guard refused
/// almost entirely, and says what actually happened to those values.
///
/// The guard doing this is correct: a replacement that exactly equals another row's
/// real value re-emits real data, so it is refused. But for a closed-domain column —
/// country, gender, city, month, status — every realistic replacement *is* another
/// row's real value, so essentially every candidate is refused and the whole column
/// lands on the fallback. The user sees Local AI apparently do nothing, and the only
/// trace was an aggregate rejection count buried further down the report. That looks
/// like the model failing when it is the guard succeeding, and a user who reads it
/// as a failure re-runs or switches the column to pass-through.
///
/// Every refused value *is* replaced, and this note says so rather than leaving the
/// reader to infer it from a bare rejection count.
///
/// The shared pass-through gate in `strategies::transform_value_with_state` must keep
/// exempting the Local AI fallback: without that exemption, a column whose detected type
/// is one `DataType::uses_default_pass_through` covers — `Enum`, `CountryCode`,
/// `Boolean`, `Currency`, `Percentage` — would write the original value out verbatim,
/// and those are exactly the closed domains that provoke the wholesale rejection. A
/// refused value takes the generic-string transformer instead. The user-visible symptom
/// is still that Local AI appears to have done nothing, and a user who reads that as a
/// failure re-runs or switches the column to pass-through — which would reintroduce by
/// hand the leak the guard prevented.
///
/// Distribution figures are quoted only where the transform ledger measured one —
/// [`FrequencyInversionRisk::FewDistinctValues`] is the project's calibrated
/// definition of a small closed set.
///
/// What this cannot do is attribute rejections to a column. `TransformReport` counts
/// them per run and per reason, never per column, so with two Local AI columns there
/// is no way to tell which was refused. The note says so rather than implying
/// otherwise.
pub(crate) fn push_smart_replacement_leak_guard_note(
    notes: &mut Vec<String>,
    columns: &[ColumnMetadata],
    report: &TransformReport,
) {
    if report.smart_replacement_requests == 0 {
        return;
    }
    let leak_guard_rejections = report
        .smart_replacement_rejection_reasons
        .iter()
        .filter(|entry| entry.reason == SmartReplacementRejectionReason::MatchesOtherOriginal)
        .map(|entry| entry.count)
        .sum::<usize>();
    // The leak guard specifically, not every rejection reason. Missing output or a
    // control character is a model or transport problem with a different remedy, and
    // attributing those to a closed value domain would send the user looking for a
    // cause their data does not have.
    if (leak_guard_rejections as f64)
        < WHOLESALE_SMART_REJECTION_SHARE * report.smart_replacement_requests as f64
    {
        return;
    }

    let smart_columns: Vec<&ColumnMetadata> = columns
        .iter()
        .filter(|column| column.is_selected && column.strategy == AnonymizationStrategy::LocalAi)
        .collect();

    let mut note = format!(
        "{leak_guard_rejections} of {} Local AI replacement candidate(s) in this run were refused because the suggested value exactly matched another row's real value. The usual cause is a column holding a small closed set of repeated values — country, gender, city, status — where any realistic replacement for one row is another row's real value. Rejections are counted per run rather than per column, so the columns below are this run's Local AI columns, not necessarily the refused ones.",
        report.smart_replacement_requests
    );
    if !smart_columns.is_empty() {
        note.push_str(&format!(
            " Refused values in {} fell back to rule-based replacement — they are replaced, never written through — which is why Local AI appears to have left these columns as unreadable pseudonyms rather than realistic values. Choose Redact, Tokenize or Pseudonymize deliberately if that is not the output you want; do not switch these columns to pass-through, which would publish the values the guard just refused.",
            column_list(&smart_columns, report)
        ));
    }
    notes.push(note);
}

/// Column names, each with its measured distribution where the ledger recorded a
/// small closed set — the evidence for the cause the note names.
fn column_list(columns: &[&ColumnMetadata], report: &TransformReport) -> String {
    columns
        .iter()
        .map(|column| {
            let closed_set = report
                .column_value_distributions
                .iter()
                .find(|distribution| distribution.column_index == column.index)
                .filter(|distribution| {
                    matches!(
                        distribution.frequency_inversion_risk(),
                        Some(FrequencyInversionRisk::FewDistinctValues)
                    )
                });
            match closed_set {
                Some(distribution) => format!(
                    "{} ({} distinct of {} values)",
                    column.name, distribution.distinct_values, distribution.total_values
                ),
                None => column.name.clone(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
