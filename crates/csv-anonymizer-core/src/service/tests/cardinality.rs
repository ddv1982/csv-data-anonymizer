use super::*;
// Reached through the module rather than a re-export in `service`: only tests need it,
// and re-exporting it there would be dead in a non-test build.
use crate::service::controls::FREQUENCY_INVERSION_MECHANISM;
use crate::types::ColumnMetadata;

/// The clause every cardinality warning ends on, whichever of the three tests fired.
///
/// Filtering on the evidence clause instead is a trap these tests fell into: they matched
/// `"distinct value(s)"`, which the dominant-value wording does not contain, so the moment
/// that wording landed they stopped recognising the warnings they exist to check — and a
/// filter that matches nothing still passes every *negative* assertion, so half this file
/// would have gone quiet without failing.
///
/// Taken from the production constant rather than retyped, which is the part that makes
/// this safe rather than merely better-chosen. A hand-copied substring is the same trap
/// one level up: reword the warning and the filter silently stops matching. Borrowing the
/// constant means a reword keeps these tests working instead of quietly disabling them.
/// What still fails, correctly, is a message that stops ending on the mechanism at all —
/// which is the change that would actually cost a reader the explanation.
const INVERSION_MECHANISM: &str = FREQUENCY_INVERSION_MECHANISM;

/// `repeated-values.csv`: 60 support tickets handled by 5 agents, with a unique
/// ticket id and a 3-valued status.
///
/// Two detection facts decide everything these tests assert, and neither is obvious:
///
/// - **`detect_enum_type` claims any column with more than ten values and at most
///   twenty distinct ones**, and `Enum` is pass-through. It runs at the last detection
///   stage, but it still beats name detection here: `agent_name` holds five repeated
///   full names and comes back `Enum`, not `FullName`. So the *generic* low-cardinality
///   text column never reaches this warning — it is diverted to pass-through before a
///   pseudonym is ever assigned.
/// - **`default_strategy_for_pii_risk` maps High and Medium risk to `Redact`**, which
///   collapses a column to one token and therefore exposes no distribution at all.
///
/// Together those mean the warning cannot fire on the default path. It fires when a
/// user *opts into* a strategy that preserves equality on a column whose values
/// repeat — which is exactly when they have taken on the linkability, and the only
/// point at which telling them is useful.
fn repeated_values_columns() -> Vec<ColumnMetadata> {
    let service = AnonymizerService::new("test-version");
    service
        .analyze_csv(fixture("repeated-values.csv"))
        .unwrap()
        .columns
}

fn cardinality_warnings(columns: Vec<usize>, controls: Vec<ColumnControl>) -> Vec<String> {
    let service = AnonymizerService::new("test-version");
    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: fixture("repeated-values.csv"),
            columns,
            controls,
            sample_count: 5,
            sample_row_count: 100,
        })
        .unwrap();

    preview
        .warnings
        .iter()
        .filter(|warning| warning.message.contains(INVERSION_MECHANISM))
        .map(|warning| warning.column_name.clone())
        .collect()
}

fn control(column_index: usize, strategy: AnonymizationStrategy) -> ColumnControl {
    ColumnControl {
        column_index,
        type_override: None,
        strategy,
    }
}

/// Pins the detection outcomes the rest of this file depends on. If any of these
/// change, the tests below stop testing what they claim to.
#[test]
fn the_fixture_types_and_default_strategies_are_what_these_tests_depend_on() {
    let columns = repeated_values_columns();

    assert_eq!(columns[0].name, "ticket_id");
    assert_eq!(columns[0].detected_type, DataType::NumericId);
    assert_eq!(columns[0].strategy, AnonymizationStrategy::Redact);

    assert_eq!(columns[1].name, "agent_email");
    assert_eq!(columns[1].detected_type, DataType::Email);
    assert_eq!(columns[1].pii_risk, PiiRisk::High);
    // High risk defaults to Redact, which is why the default path cannot warn.
    assert_eq!(columns[1].strategy, AnonymizationStrategy::Redact);

    // Five repeated full names, and Enum wins anyway.
    assert_eq!(columns[2].name, "agent_name");
    assert_eq!(columns[2].detected_type, DataType::Enum);

    assert_eq!(columns[3].name, "status");
    assert_eq!(columns[3].detected_type, DataType::Enum);
}

