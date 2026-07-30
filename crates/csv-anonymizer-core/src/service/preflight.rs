use super::controls::keeps_consistent_mapping;
use super::controls::select_columns_reporting_errors;
use super::{ensure_output_differs_from_input, validate_output_path};
use crate::csv_io::count_csv_data_rows;
use crate::error::Result;
use crate::release_report::{ReportContext, build_column_reports, build_evidence, build_readiness};
use crate::report_notes::detection_coverage_disclosure;
use crate::smart::{
    SmartReplacementMap, has_smart_replacement_columns, missing_smart_replacement_values_from_csv,
    reusable_preview_smart_replacements,
};
use crate::strategies::TransformState;
use crate::types::{
    ColumnMetadata, ColumnValueDistribution, DetectionCoverage, PreflightData, PreflightMode,
    PreflightParams, ReleaseEvidenceItem, ReleaseEvidenceStatus, ReleaseReadiness,
    ReleaseReadinessStatus,
};
use std::path::Path;

pub(super) fn run_preflight(
    file_path: &Path,
    metadata: Vec<ColumnMetadata>,
    input: PreflightParams,
    detection_coverage: DetectionCoverage,
) -> Result<PreflightData> {
    let mut state = PreflightState::new(file_path, metadata.len());
    let selected_metadata = selected_preflight_metadata(&metadata, &input, &mut state);
    let selected_smart_columns = has_smart_replacement_columns(&selected_metadata);
    // The same gate the run applies, so preflight cannot promise that a run needs no
    // Local AI and then have the run ask for it.
    let existing_smart_replacements =
        reusable_preview_smart_replacements(&input.preview_smart_replacements, &selected_metadata);

    state.verified_items.push(
        "Replacements are randomized per run with in-run reuse for repeated source values."
            .to_string(),
    );
    add_preflight_output_evidence(&input, &mut state);

    let local_ai_required = local_ai_required_for_preflight(
        file_path,
        &input,
        &selected_metadata,
        existing_smart_replacements.as_ref(),
        selected_smart_columns,
        &mut state,
    );
    add_preflight_local_ai_evidence(
        &input,
        selected_smart_columns,
        local_ai_required,
        &mut state,
    );
    add_detection_coverage_review(detection_coverage, &selected_metadata, &mut state);
    add_mapping_memory_review(file_path, &input, &selected_metadata, &mut state);
    add_release_readiness_evidence(&selected_metadata, &mut state);

    let (readiness, evidence) = state.into_readiness_and_evidence();
    Ok(PreflightData {
        mode: input.mode,
        readiness,
        evidence,
        column_reports: build_column_reports(&selected_metadata),
    })
}

struct PreflightState {
    blockers: Vec<String>,
    review_items: Vec<String>,
    verified_items: Vec<String>,
    evidence: Vec<ReleaseEvidenceItem>,
}

impl PreflightState {
    fn new(file_path: &Path, column_count: usize) -> Self {
        Self {
            blockers: Vec::new(),
            review_items: Vec::new(),
            verified_items: vec![
                "Input file is readable.".to_string(),
                format!("{column_count} column(s) analyzed."),
            ],
            evidence: vec![ReleaseEvidenceItem {
                id: "input-file".to_string(),
                label: "Input file".to_string(),
                status: ReleaseEvidenceStatus::Verified,
                detail: format!("Read metadata from {}.", file_path.display()),
            }],
        }
    }

    fn into_readiness_and_evidence(self) -> (ReleaseReadiness, Vec<ReleaseEvidenceItem>) {
        (
            finish_readiness(self.blockers, self.review_items, self.verified_items),
            self.evidence,
        )
    }
}

/// The same selection the run will make, with each step's failure recorded as a
/// blocker instead of ending the check.
///
/// Preflight has to report every problem at once — the user fixes what the panel
/// lists — so it cannot stop at the first failing step. What it must not do is judge a
/// *different* selection from the one the run would act on, which is why the steps
/// themselves come from [`select_columns_reporting_errors`] rather than being repeated
/// here: a preflight that applied controls differently would clear a run whose columns
/// are not the ones it examined.
fn selected_preflight_metadata(
    metadata: &[ColumnMetadata],
    input: &PreflightParams,
    state: &mut PreflightState,
) -> Vec<ColumnMetadata> {
    if input.columns.is_empty() {
        state
            .blockers
            .push("Select at least one column to transform or release.".to_string());
    }

    let selection = select_columns_reporting_errors(metadata, &input.columns, &input.controls);
    match selection.index_error {
        Some(error) => state.blockers.push(error.to_string()),
        None => {
            if !input.columns.is_empty() {
                state
                    .verified_items
                    .push(format!("{} column(s) selected.", input.columns.len()));
            }
        }
    }
    if let Some(error) = selection.control_error {
        state.blockers.push(error.to_string());
    }

    selection.metadata
}

