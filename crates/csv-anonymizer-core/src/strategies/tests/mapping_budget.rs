//! What the transform state's mapping costs, and what happens when it gets too big.

use super::*;
use crate::error::AnonymizerError;
use crate::service::AnonymizerService;
use crate::types::{
    AnonymizeParams, ColumnControl, PreflightMode, PreflightParams, ReleaseReadiness,
    ReleaseReadinessStatus,
};
use std::path::Path;

/// A value with no meaning of its own: every assertion here turns on how many entries
/// a strategy records for it, never on what it contains.
const VALUE: &str = "Alice Smith";

/// What a run holds and what the projection predicts it will hold have to agree.
///
/// `TransformState::mapping_entries_per_distinct_value` is a hand-written table of the
/// transform paths, read by the preflight projection, and nothing in the type system
/// ties it to what those paths actually do. The direction that matters is a strategy
/// predicted to cost *fewer* entries than it records: the projection then under-states
/// the memory of exactly the run that goes on to hit the ceiling, which is the failure
/// the projection exists to prevent.
///
/// Walked over every strategy rather than the interesting ones, because the mistake
/// this catches is made when a strategy is added — and a new variant is precisely the
/// one nobody has cross-checked. `DataType::String` for all of them so that Auto and
/// Pseudonymize reach a transformer instead of the pass-through short circuit, which
/// records nothing for a reason of its own and would make four strategies agree at zero.
///
/// The state is left unseeded so `LocalAi` takes its fallback, which is the expensive
/// of its two paths and the one the table reports. Its cheap path is
/// [`a_smart_replacement_hit_costs_less_than_the_projection_assumes`].
#[test]
fn every_strategy_records_the_entries_the_projection_predicts() {
    for strategy in all_strategies() {
        let mut subject = column(DataType::String);
        subject.strategy = strategy;

        let mut state = TransformState::new();
        transform_value_with_state(VALUE, &subject, &context(), &mut state);

        assert_eq!(
            state.mapping_entries(),
            TransformState::mapping_entries_per_distinct_value(strategy),
            "{strategy:?} recorded {} mapping entry/entries for one distinct value while the \
             projection predicts {}",
            state.mapping_entries(),
            TransformState::mapping_entries_per_distinct_value(strategy),
        );
    }
}

/// Smart replacement's hit path costs a ledger entry and no pseudonym map, which is
/// less than the projection assumes.
///
/// Asserted rather than left implicit because the projection deliberately reports the
/// fallback cost for `LocalAi`, and "the projection over-states this one path" is a
/// claim its documentation makes. If the hit path ever grew a pseudonym map of its own,
/// the two figures would meet and that paragraph would be wrong.
#[test]
fn a_smart_replacement_hit_costs_less_than_the_projection_assumes() {
    let mut subject = column(DataType::String);
    subject.strategy = AnonymizationStrategy::LocalAi;
    let mut replacements = SmartReplacementMap::default();
    replacements.insert(0, VALUE, "Maya Carter");
    let mut state = TransformState::with_smart_replacements(replacements);

    let replaced = transform_value_with_state(VALUE, &subject, &context(), &mut state);

    assert_eq!(
        replaced, "Maya Carter",
        "the seeded hit path should be taken"
    );
    assert_eq!(
        state.mapping_entries(),
        1,
        "a ledger entry and nothing else"
    );
    assert!(
        state.mapping_entries()
            < TransformState::mapping_entries_per_distinct_value(AnonymizationStrategy::LocalAi),
        "the projection is documented as an upper bound for LocalAi"
    );
}

/// The count grows with distinct values, not with rows.
///
/// This is the whole shape of the bug being fixed: a run over a million rows of one
/// repeated value has to stay flat, and the same run over a million distinct values is
/// the one that must be stopped. A counter that counted rows would make the ceiling
/// fire on files that cost nothing.
#[test]
fn repeats_cost_no_further_entries() {
    let mut subject = column(DataType::String);
    subject.strategy = AnonymizationStrategy::Pseudonymize;
    let mut state = TransformState::new();

    transform_value_with_state(VALUE, &subject, &context(), &mut state);
    let after_first = state.mapping_entries();
    for _ in 0..25 {
        transform_value_with_state(VALUE, &subject, &context(), &mut state);
    }
    let after_repeats = state.mapping_entries();
    transform_value_with_state("Someone Else", &subject, &context(), &mut state);

    assert_eq!(
        after_first, 3,
        "one distinct value costs ledger plus both map directions"
    );
    assert_eq!(
        after_repeats,
        after_first,
        "25 repeats of one value added {} entry/entries",
        after_repeats - after_first
    );
    assert_eq!(
        state.mapping_entries(),
        6,
        "a second distinct value costs a second set of entries"
    );
}