#[test]
fn the_detection_sample_measures_the_repeated_value_distribution() {
    let columns = repeated_values_columns();

    let email = columns[1].sample_value_distribution;
    assert_eq!(email.total_values, 60);
    assert_eq!(email.distinct_values, 5);
    assert_eq!(email.singleton_values, 0);
    assert_eq!(email.max_value_occurrences, 12);

    let ticket_id = columns[0].sample_value_distribution;
    assert_eq!(ticket_id.total_values, 60);
    assert_eq!(ticket_id.distinct_values, 60);
    assert_eq!(ticket_id.singleton_values, 60);
    assert_eq!(ticket_id.max_value_occurrences, 1);
}

/// The default strategies leave nothing to warn about, even though two columns hold
/// only five distinct values across sixty rows. Worth pinning: it is the difference
/// between a warning that means "you chose this" and one that fires on arrival and
/// gets ignored.
#[test]
fn no_column_is_warned_about_under_the_default_strategies() {
    assert!(
        cardinality_warnings(vec![0, 1, 2, 3], vec![]).is_empty(),
        "the default path produced a cardinality warning"
    );
}

/// The case the warning exists for: the user moves a column with repeated values onto
/// a strategy that keeps repeats linkable.
#[test]
fn warns_when_a_user_opts_into_consistent_pseudonyms_on_repeated_values() {
    for strategy in [
        AnonymizationStrategy::Pseudonymize,
        AnonymizationStrategy::Auto,
        AnonymizationStrategy::Tokenize,
        AnonymizationStrategy::Label,
    ] {
        assert_eq!(
            cardinality_warnings(vec![1], vec![control(1, strategy)]),
            vec!["agent_email".to_string()],
            "{strategy:?} did not warn"
        );
    }
}

/// Local AI belongs in the set above but cannot be driven through the preview path
/// here: it refuses to run without a live Ollama, which a unit test has no business
/// requiring. Its coverage is pinned on the predicate directly instead of being
/// quietly dropped — a Local AI column reuses one replacement per distinct value just
/// as the others do, so it leaks the same distribution.
#[test]
fn a_local_ai_column_is_covered_by_the_warning_predicate() {
    let mut column = repeated_values_columns().remove(1);
    column.strategy = AnonymizationStrategy::LocalAi;

    assert!(cardinality_warning_for_column(&column, 60).is_some());
}

/// Sixty distinct ids over sixty rows: nothing to invert, whichever strategy is
/// chosen. This is the case a singleton-based rule would have flagged — every value
/// occurs exactly once — and the reason singletons are not in the predicate.
#[test]
fn a_fully_distinct_column_is_not_warned_about_even_when_pseudonymized() {
    assert!(
        cardinality_warnings(
            vec![0],
            vec![control(0, AnonymizationStrategy::Pseudonymize)]
        )
        .is_empty()
    );
}

/// An `Enum` column stays pass-through under a pseudonymizing strategy, so it never
/// assigns a pseudonym and never exposes a mapping — regardless of how few distinct
/// values it holds.
#[test]
fn an_enum_column_is_not_warned_about_because_its_type_passes_through() {
    for column_index in [2usize, 3] {
        assert!(
            cardinality_warnings(
                vec![column_index],
                vec![control(column_index, AnonymizationStrategy::Pseudonymize)]
            )
            .is_empty(),
            "column {column_index} was warned about"
        );
    }
}

/// Redact collapses a column to one token and mask rewrites each value on its own, so
/// neither leaves a distribution to invert.
#[test]
fn strategies_that_expose_no_distribution_are_not_warned_about() {
    for strategy in [
        AnonymizationStrategy::Redact,
        AnonymizationStrategy::Mask,
        AnonymizationStrategy::PassThrough,
    ] {
        assert!(
            cardinality_warnings(vec![1], vec![control(1, strategy)]).is_empty(),
            "{strategy:?} was warned about"
        );
    }
}

/// The cardinality warning must not be mistaken for the pass-through signal.
///
/// Two report builders read *whether* `preview_warning_for_column` returned anything
/// as a proxy for "this column is effectively pass-through". A cardinality note added
/// to that function would have silently counted this pseudonymized column as
/// pass-through. The warning lives in its own function for that reason, and this pins
/// the consequence rather than the mechanism.
#[test]
fn warning_about_cardinality_does_not_count_the_column_as_pass_through() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("repeated-values-anonymized.csv");

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("repeated-values.csv"),
            output_path,
            columns: vec![1],
            controls: vec![control(1, AnonymizationStrategy::Pseudonymize)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let report = result.privacy_report;
    assert_eq!(report.pseudonymized_columns, 1);
    assert_eq!(report.pass_through_columns, 0);
}

