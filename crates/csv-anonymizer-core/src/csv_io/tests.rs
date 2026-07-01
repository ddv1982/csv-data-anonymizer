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
            empty_format: crate::types::EmptyFormat::EmptyString,
            is_selected: false,
            strategy: crate::types::AnonymizationStrategy::Auto,
        },
        ColumnMetadata {
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