/// Passing the ceiling refuses the run and says what to do instead.
///
/// Run against a ceiling of 5 rather than the real 32,000,000, which stands for about
/// 5 GB of mapping: materializing that to test the message would need a machine with
/// the memory the ceiling exists to protect. The ceiling is an argument to
/// `check_mapping_budget_against`, not a setting, so this cannot leak into a
/// production run — `check_mapping_budget`, the only non-test caller, passes the
/// constant. [`the_real_ceiling_is_not_reachable_by_ordinary_work`] covers the wiring
/// of the constant itself.
///
/// The message is asserted clause by clause because each clause answers a question the
/// user is left with otherwise: how far did it get, what is the limit, is there half a
/// file on disk, and what should be done differently. A message that merely said the
/// run was too big would satisfy a `matches!` assertion and none of those.
#[test]
fn passing_the_ceiling_refuses_the_run_and_names_the_remedy() {
    let mut subject = column(DataType::String);
    subject.strategy = AnonymizationStrategy::Pseudonymize;
    let mut state = TransformState::new();
    transform_value_with_state(VALUE, &subject, &context(), &mut state);
    transform_value_with_state("Someone Else", &subject, &context(), &mut state);

    assert!(
        state.check_mapping_budget_against(6).is_ok(),
        "six entries against a ceiling of six is at the ceiling, not past it"
    );
    let error = state
        .check_mapping_budget_against(5)
        .expect_err("six entries past a ceiling of five must refuse the run");

    assert!(
        matches!(
            error,
            AnonymizerError::MappingBudgetExceeded {
                reached: 6,
                ceiling: 5,
                ..
            }
        ),
        "unexpected error: {error:?}"
    );
    let message = error.to_string();
    for clause in [
        "6 mapping entries",
        "ceiling of 5",
        "No output was written",
        "distinct values",
        "Redact or Mask",
        "select fewer columns",
    ] {
        assert!(
            message.contains(clause),
            "the refusal does not say {clause:?}: {message}"
        );
    }
}

/// The real ceiling clears every run this project has measured.
///
/// A ceiling that fired on work the app does today would be a regression dressed as a
/// safeguard, so the figure is pinned against the two runs that are actually measured:
/// the README's four all-distinct columns of a 63 MB input at about 1.9 GB, and its
/// worst single-column case at 477 MiB. Both are re-derived from the entry cost rather
/// than written as literals, so a change to what a strategy records moves the
/// comparison with it.
#[test]
fn the_real_ceiling_is_not_reachable_by_ordinary_work() {
    let per_distinct_value =
        TransformState::mapping_entries_per_distinct_value(AnonymizationStrategy::Pseudonymize);
    let four_all_distinct_columns = 4 * MEASURED_ROWS * per_distinct_value;
    let one_all_distinct_column = MEASURED_ROWS * per_distinct_value;

    assert!(
        TransformState::MAPPING_ENTRY_CEILING > four_all_distinct_columns,
        "the ceiling of {} entries would refuse the largest run this project has measured \
         ({four_all_distinct_columns} entries, about 1.9 GB)",
        TransformState::MAPPING_ENTRY_CEILING
    );
    assert!(
        TransformState::approximate_mapping_megabytes(one_all_distinct_column) > 400,
        "the bytes-per-entry estimate no longer reproduces the 477 MiB measured for one \
         all-distinct pseudonymized column of {MEASURED_ROWS} rows"
    );

    let mut subject = column(DataType::String);
    subject.strategy = AnonymizationStrategy::Pseudonymize;
    let mut state = TransformState::new();
    transform_value_with_state(VALUE, &subject, &context(), &mut state);
    assert!(
        state.check_mapping_budget().is_ok(),
        "the production check must pass for a run holding three entries"
    );
}

/// Preflight reports the projected mapping before the run rather than after it.
///
/// End to end through `AnonymizerService::preflight_anonymization`, because the
/// projection is only worth anything if it survives the whole path: the detection
/// sample has to reach it, the row count has to be the file's rather than the sample's,
/// and the review item has to end up somewhere a user sees. A unit test on the
/// projection arithmetic would have passed with the item wired to nothing.
///
/// Sized to just clear the review threshold with the smallest input that can: the
/// threshold is in entries, and entries are distinct values times three, so twenty
/// columns of sixty thousand distinct values reach 3,600,000 — over the 3,000,000
/// threshold, and the same figure a single 1.2M-row column would need. `String` is
/// forced by control so that Pseudonymize cannot land on a type that defaults to
/// pass-through and hold nothing.
#[test]
fn preflight_projects_the_mapping_a_large_run_would_hold() {
    let readiness = preflight_all_distinct(AnonymizationStrategy::Pseudonymize, DataType::String);

    let projection = readiness
        .review_items
        .iter()
        .find(|item| item.contains("mapping entries"))
        .unwrap_or_else(|| {
            panic!(
                "no mapping projection among the review items: {:?} (status {:?}, blockers {:?})",
                readiness.review_items, readiness.status, readiness.blockers
            )
        });

    assert!(
        projection.contains(&format!("{}", PROJECTED_COLUMNS * PROJECTED_ROWS * 3)),
        "the projection should name the entries it expects: {projection}"
    );
    assert!(
        projection.contains(&format!("{PROJECTED_ROWS} row(s)")),
        "the projection should be scaled to the file's rows, not the sample's: {projection}"
    );
    assert!(
        projection.contains("Redact and Mask"),
        "the projection should name the remedy: {projection}"
    );
    assert_ne!(
        readiness.status,
        ReleaseReadinessStatus::Verified,
        "a review item must move the readiness off Verified"
    );
}