/// The post-run report measures the same thing the pre-run warning estimated, over
/// every row rather than a sample. Here the sample covered the whole file, so the two
/// agree exactly.
#[test]
fn the_run_reports_the_exact_distribution_of_each_pseudonymized_column() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("repeated-values-anonymized.csv");

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("repeated-values.csv"),
            output_path,
            columns: vec![1],
            controls: vec![control(1, AnonymizationStrategy::Pseudonymize)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let distributions = result.privacy_report.column_value_distributions;
    assert_eq!(distributions.len(), 1, "{distributions:?}");
    assert_eq!(distributions[0].column_index, 1);
    assert_eq!(distributions[0].distinct_values, 5);
    assert_eq!(distributions[0].total_values, 60);
    assert_eq!(distributions[0].max_value_occurrences, 12);
}

/// A redacted column contributes no distribution to the report, matching the reason
/// it is not warned about.
#[test]
fn a_redacted_column_reports_no_distribution() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("repeated-values-redacted.csv");

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("repeated-values.csv"),
            output_path,
            columns: vec![1],
            controls: vec![control(1, AnonymizationStrategy::Redact)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    assert!(
        result.privacy_report.column_value_distributions.is_empty(),
        "{:?}",
        result.privacy_report.column_value_distributions
    );
    assert_eq!(result.privacy_report.redacted_columns, 1);
}

/// The report has to say the two things the warning implies, in the reviewer's own
/// view: which columns could be matched back by frequency, and that consistent
/// replacement means the output is pseudonymized rather than anonymized.
#[test]
fn the_report_names_the_invertible_columns_and_the_pseudonymization_caveat() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("repeated-values-reported.csv");

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("repeated-values.csv"),
            output_path,
            columns: vec![1],
            controls: vec![control(1, AnonymizationStrategy::Pseudonymize)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();
    let report = result.privacy_report;

    let frequency_note = report
        .notes
        .iter()
        .find(|note| note.contains("matched back by how often"))
        .expect("expected a frequency-inversion note");
    assert!(frequency_note.contains("agent_email"), "{frequency_note}");
    assert!(
        frequency_note.contains("5 distinct of 60 values"),
        "{frequency_note}"
    );

    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("pseudonymized rather than anonymized")),
        "{:?}",
        report.notes
    );
    assert!(
        report
            .readiness
            .review_items
            .iter()
            .any(|item| item.contains("matched back by value frequency")),
        "{:?}",
        report.readiness.review_items
    );
}

/// A redacted column earns neither note: nothing was pseudonymized, so there is no
/// mapping to invert and no linkability caveat to make.
#[test]
fn a_redacted_run_reports_no_frequency_or_pseudonymization_caveat() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("repeated-values-redacted-notes.csv");

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("repeated-values.csv"),
            output_path,
            columns: vec![1],
            controls: vec![control(1, AnonymizationStrategy::Redact)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    for note in &result.privacy_report.notes {
        assert!(!note.contains("matched back by how often"), "{note}");
        assert!(
            !note.contains("pseudonymized rather than anonymized"),
            "{note}"
        );
    }
}

/// Builds a CSV whose only interesting property is its shape: `rows` rows over
/// `distinct` repeating values, which is the shape the sampled ratio test could not
/// see.
fn repeating_column_file(
    directory: &std::path::Path,
    rows: usize,
    distinct: usize,
) -> std::path::PathBuf {
    let path = directory.join(format!("repeating-{rows}-{distinct}.csv"));
    let mut text = String::from("row_id,department\n");
    for row in 0..rows {
        text.push_str(&format!("{row},dept-{}\n", row % distinct));
    }
    std::fs::write(&path, text).unwrap();
    path
}

fn cardinality_warnings_for(
    file_path: std::path::PathBuf,
    strategy: AnonymizationStrategy,
) -> Vec<crate::types::PreviewWarning> {
    let service = AnonymizerService::new("test-version");
    service
        .preview_anonymization(PreviewParams {
            file_path,
            columns: vec![1],
            controls: vec![control(1, strategy)],
            sample_count: 5,
            sample_row_count: 100,
        })
        .unwrap()
        .warnings
        .into_iter()
        .filter(|warning| warning.message.contains(INVERSION_MECHANISM))
        .collect()
}

fn warned_columns(file_path: std::path::PathBuf, strategy: AnonymizationStrategy) -> Vec<String> {
    cardinality_warnings_for(file_path, strategy)
        .into_iter()
        .map(|warning| warning.column_name)
        .collect()
}

/// The message of the single warning column 1 draws, for the tests that assert wording.
///
/// Asserts the count rather than taking the first, so a second warning appearing cannot
/// let a wording test quietly check the wrong message.
fn sole_warning_message(file_path: std::path::PathBuf, strategy: AnonymizationStrategy) -> String {
    let mut warnings = cardinality_warnings_for(file_path, strategy);
    assert_eq!(
        warnings.len(),
        1,
        "expected exactly one cardinality warning, got {warnings:?}"
    );
    warnings.remove(0).message
}

