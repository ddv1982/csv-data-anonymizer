use super::*;
use crate::error::AnonymizerError;
use crate::metadata::{apply_column_selection, build_column_metadata};
use crate::types::ProcessProgress;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn reads_sample_and_strips_bom() {
    let sample = read_sample(&fixture("bom-file.csv"), 10).unwrap();
    assert_eq!(sample.headers[0], "id");
}

#[test]
fn head_sample_keeps_the_opening_rows_and_reports_it_stopped_early() {
    let content = build_numbered_csv(50);
    let sample = read_csv_sample_from_str(&content, 10).unwrap();

    assert_eq!(sample.rows.len(), 10);
    assert_eq!(sample.rows[0][0], "0");
    assert_eq!(sample.rows[9][0], "9");
    assert!(!sample.scanned_entire_input);
}

#[test]
fn detection_sample_spans_the_whole_input_in_input_order() {
    let content = build_numbered_csv(1_000);
    let sample = read_csv_detection_sample_from_str(&content, 100).unwrap();

    assert_eq!(sample.rows.len(), 100);
    assert_eq!(sample.data_rows_scanned, 1_000);
    assert!(sample.scanned_entire_input);

    let kept = kept_row_numbers(&sample);
    assert!(
        kept.windows(2).all(|pair| pair[0] < pair[1]),
        "the sample must come back in input order, got {kept:?}"
    );

    // Every tenth of the input contributes. A 100-row sample of 1,000 rows draws
    // about 10 from each, so an empty tenth would mean the sample is reading a
    // window of the input rather than the whole of it.
    for tenth in 0..10 {
        let range = tenth * 100..(tenth + 1) * 100;
        assert!(
            kept.iter().any(|row| range.contains(row)),
            "rows {range:?} contributed nothing to {kept:?}"
        );
    }
}

/// The choice of rows must not correlate with position modulo anything, because
/// real inputs are periodic: a flattened export writes one logical record per k
/// rows and puts each field on a fixed row of the block. Sampling such a file at a
/// fixed phase — which is what keeping every nth row does, k and n being powers of
/// two often enough — either sees a field on every sampled row or never sees it at
/// all. The second case classifies the column off filler values, which is a column
/// of real PII detected as `String` and left unselected.
#[test]
fn detection_sample_does_not_align_with_a_periodic_input() {
    const WANTED: usize = 200;

    for period in [2usize, 3, 4, 5, 8, 16] {
        let sample =
            read_csv_detection_sample_from_str(&build_numbered_csv(period * 500), WANTED).unwrap();
        let kept = kept_row_numbers(&sample);

        for phase in 0..period {
            let hits = kept.iter().filter(|row| *row % period == phase).count();
            assert!(
                hits >= WANTED / (period * 4),
                "phase {phase} of {period} got {hits} of {WANTED} sampled rows"
            );
        }
    }
}

fn kept_row_numbers(sample: &ParsedSample) -> Vec<usize> {
    sample
        .rows
        .iter()
        .map(|row| row[0].parse().unwrap())
        .collect()
}

#[test]
fn detection_sample_is_deterministic() {
    let content = build_numbered_csv(777);
    let first = read_csv_detection_sample_from_str(&content, 32).unwrap();
    let second = read_csv_detection_sample_from_str(&content, 32).unwrap();

    assert_eq!(first.rows, second.rows);
}

#[test]
fn detection_sample_keeps_every_row_of_a_short_input() {
    let content = build_numbered_csv(20);
    let sample = read_csv_detection_sample_from_str(&content, 100).unwrap();

    assert_eq!(sample.rows.len(), 20);
    assert_eq!(sample.data_rows_scanned, 20);
    assert!(sample.scanned_entire_input);
}

/// The kept count must be exactly what the caller asked for, at any input length.
/// Detection votes on match ratios, so a sample that quietly varies in size with
/// where the input's length happens to fall is a vote of varying and unstated
/// confidence. An earlier thinning sampler returned between half the request and
/// all of it for that reason — 200 rows yielded 50, a million yielded 62.
#[test]
fn detection_sample_keeps_exactly_the_requested_row_count() {
    const WANTED: usize = 100;

    for row_count in [150, 200, 300, 512, 1_000, 4_097, 10_500, 65_536] {
        let sample = read_csv_detection_sample_from_str(&build_numbered_csv(row_count), WANTED)
            .expect("sample reads");

        assert_eq!(
            sample.rows.len(),
            WANTED,
            "a {row_count}-row input yielded {} of {WANTED} requested rows",
            sample.rows.len()
        );
        assert_eq!(sample.data_rows_scanned, row_count);
    }
}

fn build_numbered_csv(row_count: usize) -> String {
    let mut content = String::from("n\n");
    for row in 0..row_count {
        content.push_str(&format!("{row}\n"));
    }
    content
}

#[test]
fn reads_csv_sample_from_str() {
    let sample = read_csv_sample_from_str("email\nada@example.com\n", 10).unwrap();

    assert_eq!(sample.headers, vec!["email"]);
    assert_eq!(sample.rows, vec![vec!["ada@example.com"]]);
}