/// The identical file draws no projection when the columns hold no mapping.
///
/// The control for the test above, and it takes two cases because a column is excluded
/// by two independent gates and either one alone would have made this pass:
///
/// - Redact is excluded by its entry cost, which is zero. This is the README's claim
///   that Redact stays flat at any cardinality, measured here as "nothing to report".
/// - Pseudonymize on `Boolean` is excluded by `keeps_consistent_mapping`, and *only* by
///   it: the strategy's entry cost is three, so with that gate gone this input would
///   project 3,600,000 entries for a set of columns the transform returns unchanged.
///   That is the case the extraction of the predicate is there to keep honest.
#[test]
fn preflight_projects_nothing_for_a_run_that_keeps_no_mapping() {
    for (strategy, detected_type) in [
        (AnonymizationStrategy::Redact, DataType::String),
        (AnonymizationStrategy::Pseudonymize, DataType::Boolean),
    ] {
        let readiness = preflight_all_distinct(strategy, detected_type);
        assert!(
            !readiness
                .review_items
                .iter()
                .any(|item| item.contains("mapping entries")),
            "{strategy:?} on {detected_type:?} holds no mapping, so there is nothing to \
             project: {:?}",
            readiness.review_items
        );
    }
}

/// Columns and rows of the projection fixture.
///
/// Sized to just clear the review threshold with the least work: the threshold is
/// 3,000,000 entries and a pseudonymized distinct value costs three of them, so twenty
/// columns of sixty thousand all-distinct values reach 3,600,000 — the same figure a
/// single 1.2M-row column would need, at a twentieth of the rows to parse.
const PROJECTED_COLUMNS: usize = 20;
const PROJECTED_ROWS: usize = 60_000;

/// Runs preflight over an all-distinct fixture with every column on `strategy`.
///
/// The type is forced by control rather than left to detection so that each case tests
/// the gate it means to: detection is free to classify `c0_v41` however its rules say,
/// and a case that turned on that classification would be testing detection.
fn preflight_all_distinct(
    strategy: AnonymizationStrategy,
    detected_type: DataType,
) -> ReleaseReadiness {
    let directory = tempfile::tempdir().expect("preflight temp dir should be created");
    let input_path = directory.path().join("wide-distinct.csv");
    write_all_distinct_csv(&input_path, PROJECTED_COLUMNS, PROJECTED_ROWS);

    AnonymizerService::new("mapping-budget-test")
        .preflight_anonymization(PreflightParams {
            mode: PreflightMode::Anonymize,
            file_path: input_path,
            output_path: Some(directory.path().join("wide-distinct-output.csv")),
            columns: (0..PROJECTED_COLUMNS).collect(),
            controls: (0..PROJECTED_COLUMNS)
                .map(|column_index| ColumnControl {
                    column_index,
                    type_override: Some(detected_type),
                    strategy,
                })
                .collect(),
            force: true,
            sample_row_count: 100,
            preview_smart_replacements: vec![],
            local_ai_ready: false,
            local_ai_message: None,
        })
        .expect("preflight should succeed")
        .readiness
}

fn write_all_distinct_csv(path: &Path, columns: usize, rows: usize) {
    let mut writer = csv::Writer::from_path(path).expect("test CSV should be writable");
    writer
        .write_record((0..columns).map(|column| format!("column_{column}")))
        .expect("header should write");
    for row in 0..rows {
        writer
            .write_record((0..columns).map(|column| format!("c{column}_v{row}")))
            .expect("row should write");
    }
    writer.flush().expect("test CSV should flush");
}

/// Rows used by every measurement below.
///
/// One million because that is the figure the README's memory table is stated
/// against, so a re-measurement is comparable with the numbers already published
/// rather than a second, differently-shaped dataset.
const MEASURED_ROWS: usize = 1_000_000;