/// The regression this rule exists for. 30 departments across 5000 rows is trivially
/// invertible, and the detection sample sees 29 distinct of 100 — which the absolute
/// test misses (29 is not below 10) and which the old ratio test also missed, because
/// 29/100 is nowhere near 0.05. The warning arrived only in the privacy report, after
/// the output had been written and the choice could no longer be changed.
#[test]
fn warns_before_the_run_about_values_that_repeat_across_a_file_larger_than_the_sample() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = repeating_column_file(temp_dir.path(), 5000, 30);

    assert_eq!(
        warned_columns(path, AnonymizationStrategy::Pseudonymize),
        vec!["department".to_string()]
    );
}

/// The same column redacted is silent, so the warning still tracks the strategy
/// rather than the shape of the data.
#[test]
fn a_redacted_column_is_silent_however_much_its_values_repeat() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = repeating_column_file(temp_dir.path(), 5000, 30);

    assert!(warned_columns(path, AnonymizationStrategy::Redact).is_empty());
}

/// The counterweight, and the reason the coverage gate exists: a column where every
/// value is distinct must stay silent in a large file. Judged on the row count alone
/// its 100 sampled values imply a ratio of 0.0001 and it would be flagged as
/// trivially invertible — the opposite of true.
#[test]
fn a_unique_column_in_a_large_file_is_not_warned_about() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("unique.csv");
    let mut text = String::from("row_id,reference\n");
    for row in 0..5000 {
        text.push_str(&format!("{row},ref-{row}\n"));
    }
    std::fs::write(&path, text).unwrap();

    assert!(warned_columns(path, AnonymizationStrategy::Pseudonymize).is_empty());
}

/// A column with enough distinct values to make frequency matching impractical stays
/// silent too — 1000 values over 5000 rows is five rows a group.
#[test]
fn a_high_cardinality_column_is_not_warned_about() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = repeating_column_file(temp_dir.path(), 5000, 1000);

    assert!(warned_columns(path, AnonymizationStrategy::Pseudonymize).is_empty());
}