#[test]
fn processes_csv_text() {
    let input = "email\nada@example.com\n";
    let sample = read_csv_sample_from_str(input, 10).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[0]);
    let (output, result) = process_csv_text(
        input,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    assert_eq!(result.row_count, 1);
    assert!(output.starts_with("email\n"));
    assert!(!output.contains("ada@example.com"));
}

#[test]
fn processes_selected_columns() {
    let input_path = fixture("sample.csv");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("sample-output.csv");
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    let result = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    assert_eq!(result.row_count, 5);
    let output = read_sample(&output_path, 100).unwrap();
    assert_eq!(output.headers, sample.headers);
    assert_eq!(output.rows[0][1], "[EMAIL]");
    assert_eq!(output.rows[0][0], sample.rows[0][0]);
}

#[test]
fn rejects_non_empty_fields_beyond_headers_without_committing_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("ragged.csv");
    let output_path = temp_dir.path().join("ragged-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com,unmodeled-secret\n2,b@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap_err();

    assert!(sample.to_string().contains("CSV privacy error"));

    let columns = vec![
        ColumnMetadata {
            header_label_is_ambiguous: false,
            name: "id".to_string(),
            source_path: None,
            index: 0,
            detected_type: crate::types::DataType::NumericId,
            confidence: crate::types::Confidence::High,
            detection_trace: None,
            privacy_findings: Vec::new(),
            privacy_evidence: Vec::new(),
            pii_risk: crate::types::PiiRisk::High,
            sample_values: vec![],
            sample_value_distribution: Default::default(),
            empty_format: crate::types::EmptyFormat::EmptyString,
            is_selected: false,
            strategy: crate::types::AnonymizationStrategy::Auto,
        },
        ColumnMetadata {
            header_label_is_ambiguous: false,
            name: "email".to_string(),
            source_path: None,
            index: 1,
            detected_type: crate::types::DataType::Email,
            confidence: crate::types::Confidence::High,
            detection_trace: None,
            privacy_findings: Vec::new(),
            privacy_evidence: Vec::new(),
            pii_risk: crate::types::PiiRisk::High,
            sample_values: vec![],
            sample_value_distribution: Default::default(),
            empty_format: crate::types::EmptyFormat::EmptyString,
            is_selected: true,
            strategy: crate::types::AnonymizationStrategy::Auto,
        },
    ];

    let error = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap_err();

    assert!(error.to_string().contains("non-header field"));
    assert!(!output_path.exists());
}

#[test]
fn pads_short_rows_and_truncates_empty_extra_cells() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("short-rows.csv");
    let output_path = temp_dir.path().join("short-rows-output.csv");
    fs::write(
        &input_path,
        "id,email,city\n1,a@example.com\n2,b@example.com,NL,,\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    let output = read_sample(&output_path, 100).unwrap();
    assert_eq!(output.rows[0].len(), 3);
    assert_eq!(output.rows[0][2], "");
    assert_eq!(output.rows[1].len(), 3);
    assert_eq!(output.rows[1][2], "NL");
}

#[test]
fn neutralizes_formula_like_headers_and_cells_in_standard_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("formula.csv");
    let output_path = temp_dir.path().join("formula-output.csv");
    fs::write(
        &input_path,
        "=name,email\n=cmd,a@example.com\n  +SUM(1 1),b@example.com\n\tTabbed,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    let output = read_sample(&output_path, 100).unwrap();
    assert_eq!(output.headers[0], "'=name");
    assert_eq!(output.rows[0][0], "'=cmd");
    assert_eq!(output.rows[1][0], "'  +SUM(1 1)");
    assert_eq!(output.rows[2][0], "'\tTabbed");
}

#[test]
fn neutralizes_full_width_formula_prefixes() {
    assert_eq!(neutralize_spreadsheet_formula("＝cmd").as_ref(), "'＝cmd");
    assert_eq!(
        neutralize_spreadsheet_formula("＋SUM(1 1)").as_ref(),
        "'＋SUM(1 1)"
    );
    assert_eq!(neutralize_spreadsheet_formula("－10").as_ref(), "'－10");
    assert_eq!(neutralize_spreadsheet_formula("＠cmd").as_ref(), "'＠cmd");
    assert_eq!(
        neutralize_spreadsheet_formula("\u{3000}＋SUM(1 1)").as_ref(),
        "'\u{3000}＋SUM(1 1)"
    );
    assert_eq!(
        neutralize_spreadsheet_formula("ordinary text").as_ref(),
        "ordinary text"
    );
}

#[test]
fn process_row_count_skips_blank_data_rows_but_preserves_them() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("blank-rows.csv");
    let output_path = temp_dir.path().join("blank-rows-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com\n,\n2,b@example.com\n   ,   \n3,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    let result = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            mapping_entry_ceiling: None,
        },
    )
    .unwrap();

    assert_eq!(count_csv_data_rows(&input_path).unwrap(), 3);
    assert_eq!(result.row_count, 3);
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("\n,\n"));
    assert!(output.contains("\n   ,   \n"));
}