/// Peak resident memory of this process, in KiB, or `None` off Linux.
///
/// `VmHWM` rather than a sampled reading of `VmRSS`: the figure wanted is the high
/// water mark over the whole run, and the run's peak is reached somewhere in the
/// middle of the streaming pass, which no reading taken afterwards can see.
fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|value| value.trim().strip_suffix(" kB"))
        .and_then(|value| value.trim().parse().ok())
}

fn write_cardinality_csv(path: &Path, rows: usize, distinct_values: usize) {
    let distinct_values = distinct_values.max(1);
    let mut writer = csv::Writer::from_path(path).expect("measurement CSV should be writable");
    writer
        .write_record(["id", "value"])
        .expect("header should write");
    for index in 0..rows {
        writer
            .write_record([
                index.to_string(),
                format!("value_{:010}", index % distinct_values),
            ])
            .expect("row should write");
    }
    writer.flush().expect("measurement CSV should flush");
}

/// Runs one column of `MEASURED_ROWS` rows under `strategy` and reports the process
/// peak RSS in MiB together with the mapping entries the run ended up holding.
fn measure(strategy: AnonymizationStrategy, distinct_values: usize) -> (u64, usize) {
    let directory = tempfile::tempdir().expect("measurement temp dir should be created");
    let input_path = directory.path().join("cardinality.csv");
    let output_path = directory.path().join("cardinality-output.csv");
    write_cardinality_csv(&input_path, MEASURED_ROWS, distinct_values);

    let service = AnonymizerService::new("mapping-budget-measurement");
    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![1],
            controls: vec![ColumnControl {
                column_index: 1,
                type_override: Some(DataType::String),
                strategy,
            }],
            force: true,
            preview_smart_replacements: vec![],
        })
        .expect("measurement run should succeed");

    // Recomputed here rather than read from the run, because the run's state is
    // dropped with it: one ledger entry per distinct value, plus both directions of
    // the pseudonym map on the strategies that keep one.
    let entries_per_distinct_value = match strategy {
        AnonymizationStrategy::Label => 1,
        AnonymizationStrategy::Pseudonymize | AnonymizationStrategy::Tokenize => 3,
        _ => 0,
    };
    (
        peak_rss_kib().expect("Linux is required for this measurement") / 1024,
        distinct_values * entries_per_distinct_value,
    )
}

/// Reports one measurement in a form that can be pasted into a constant's docs.
fn report(label: &str, strategy: AnonymizationStrategy, distinct_values: usize) {
    let (peak_mib, entries) = measure(strategy, distinct_values);
    println!(
        "{label}: peak RSS {peak_mib} MiB over {MEASURED_ROWS} rows, {distinct_values} distinct \
         value(s), {entries} mapping entry/entries"
    );
}

/// The measurements behind `TransformState::APPROXIMATE_BYTES_PER_MAPPING_ENTRY`.
///
/// Ignored, and each one has to be run in a **separate process**: peak RSS is a
/// process-wide high water mark, so two cardinalities measured in one test binary
/// would both report the larger. Run them one at a time:
///
/// ```text
/// cargo test -p csv-anonymizer-core --release \
///   strategies::tests::mapping_budget::peak_rss_pseudonymize_all_distinct \
///   -- --ignored --exact --nocapture
/// ```
#[test]
#[ignore = "measurement harness: minutes long, and only meaningful one per process"]
fn peak_rss_pseudonymize_all_distinct() {
    report(
        "pseudonymize/all distinct",
        AnonymizationStrategy::Pseudonymize,
        MEASURED_ROWS,
    );
}

/// See [`peak_rss_pseudonymize_all_distinct`] for how to run this.
#[test]
#[ignore = "measurement harness: minutes long, and only meaningful one per process"]
fn peak_rss_pseudonymize_quarter_distinct() {
    report(
        "pseudonymize/250k distinct",
        AnonymizationStrategy::Pseudonymize,
        MEASURED_ROWS / 4,
    );
}

/// See [`peak_rss_pseudonymize_all_distinct`] for how to run this.
#[test]
#[ignore = "measurement harness: minutes long, and only meaningful one per process"]
fn peak_rss_label_all_distinct() {
    report(
        "label/all distinct",
        AnonymizationStrategy::Label,
        MEASURED_ROWS,
    );
}

/// The streaming floor: the same run with a strategy that keeps no mapping at all.
///
/// Subtracted from the measurements above to get the mapping's own cost, so a change
/// in reader or writer buffering cannot be mistaken for a change in bytes per entry.
/// See [`peak_rss_pseudonymize_all_distinct`] for how to run this.
#[test]
#[ignore = "measurement harness: minutes long, and only meaningful one per process"]
fn peak_rss_redact_all_distinct() {
    report(
        "redact/all distinct (floor)",
        AnonymizationStrategy::Redact,
        MEASURED_ROWS,
    );
}