fn add_preflight_output_evidence(input: &PreflightParams, state: &mut PreflightState) {
    match input.mode {
        PreflightMode::Preview => state
            .verified_items
            .push("Preview does not require an output path.".to_string()),
        PreflightMode::Anonymize => match input.output_path.as_ref() {
            Some(output_path) => {
                match ensure_output_differs_from_input(&input.file_path, output_path)
                    .and_then(|()| validate_output_path(output_path, input.force))
                {
                    Ok(path) => {
                        state
                            .verified_items
                            .push("Output path is writable.".to_string());
                        state.evidence.push(ReleaseEvidenceItem {
                            id: "output-path".to_string(),
                            label: "Output path".to_string(),
                            status: ReleaseEvidenceStatus::Verified,
                            detail: format!("Output can be written to {}.", path.display()),
                        });
                    }
                    Err(error) => {
                        state.blockers.push(error.to_string());
                        state.evidence.push(ReleaseEvidenceItem {
                            id: "output-path".to_string(),
                            label: "Output path".to_string(),
                            status: ReleaseEvidenceStatus::Blocked,
                            detail: error.to_string(),
                        });
                    }
                }
            }
            None => state.blockers.push("Choose an output path.".to_string()),
        },
    }
}

fn local_ai_required_for_preflight(
    file_path: &Path,
    input: &PreflightParams,
    selected_metadata: &[ColumnMetadata],
    existing_smart_replacements: Option<&SmartReplacementMap>,
    selected_smart_columns: bool,
    state: &mut PreflightState,
) -> bool {
    if !selected_smart_columns {
        return false;
    }

    match input.mode {
        PreflightMode::Preview => true,
        PreflightMode::Anonymize => match missing_smart_replacement_values_from_csv(
            file_path,
            selected_metadata,
            existing_smart_replacements,
        ) {
            Ok(has_missing_values) => has_missing_values,
            Err(error) => {
                state.blockers.push(error.to_string());
                true
            }
        },
    }
}

fn add_preflight_local_ai_evidence(
    input: &PreflightParams,
    selected_smart_columns: bool,
    local_ai_required: bool,
    state: &mut PreflightState,
) {
    if local_ai_required {
        if input.local_ai_ready {
            state
                .verified_items
                .push("Local AI is ready for Smart replacement columns.".to_string());
            state.evidence.push(ReleaseEvidenceItem {
                id: "local-ai".to_string(),
                label: "Local AI".to_string(),
                status: ReleaseEvidenceStatus::Verified,
                detail: input.local_ai_message.clone().unwrap_or_else(|| {
                    "Local AI is ready for selected Smart replacement columns.".to_string()
                }),
            });
        } else {
            let message = input.local_ai_message.clone().unwrap_or_else(|| {
                "Local AI is not ready for selected Smart replacement columns.".to_string()
            });
            state.blockers.push(message.clone());
            state.evidence.push(ReleaseEvidenceItem {
                id: "local-ai".to_string(),
                label: "Local AI".to_string(),
                status: ReleaseEvidenceStatus::Blocked,
                detail: message,
            });
        }
    } else if selected_smart_columns {
        state
            .verified_items
            .push("Preview Smart replacements cover selected Smart columns.".to_string());
    } else {
        state
            .verified_items
            .push("No selected column requires Local AI.".to_string());
    }
    state
        .verified_items
        .push("Transform settings passed backend validation.".to_string());
}

/// Projected mapping entries at or above which the run is worth telling the user
/// about before it starts.
///
/// 3,000,000 entries is about 480 MB at
/// `TransformState::APPROXIMATE_BYTES_PER_MAPPING_ENTRY`, and it is exactly the run
/// the README's memory table measures at the top of its range: one all-distinct
/// column of 1,000,000 rows on Pseudonymize, measured at 477 MiB peak RSS against an
/// 11 MiB floor. So the threshold is set at the smallest run this project has measured
/// where the mapping is the dominant term in the process's memory rather than a
/// rounding error on top of the streaming floor.
///
/// The other end of the interval the measurements support: the same table's 250,000
/// distinct values — 750,000 entries, 127 MiB — is a run nobody needs to be told
/// about, so the threshold belongs somewhere in (750,000, 3,000,000]. 3,000,000 is the
/// quiet end of that interval, chosen because this item competes for attention with
/// blockers in the same panel.
///
/// An order of magnitude below `TransformState::MAPPING_ENTRY_CEILING`, so a run that
/// would be refused mid-way is always reported here first.
///
/// Not tested: whether users act on the item, and inputs between 3,000,000 and
/// 12,000,000 entries, where the projection has been reasoned about but not measured
/// end to end.
const MAPPING_MEMORY_REVIEW_ENTRIES: usize = 3_000_000;