#[test]
fn process_control_reports_progress_and_cancels_before_next_row() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("cancel.csv");
    let output_path = temp_dir.path().join("cancel-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com\n,\n2,b@example.com\n3,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);
    let last_progress = std::cell::Cell::new(0);
    let mut progress_events = Vec::new();
    let error = {
        let mut on_progress = |progress: ProcessProgress| {
            last_progress.set(progress.rows_processed);
            progress_events.push(progress.rows_processed);
        };
        let should_cancel = || last_progress.get() >= 2;
        let mut control = ProcessControl {
            on_progress: Some(&mut on_progress),
            should_cancel: Some(&should_cancel),
        };

        process_file_with_control(
            &input_path,
            &output_path,
            &columns,
            ProcessOptions {
                smart_replacements: None,
                mapping_entry_ceiling: None,
            },
            Some(&mut control),
        )
        .unwrap_err()
    };

    assert!(matches!(error, AnonymizerError::Canceled));
    assert_eq!(progress_events, vec![1, 2]);
    assert!(!output_path.exists());
}

#[test]
fn process_control_cancels_after_final_progress_before_output_commit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("cancel-final.csv");
    let output_path = temp_dir.path().join("cancel-final-output.csv");
    fs::write(&input_path, "id,email\n1,a@example.com\n,\n").unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);
    let last_progress = std::cell::Cell::new(0);
    let error = {
        let mut on_progress =
            |progress: ProcessProgress| last_progress.set(progress.rows_processed);
        let should_cancel = || last_progress.get() >= 1;
        let mut control = ProcessControl {
            on_progress: Some(&mut on_progress),
            should_cancel: Some(&should_cancel),
        };

        process_file_with_control(
            &input_path,
            &output_path,
            &columns,
            ProcessOptions {
                smart_replacements: None,
                mapping_entry_ceiling: None,
            },
            Some(&mut control),
        )
        .unwrap_err()
    };

    assert!(matches!(error, AnonymizerError::Canceled));
    assert!(!output_path.exists());
}

#[test]
fn plain_signed_numbers_are_not_neutralized() {
    assert_eq!(neutralize_spreadsheet_formula("-42.50").as_ref(), "-42.50");
    assert_eq!(neutralize_spreadsheet_formula("+31").as_ref(), "+31");
    assert_eq!(neutralize_spreadsheet_formula(" -7 ").as_ref(), " -7 ");
}

#[test]
fn signed_non_numeric_values_are_still_neutralized() {
    assert_eq!(neutralize_spreadsheet_formula("-2+3").as_ref(), "'-2+3");
    assert_eq!(neutralize_spreadsheet_formula("-1.2.3").as_ref(), "'-1.2.3");
    assert_eq!(
        neutralize_spreadsheet_formula("+cmd|calc").as_ref(),
        "'+cmd|calc"
    );
    assert_eq!(neutralize_spreadsheet_formula("－10").as_ref(), "'－10");
}

/// The mapping ceiling has to be *consulted by the run loop*, not merely defined.
///
/// `TransformState::check_mapping_budget_against` can be perfectly correct in
/// isolation while nothing ever calls it, and that failure mode is invisible in the
/// worst way: every unit test passes, the error message is well worded, the README
/// describes a guard — and a large run is still killed by the operating system with no
/// explanation, because the check sits in unreachable code. This test fails if the
/// call is removed from the loop, which no test of the method itself can do.
///
/// The real ceiling stands for roughly 5 GB of mapping, far past what a test can
/// build, which is the whole reason `ProcessOptions::mapping_entry_ceiling` exists.
///
/// It also asserts the destination is untouched, because refusing part-way through is
/// only an improvement on running out of memory if it does not leave a half-written
/// file behind for someone to mistake for a finished one.
#[test]
fn refuses_a_run_that_outgrows_its_mapping_ceiling_without_committing_output() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("wide-cardinality.csv");
    let output_path = temp_dir.path().join("wide-cardinality-output.csv");
    let mut text = String::from("id,email\n");
    for row in 0..40 {
        text.push_str(&format!("{row},person{row}@example.com\n"));
    }
    fs::write(&input_path, &text).unwrap();

    let sample = read_sample(&input_path, 100).unwrap();
    let mut columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);
    // Pseudonymize explicitly. Left on `Auto` this column is High risk and so defaults
    // to Redact, which holds no mapping at all — the run then finishes cleanly at any
    // ceiling, and the test would pass while proving nothing about the guard.
    columns[1].strategy = crate::types::AnonymizationStrategy::Pseudonymize;

    let error = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            smart_replacements: None,
            // Every row here introduces a distinct value, so a ceiling this small is
            // passed within the first few rows: the refusal lands mid-file, which is
            // the case that matters for the output-commit assertion below.
            mapping_entry_ceiling: Some(4),
        },
    )
    .unwrap_err();

    assert!(
        matches!(error, AnonymizerError::MappingBudgetExceeded { .. }),
        "expected the mapping ceiling to refuse the run, got {error:?}"
    );
    assert!(error.to_string().contains("No output was written"));
    assert!(!output_path.exists());
}