/// SplitMix64, so every generated column below is a pure function of its parameters.
///
/// A generator seeded from the clock would make a failure here unreproducible, and these
/// tests exist to hold a threshold in place: one that fails on a Tuesday and passes on a
/// Wednesday tells a reader the threshold is fine when it is not. The same function is
/// what [`crate::sampling`] hashes positions with, so the two are independent only in the
/// sense that they are offset — hence the offsets below.
fn deterministic_unit(position: usize) -> f64 {
    let mut state = (position as u64).wrapping_add(0x9e37_79b9_7f4a_7c15);
    state = (state ^ (state >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    state = (state ^ (state >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    let hashed = state ^ (state >> 31);
    (hashed >> 11) as f64 / (1u64 << 53) as f64
}

/// A CSV whose second column is Zipf-distributed: `distinct` labels whose frequencies
/// fall off as `1 / rank^exponent`, over `rows` rows.
///
/// The knob that matters is `exponent`, because it is what real categorical data varies
/// along and what the dominant-share threshold had to be calibrated against. At 0 the
/// column is uniform; at 1 it is Zipf's law, the ordinary shape of a city or surname
/// column, whose most common value takes a seventh to a sixth of the rows; by 2 one value
/// takes three fifths of them. `MIN_INVERTIBLE_DOMINANT_SHARE` in `types.rs` carries the
/// sweep this produced.
///
/// `seed` shifts the draw without changing the shape, which is what makes a fire *rate*
/// measurable rather than a single yes or no — the figures in that constant's doc comment
/// are fire rates over thousands of seeds.
fn zipf_column_file(
    directory: &std::path::Path,
    rows: usize,
    distinct: usize,
    exponent: f64,
    seed: usize,
) -> std::path::PathBuf {
    let mut cumulative = Vec::with_capacity(distinct);
    let mut running = 0.0;
    for rank in 1..=distinct {
        running += 1.0 / (rank as f64).powf(exponent);
        cumulative.push(running);
    }

    let path = directory.join(format!("zipf-{rows}-{distinct}-{exponent}-{seed}.csv"));
    let mut text = String::from("row_id,department\n");
    for row in 0..rows {
        // Offset so the value drawn at a position is independent of the spread sampler's
        // own hash of that same position; without it the sample is drawn preferentially
        // from one part of the distribution.
        let target = deterministic_unit(row + 0x5000_0000 + seed * 0x0100_0000) * running;
        let rank = cumulative.partition_point(|bound| *bound < target);
        text.push_str(&format!("{row},dept-{}\n", rank.min(distinct - 1)));
    }
    std::fs::write(&path, text).unwrap();
    path
}

/// A CSV whose second column holds one value covering `share` of the rows, with the rest
/// spread evenly over `tail` other values.
///
/// Separate from [`zipf_column_file`] because Zipf cannot produce this shape, and the
/// difference is what the dominant-value term turns on. A Zipf column concentrates its
/// mass in a *body* of common values, which raises Good–Turing coverage along with the
/// skew; a dominant value over a long thin tail is just as invertible while its coverage
/// stays low, because the tail is nearly all singletons. That is the shape that gets past
/// both older terms, and the shape that made a coverage gate on the new one unusable.
fn dominant_value_column_file(
    directory: &std::path::Path,
    rows: usize,
    share: f64,
    tail: usize,
    seed: usize,
) -> std::path::PathBuf {
    let path = directory.join(format!("dominant-{rows}-{share}-{tail}-{seed}.csv"));
    let mut text = String::from("row_id,department\n");
    for row in 0..rows {
        let draw = deterministic_unit(row + 0x7000_0000 + seed * 0x0100_0000);
        if draw < share {
            text.push_str(&format!("{row},dept-central\n"));
        } else {
            let index = ((draw - share) / (1.0 - share) * tail as f64) as usize;
            text.push_str(&format!("{row},dept-{}\n", index.min(tail - 1)));
        }
    }
    std::fs::write(&path, text).unwrap();
    path
}

/// `dominant-value.csv`: 200 tickets, 101 handling queues, one of which took half of
/// them.
///
/// Pinned because every assertion about this fixture depends on figures the detection
/// sample computes rather than on the file's own shape, and the two differ here — the
/// file has 200 rows but detection reads a 100-row spread sample of it. Also pins that
/// the column is *not* diverted before the warning can be reached: 101 queues is well
/// past `detect_enum_type`'s twenty-value limit, so unlike `repeated-values.csv`'s
/// `agent_name` this column stays pseudonymizable.
#[test]
fn the_dominant_value_fixture_has_the_shape_these_tests_depend_on() {
    let service = AnonymizerService::new("test-version");
    let analysis = service.analyze_csv(fixture("dominant-value.csv")).unwrap();
    assert_eq!(analysis.row_count, 200);

    let queue = &analysis.columns[1];
    assert_eq!(queue.name, "handling_queue");
    assert_ne!(
        queue.detected_type,
        DataType::Enum,
        "an Enum column is pass-through, so the warning could never be reached"
    );
    assert!(!queue.detected_type.uses_default_pass_through());

    let distribution = queue.sample_value_distribution;
    assert_eq!(
        distribution.total_values, 100,
        "a 100-row sample of 200 rows"
    );
    assert_eq!(distribution.max_value_occurrences, 51);
    assert_eq!(distribution.distinct_values, 50);
    assert_eq!(distribution.singleton_values, 49);

    // The two figures that decide the verdict, stated as the predicate reads them: the
    // dominant value covers 0.51 of the sample, and neither other term can answer.
    assert!(distribution.distinct_values >= 10, "absolute term is out");
    let coverage = 1.0 - distribution.singleton_values as f64 / distribution.total_values as f64;
    assert!(coverage < 0.75, "{coverage} — ratio term is gated out");
}

/// The gap this term closes, end to end through the real analyze-and-preview path.
///
/// 101 distinct queues is a diverse column by every measure the other two terms use, and
/// one queue still handles half the tickets. Before the dominant-value term the preview
/// was silent here and the user learned about it only from the privacy report, after the
/// output had been written — the same failure the ratio term was added for, in a column
/// the ratio term cannot reach.
#[test]
fn warns_before_the_run_about_a_single_value_that_dominates_a_diverse_column() {
    for strategy in [
        AnonymizationStrategy::Pseudonymize,
        AnonymizationStrategy::Auto,
        AnonymizationStrategy::Tokenize,
        AnonymizationStrategy::Label,
    ] {
        assert_eq!(
            warned_columns(fixture("dominant-value.csv"), strategy),
            vec!["handling_queue".to_string()],
            "{strategy:?} did not warn"
        );
    }
}

/// Each of the three tests has to describe *its own* evidence.
///
/// The warning is the only place a user learns why a column was flagged, and for a while
/// all three verdicts rendered as "holds only N distinct value(s)". On the dominant-value
/// column that came out as "holds only 101 distinct value(s)" — a true statement, a
/// reassuring one, and a description of a risk the column does not have. So each wording
/// is pinned to the figure that justifies it, and pinned *against* the other two terms'
/// framing, which is the half that catches a regression to one-size-fits-all wording.
#[test]
fn each_warning_names_the_evidence_that_produced_it() {
    let dominant = sole_warning_message(
        fixture("dominant-value.csv"),
        AnonymizationStrategy::Pseudonymize,
    );
    // 51 of the 100 sampled values, so the share is reported as 51%.
    assert!(
        dominant.contains("One value fills 51% of handling_queue's 100 measured value(s)"),
        "{dominant}"
    );
    assert!(dominant.contains("out of 200 row(s)"), "{dominant}");
    assert!(
        dominant.contains("recovers that share of the column"),
        "{dominant}"
    );
    assert!(
        !dominant.contains("distinct value(s)"),
        "the dominant-value warning still claims a distinct-count risk: {dominant}"
    );

    let few_distinct = sole_warning_message(
        fixture("repeated-values.csv"),
        AnonymizationStrategy::Pseudonymize,
    );
    assert!(
        few_distinct.contains(
            "agent_email holds only 5 distinct value(s) in a 60-value sample of 60 row(s)"
        ),
        "{few_distinct}"
    );
    assert!(!few_distinct.contains("One value fills"), "{few_distinct}");

    // 30 departments across 5000 rows: too many to enumerate, none dominant, and every
    // replacement covers around 166 rows. The estimate is the sample's, not the file's,
    // which is why the wording says "estimated" and the figure is not exactly 30.
    let temp_dir = tempfile::tempdir().unwrap();
    let large_groups = sole_warning_message(
        repeating_column_file(temp_dir.path(), 5000, 30),
        AnonymizationStrategy::Pseudonymize,
    );
    assert!(
        large_groups
            .contains("department holds an estimated 29 distinct value(s) across 5000 row(s)"),
        "{large_groups}"
    );
    assert!(
        large_groups.contains("each replacement covers around 172 of them"),
        "{large_groups}"
    );
    assert!(!large_groups.contains("One value fills"), "{large_groups}");

    // The mechanism clause is what every wording shares, and what the filters key on.
    // Asserted against the constant itself: this is the property that lets the rest of
    // the file recognise a cardinality warning at all, so it is worth stating in full
    // rather than by a fragment that could drift out of the sentence it came from.
    for message in [&dominant, &few_distinct, &large_groups] {
        assert!(message.contains(FREQUENCY_INVERSION_MECHANISM), "{message}");
    }
}

/// The post-run report has the same obligation, on exact figures rather than sampled
/// ones.
///
/// Here the misleading version is at its worst: the ledger counted every row, so the
/// report would have stated "handling_queue (101 distinct of 200 values)" as a measured
/// fact. A reviewer reading a *flagged* column described by a comfortable-looking ratio
/// has been given a reason to dismiss it.
#[test]
fn the_post_run_report_names_the_dominant_value_rather_than_the_distinct_count() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("dominant-value.csv"),
            output_path: temp_dir.path().join("dominant-value-reported.csv"),
            columns: vec![1],
            controls: vec![control(1, AnonymizationStrategy::Pseudonymize)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();
    let report = result.privacy_report;

    let note = report
        .notes
        .iter()
        .find(|note| note.contains("matched back by how often"))
        .expect("expected a frequency-inversion note");
    // 100 of 200 values, counted exactly rather than estimated from a sample.
    assert!(
        note.contains("handling_queue (one value in 50% of 200 values)"),
        "{note}"
    );
    assert!(
        !note.contains("101 distinct"),
        "the report still describes the column by its distinct count: {note}"
    );
}

/// Redact still silences it, so the new term tracks the strategy rather than the shape
/// of the data exactly as the older ones do.
#[test]
fn a_dominant_value_column_is_silent_when_it_exposes_no_distribution() {
    for strategy in [
        AnonymizationStrategy::Redact,
        AnonymizationStrategy::Mask,
        AnonymizationStrategy::PassThrough,
    ] {
        assert!(
            warned_columns(fixture("dominant-value.csv"), strategy).is_empty(),
            "{strategy:?} was warned about"
        );
    }
}

/// A dominant value in a file far larger than the sample, which is the case the term was
/// calibrated for and the one an absolute count of occurrences would get wrong.
///
/// One value over half of 5000 rows with the other 5000-odd values spread thin: 0 of 120
/// measured draws of this shape fired either older term, at every combination of tail
/// size and row count swept.
#[test]
fn warns_about_a_dominant_value_in_a_file_much_larger_than_the_sample() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = dominant_value_column_file(temp_dir.path(), 5000, 0.5, 5000, 0);

    assert_eq!(
        warned_columns(path, AnonymizationStrategy::Pseudonymize),
        vec!["department".to_string()]
    );
}

/// The false-positive counterweight, and the reason the threshold is a third rather than
/// a fifth. A Zipf column with exponent 1.0 is the ordinary shape of real categorical
/// data — a city, a surname, a product code — and its most common value takes a seventh
/// of the rows. If this fires, the warning fires on most text columns in most files and
/// stops being read.
#[test]
fn an_ordinarily_skewed_column_is_not_warned_about() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = zipf_column_file(temp_dir.path(), 5000, 1000, 1.0, 0);

    assert!(warned_columns(path, AnonymizationStrategy::Pseudonymize).is_empty());
}

/// Ten independent draws of that same shape, because one draw says nothing about a
/// threshold judged on a 100-value sample.
///
/// The sampled dominant share of a Zipf-1.0 column varies by about ±0.09 between draws,
/// so a single seed can sit either side of a boundary by luck. This is the cheap standing
/// version of the 4000-draw measurement behind `MIN_INVERTIBLE_DOMINANT_SHARE`: it is
/// what would catch someone lowering that constant to a fifth, where the measured fire
/// rate on this shape is 25%.
#[test]
fn an_ordinarily_skewed_column_stays_silent_across_independent_draws() {
    let temp_dir = tempfile::tempdir().unwrap();

    for seed in 0..10 {
        let path = zipf_column_file(temp_dir.path(), 5000, 1000, 1.0, seed);
        assert!(
            warned_columns(path, AnonymizationStrategy::Pseudonymize).is_empty(),
            "seed {seed} warned about an ordinarily skewed column"
        );
    }
}

/// And ten draws of the shape that must always be caught, for the same reason in the
/// other direction. Measured fire rate at the chosen threshold is 0.999 or better, so ten
/// draws passing is expected and one failing is a real signal.
#[test]
fn a_dominated_column_is_warned_about_across_independent_draws() {
    let temp_dir = tempfile::tempdir().unwrap();

    for seed in 0..10 {
        let path = dominant_value_column_file(temp_dir.path(), 5000, 0.5, 5000, seed);
        assert_eq!(
            warned_columns(path, AnonymizationStrategy::Pseudonymize),
            vec!["department".to_string()],
            "seed {seed} missed a value covering half the column"
        );
    }
}

/// The post-run report has to reach the same verdict the preview did, on the same file.
///
/// The two measure the column differently — the preview estimates from 100 sampled values
/// and the run counts all 200 — so agreement is a property worth testing rather than an
/// identity. It is also where a share-based threshold earns its keep: the sample's
/// dominant count is 51 and the run's is 100, and only the share is comparable.
#[test]
fn the_run_reports_the_dominant_value_the_preview_warned_about() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("dominant-value-anonymized.csv");

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("dominant-value.csv"),
            output_path,
            columns: vec![1],
            controls: vec![control(1, AnonymizationStrategy::Pseudonymize)],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let distributions = result.privacy_report.column_value_distributions;
    assert_eq!(distributions.len(), 1, "{distributions:?}");
    assert_eq!(distributions[0].distinct_values, 101);
    assert_eq!(distributions[0].total_values, 200);
    assert_eq!(distributions[0].max_value_occurrences, 100);

    assert!(
        result
            .privacy_report
            .notes
            .iter()
            .any(|note| note.contains("matched back by how often")),
        "{:?}",
        result.privacy_report.notes
    );
}

/// A CSV whose second column holds two near-equally common values — `dominant` rows of
/// one and `runner_up` of the other — with the remaining `tail` rows spread over that
/// many distinct singleton values.
///
/// Exact rather than drawn, unlike the two generators above, because the whole point of
/// the shape is where the top two counts sit relative to each other and to
/// `MIN_INVERTIBLE_DOMINANT_SHARE`. A sampling generator puts the top two within a few
/// draws of each other and the straddle becomes a coin flip. Neither existing generator
/// can produce this: `repeating_column_file` is uniform, `dominant_value_column_file`
/// spreads the non-dominant mass evenly so the runner-up is tiny, and a Zipf column with
/// a near-tie at the top (`exponent` near 0) is uniform and has no dominant value at all.
///
/// `dominant + runner_up + tail` is kept at or below the 100-value detection sample so
/// the sample covers every row and the shares are the file's own, not an estimate of
/// them — the estimate's variance is a separate question, measured elsewhere in this file.
fn near_tie_column_file(
    directory: &std::path::Path,
    dominant: usize,
    runner_up: usize,
    tail: usize,
) -> std::path::PathBuf {
    let path = directory.join(format!("near-tie-{dominant}-{runner_up}-{tail}.csv"));
    let mut text = String::from("row_id,department\n");
    let mut row = 0;
    for _ in 0..dominant {
        text.push_str(&format!("{row},dept-central\n"));
        row += 1;
    }
    for _ in 0..runner_up {
        text.push_str(&format!("{row},dept-second\n"));
        row += 1;
    }
    for index in 0..tail {
        text.push_str(&format!("{row},dept-{index}\n"));
        row += 1;
    }
    std::fs::write(&path, text).unwrap();
    path
}

/// A recorded over-warn, kept rather than fixed.
///
/// At 34 / 33 out of 100 the dominant-value term fires on the 34, and the message tells
/// the user that identifying that one value "recovers that share of the column at a
/// stroke". That overstates this column: the two values are one row apart, so an attacker
/// reading the frequency table cannot tell which pseudonym is which, and the recovery the
/// wording promises is closer to a coin flip than a certainty. The term is measuring the
/// wrong thing here — a share is only an anchor if it is *distinguishable* from the next
/// one down.
///
/// It is deliberately left as it is, and this test is the record of that rather than of an
/// oversight. Two reasons:
///
/// - **The error is in the safe direction.** A user is told a column is invertible when it
///   is only half-invertible, and the remedy offered — pick a strategy that does not
///   preserve equality — is correct for both cases. The opposite error, staying silent on
///   a genuinely dominated column, is the failure the term was added to close.
/// - **The precise rule is not expressible from what is measured.** Refining it needs the
///   *second*-largest occurrence count, so the fire condition could read "the dominant
///   value is both large and clearly ahead of the runner-up".
///   `ColumnValueDistribution` carries `max_value_occurrences` and nothing about rank two,
///   and it is a serialized type: adding a field means a change to the serde contract
///   checked by `scripts/check-contracts.mjs` and a matching change to the frontend
///   mirror, for a refinement that only narrows a warning already erring safely.
///
/// What would have to change to fix it properly, in order: carry the runner-up count on
/// `ColumnValueDistribution` (contract plus frontend mirror), gate
/// `FrequencyInversionRisk::DominantValue` on a *margin* between rank one and rank two as
/// well as on `MIN_INVERTIBLE_DOMINANT_SHARE`, calibrate that margin the way the share was
/// calibrated — against Zipf draws, where rank one and rank two differ by `2^exponent` and
/// an ordinary column's top two are already close — and then invert this test.
///
/// The runner-up's 33 rows appear only in the generator call, which is the point stated as
/// a property: nothing the predicate can read distinguishes this column from one whose
/// other 66 values are all singletons.
#[test]
fn a_dominant_value_its_runner_up_nearly_matches_is_warned_about_anyway() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = near_tie_column_file(temp_dir.path(), 34, 33, 33);

    // The shape the verdict turns on, read as the predicate reads it: 34 of 100 values is
    // a 0.34 share, just past the 1/3 threshold, and both other terms are out — 35
    // distinct values is well past the absolute term's ten, and 33 singletons put coverage
    // at 0.67, below the ratio term's gate.
    let service = AnonymizerService::new("test-version");
    let analysis = service.analyze_csv(path.clone()).unwrap();
    assert_eq!(analysis.row_count, 100);

    let column = &analysis.columns[1];
    assert_eq!(column.name, "department");
    assert_ne!(
        column.detected_type,
        DataType::Enum,
        "an Enum column is pass-through, so the warning could never be reached"
    );
    assert!(!column.detected_type.uses_default_pass_through());

    let distribution = column.sample_value_distribution;
    assert_eq!(
        distribution.total_values, 100,
        "the sample has to cover every row for the shares to be the file's own"
    );
    assert_eq!(distribution.max_value_occurrences, 34);
    assert_eq!(distribution.distinct_values, 35);
    let coverage = 1.0 - distribution.singleton_values as f64 / distribution.total_values as f64;
    assert!(coverage < 0.75, "{coverage} — ratio term is gated out");

    // Matched on the dominant-value clause rather than on `FREQUENCY_INVERSION_MECHANISM`:
    // the shared mechanism sentence would be satisfied by any of the three terms, and the
    // over-warn being pinned is specifically this term's claim about a single value.
    let message = sole_warning_message(path, AnonymizationStrategy::Pseudonymize);
    assert!(
        message.contains("One value fills 34% of department's 100 measured value(s)"),
        "{message}"
    );
    assert!(
        message
            .contains("Identifying that one value recovers that share of the column at a stroke."),
        "{message}"
    );
}