/// Says the run is about to act on types detected from part of the file.
///
/// A review item rather than a blocker, for the same reason the mapping-memory
/// projection is one: sampling is how this app reads inputs of any size, every
/// large-file run relies on it, and refusing them would remove the feature rather
/// than inform the decision. What the user can act on is the sample size, so the
/// item names it.
///
/// The wording is [`detection_coverage_disclosure`], the same sentence the finished
/// run's report carries. Pre-run and post-run are the only difference between the two
/// moments, and the limitation is identical, so stating it twice in two voices only
/// created the chance of stating it with two different strengths.
///
/// Silent when the sample covered everything, so the common small-file run is not
/// given a caveat that does not apply to it — and says so as a verified item, which
/// the report has no equivalent of.
fn add_detection_coverage_review(
    coverage: DetectionCoverage,
    selected_metadata: &[ColumnMetadata],
    state: &mut PreflightState,
) {
    let Some(disclosure) = detection_coverage_disclosure(coverage, selected_metadata) else {
        state
            .verified_items
            .push("Every row was examined for detection.".to_string());
        return;
    };

    let examined = coverage.examined();
    let total = coverage.total();
    state.review_items.push(disclosure);
    state.evidence.push(ReleaseEvidenceItem {
        id: "detection-coverage".to_string(),
        label: "Detection coverage".to_string(),
        status: ReleaseEvidenceStatus::Review,
        // "data row(s)" rather than the unit noun: preflight has exactly one caller,
        // `preflight_anonymization`, which takes a file path, so the unit here is
        // always rows. A field-based paste never reaches this item.
        detail: format!("Types were detected from {examined} of {total} data row(s)."),
    });
}

/// Projects peak mapping memory before the run, and reports it if it is large.
///
/// The point of doing this in preflight is that it is the last moment the user can
/// still change strategy: the same figure discovered during the run is a refusal, and
/// the same figure discovered by the operating system is an OOM kill with no message.
///
/// Only for `Anonymize`. A preview transforms the handful of rows it displays, so its
/// mapping holds a few dozen entries whatever the file's cardinality, and projecting
/// the whole file's mapping onto it would report memory a preview never allocates.
///
/// A review item rather than a blocker, deliberately. The projection is an upper bound
/// drawn from a sample of about a hundred values per column — see
/// [`projected_distinct_values`] — and it can exceed the truth by two orders of
/// magnitude on a column whose values repeat in a way a sample that size cannot see.
/// Blocking on that would refuse runs that would have finished in a few hundred MB.
/// The run itself refuses on measured entries, not on this estimate.
fn add_mapping_memory_review(
    file_path: &Path,
    input: &PreflightParams,
    selected_metadata: &[ColumnMetadata],
    state: &mut PreflightState,
) {
    if input.mode != PreflightMode::Anonymize {
        return;
    }
    let mapping_columns = selected_metadata
        .iter()
        .filter(|column| column.is_selected && keeps_consistent_mapping(column))
        .collect::<Vec<_>>();
    if mapping_columns.is_empty() {
        return;
    }

    // Counted rather than taken from the detection sample, because the projection
    // scales per-column cardinality by the file's real size and the sample's size
    // says nothing about that. Behind the emptiness check above so a run made
    // entirely of Redact and Mask columns does not pay for a pass it cannot use.
    //
    // A count that fails is left to the run to report: this projection is not the
    // place to introduce a second, differently worded file error, and the detection
    // pass that has already succeeded is the evidence that the file is readable.
    let Ok(row_count) = count_csv_data_rows(file_path) else {
        return;
    };

    let mut projected_entries: usize = 0;
    let mut largest: Option<(&str, usize)> = None;
    for column in mapping_columns {
        let entries = projected_distinct_values(column.sample_value_distribution, row_count)
            .saturating_mul(TransformState::mapping_entries_per_distinct_value(
                column.strategy,
            ));
        projected_entries = projected_entries.saturating_add(entries);
        if largest.is_none_or(|(_, most)| entries > most) {
            largest = Some((column.name.as_str(), entries));
        }
    }

    if projected_entries < MAPPING_MEMORY_REVIEW_ENTRIES {
        return;
    }
    state.review_items.push(mapping_memory_review_message(
        projected_entries,
        row_count,
        largest.map(|(name, _)| name),
    ));
}

/// Names the figure, where most of it comes from, and what to do about it.
///
/// Says "up to" in the first clause and repeats the reason in the second, because a
/// projection stated as a flat figure invites the reader to plan around a number that
/// can be a hundred times the truth. The remedy names Redact and Mask rather than
/// saying "select fewer columns" first: dropping a column loses the protection,
/// whereas moving it to a strategy that keeps no mapping loses only the linkage.
fn mapping_memory_review_message(
    projected_entries: usize,
    row_count: usize,
    largest_column: Option<&str>,
) -> String {
    let megabytes = TransformState::approximate_mapping_megabytes(projected_entries);
    let mut message = format!(
        "Keeping repeated values linkable could hold up to {projected_entries} mapping entries \
         (about {megabytes} MB) across {row_count} row(s)."
    );
    if let Some(column_name) = largest_column {
        message.push_str(&format!(" Most of it in {column_name}."));
    }
    message.push_str(
        " Projected from the detection sample, so it is an upper bound rather than a measurement.",
    );
    // A refusal rather than a risk, because the run loop in `crate::csv_io` calls
    // `TransformState::check_mapping_budget` once per row. Kept conditional on the
    // projection clearing the ceiling: below it the run is expected to finish, and
    // naming a refusal that will not happen is how a warning stops being read.
    if projected_entries > TransformState::MAPPING_ENTRY_CEILING {
        message.push_str(&format!(
            " That is past the {}-entry ceiling this transform is sized for, so a run whose \
             values really are this varied will stop part-way through with no output written, \
             rather than running the machine out of memory.",
            TransformState::MAPPING_ENTRY_CEILING
        ));
    }
    message.push_str(
        " Redact and Mask hold no mapping and stay flat at any number of distinct values.",
    );
    message
}

/// Distinct values a column is projected to hold over `row_count` rows, from the
/// distribution of its detection sample.
///
/// Two terms, because a sample answers this well at one end and badly at the other:
///
/// - Chao1 over the sample — `distinct + f1² / 2·f2` — estimates how many groups the
///   column has, including groups the sample missed. It is exact for a column whose
///   values repeat often enough that the sample saturates (a 4-value status column
///   projects 4), and it is a *lower bound*, so on a column of a million distinct
///   values a hundred-value sample yields about 5,000. Shared with the cardinality
///   warning through `ColumnValueDistribution::estimated_distinct_values` rather than
///   reproduced here: two copies of an estimator are two things to keep in agreement.
/// - The singleton share is Good–Turing's estimate of how much of the column sits in
///   groups the sample never saw. Counting each of those groups as if it occurred once
///   — the worst case for memory — turns that share into `row_count × f1 / n` further
///   distinct values, which is what carries the all-distinct column from 5,000 up to
///   the row count where it belongs.
///
/// Exact at both ends by construction: a saturated sample has no singletons, so the
/// second term vanishes and Chao1 stands alone; an all-singleton sample makes the
/// second term the whole row count, which the cap then confirms. In between it
/// over-projects, and that is the deliberate direction — a projection that under-states
/// the memory is worse than useless, because the run it should have warned about is the
/// run that fails. A column of a thousand values spread over a million rows samples as
/// almost all singletons and is projected near a million distinct: about 900 times its
/// true cardinality. That is the honest limit of a hundred-value sample, which is why
/// the message says "up to" and the item does not block.
///
/// Capped at `row_count` because a column cannot hold more distinct values than rows,
/// and short-circuited when the sample covered the whole column, where the observed
/// count is a measurement and needs no estimator at all.
fn projected_distinct_values(distribution: ColumnValueDistribution, row_count: usize) -> usize {
    if distribution.total_values == 0 {
        return 0;
    }
    if distribution.total_values >= row_count {
        return distribution.distinct_values;
    }

    let estimated = distribution.estimated_distinct_values();
    let unseen_share = (distribution.singleton_values as f64) / (distribution.total_values as f64);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a share of a row count is non-negative and below it, and the result is capped \
                  at the row count immediately below"
    )]
    let projected_unseen = ((row_count as f64) * unseen_share) as usize;
    estimated.saturating_add(projected_unseen).min(row_count)
}

fn add_release_readiness_evidence(
    selected_metadata: &[ColumnMetadata],
    state: &mut PreflightState,
) {
    let context = ReportContext::default();
    let release_readiness = build_readiness(selected_metadata, &context);
    state.blockers.extend(release_readiness.blockers);
    state.review_items.extend(release_readiness.review_items);
    state
        .verified_items
        .extend(release_readiness.verified_items);
    state
        .evidence
        .extend(build_evidence(selected_metadata, &context));
}

fn finish_readiness(
    blockers: Vec<String>,
    review_items: Vec<String>,
    verified_items: Vec<String>,
) -> ReleaseReadiness {
    let status = if !blockers.is_empty() {
        ReleaseReadinessStatus::Blocked
    } else if !review_items.is_empty() {
        ReleaseReadinessStatus::Review
    } else {
        ReleaseReadinessStatus::Verified
    };

    ReleaseReadiness {
        status,
        blockers,
        review_items,
        verified_items,
    }
}
