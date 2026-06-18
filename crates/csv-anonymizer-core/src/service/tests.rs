use super::*;
use crate::types::{AnonymizationStrategy, ColumnControl, DataType};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn analyzes_csv_headers_and_default_output_path() {
    let service = AnonymizerService::new("test-version");
    let result = service.analyze_csv(fixture("sample.csv")).unwrap();

    assert_eq!(result.row_count, 5);
    assert!(result.row_count_is_complete);
    assert!(
        result
            .default_output_path
            .ends_with("sample_anonymized.csv")
    );
    assert_eq!(result.columns[1].name, "email");
}

#[test]
fn sampled_analysis_defers_full_row_count() {
    let service = AnonymizerService::new("test-version");
    let result = service
        .analyze_csv_sampled(fixture("large.csv"), 25)
        .unwrap();

    assert_eq!(result.row_count, 25);
    assert!(!result.row_count_is_complete);
    assert_eq!(
        service.count_csv_rows(fixture("large.csv")).unwrap(),
        10_500
    );
}

#[test]
fn previews_are_deterministic() {
    let service = AnonymizerService::new("test-version");
    let params = PreviewParams {
        file_path: fixture("sample.csv"),
        columns: vec![1],
        controls: vec![],
        deterministic: true,
        seed: "preview-seed".to_string(),
        sample_count: 2,
    };

    let first = service.preview_anonymization(params.clone()).unwrap();
    let second = service.preview_anonymization(params).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.previews[0].samples.len(), 2);
}

#[test]
fn anonymizes_selected_columns_without_web_runtime() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("sample-anonymized.csv");

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: fixture("sample.csv"),
            output_path: output_path.clone(),
            columns: vec![1],
            controls: vec![],
            deterministic: true,
            seed: "service-seed".to_string(),
            force: false,
        })
        .unwrap();

    assert_eq!(result.output_path, output_path);
    assert_eq!(result.row_count, 5);
    assert_eq!(result.columns_anonymized, 1);
}

#[test]
fn anonymize_csv_with_control_reports_progress() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("sample-anonymized.csv");
    let mut progress_events = Vec::new();
    let result = {
        let mut on_progress = |progress: crate::types::ProcessProgress| {
            progress_events.push(progress.rows_processed);
        };
        let mut control = ProcessControl {
            on_progress: Some(&mut on_progress),
            should_cancel: None,
        };

        service
            .anonymize_csv_with_control(
                AnonymizeParams {
                    file_path: fixture("sample.csv"),
                    output_path: output_path.clone(),
                    columns: vec![1],
                    controls: vec![],
                    deterministic: true,
                    seed: "service-seed".to_string(),
                    force: false,
                },
                &mut control,
            )
            .unwrap()
    };

    assert_eq!(result.row_count, 5);
    assert_eq!(progress_events, vec![1, 2, 3, 4, 5]);
}

#[test]
fn selected_sample_empty_columns_transform_later_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("sparse.csv");
    let output_path = temp_dir.path().join("sparse-anonymized.csv");
    fs::write(&input_path, "id,secret\n1,\n2,\n3,late-secret\n").unwrap();

    let result = service
        .anonymize_csv_with_sample_rows(
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![1],
                controls: vec![],
                deterministic: true,
                seed: "sparse-seed".to_string(),
                force: false,
            },
            2,
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(result.row_count, 3);
    assert_eq!(output.rows[2][0], "3");
    assert_ne!(output.rows[2][1], "late-secret");
    assert!(!output.rows[2][1].is_empty());
}

#[test]
fn preview_preserves_short_numeric_code_shape() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("numeric-looking.csv");
    fs::write(&input_path, "code\n1\n2\n3\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![],
            deterministic: true,
            seed: "numeric-string-seed".to_string(),
            sample_count: 3,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 3);
    assert!(preview.previews[0].samples.iter().all(|sample| {
        sample
            .anonymized
            .chars()
            .all(|character| character.is_ascii_digit())
    }));
}

#[test]
fn preview_preserves_decimal_numeric_shape() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("decimal-values.csv");
    fs::write(&input_path, "amount\n-12.50\n0.00\n42.75\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![],
            deterministic: true,
            seed: "decimal-seed".to_string(),
            sample_count: 3,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 3);
    assert!(
        preview.previews[0]
            .samples
            .iter()
            .all(|sample| sample.anonymized.parse::<f64>().is_ok())
    );
    assert_eq!(
        preview.previews[0].samples[0].anonymized.len(),
        "-12.50".len()
    );
    assert!(preview.previews[0].samples[0].anonymized.starts_with('-'));
}

#[test]
fn anonymize_preserves_numeric_shapes_in_output_file() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("numeric-shapes.csv");
    let output_path = temp_dir.path().join("numeric-shapes-anonymized.csv");
    fs::write(
        &input_path,
        "id,code,padded,amount,sparse\n1,7,0001,-12.50,\n2,8,0002,0.00,null\n3,9,0010,42.75,123\n",
    )
    .unwrap();

    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1, 2, 3, 4],
            controls: vec![],
            deterministic: true,
            seed: "numeric-output-seed".to_string(),
            force: false,
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(output.rows[0][0].len(), 1);
    assert!(
        output.rows[0][0]
            .chars()
            .all(|character| character.is_ascii_digit())
    );
    assert_eq!(output.rows[0][1].len(), 1);
    assert!(
        output.rows[0][1]
            .chars()
            .all(|character| character.is_ascii_digit())
    );
    assert_eq!(output.rows[0][2].len(), 4);
    assert!(output.rows[0][2].starts_with("000"));
    assert_eq!(output.rows[0][3].len(), "-12.50".len());
    assert!(output.rows[0][3].starts_with('-'));
    assert_eq!(output.rows[0][3].split_once('.').unwrap().1.len(), 2);
    assert_eq!(output.rows[0][4], "");
    assert_eq!(output.rows[1][4], "null");
    assert_eq!(output.rows[2][4].len(), 3);
    assert!(
        output.rows[2][4]
            .chars()
            .all(|character| character.is_ascii_digit())
    );
}

#[test]
fn preview_skips_empty_and_null_samples() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("empty-values.csv");
    fs::write(&input_path, "email\n\nnull\nuser@example.com\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![],
            deterministic: true,
            seed: "empty-seed".to_string(),
            sample_count: 3,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 1);
    assert_eq!(preview.previews[0].samples[0].original, "user@example.com");
}

#[test]
fn preview_uses_type_specific_phone_and_name_strategies() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("people.csv");
    fs::write(
        &input_path,
        "phone,first_name,last_name,full_name\n555-867-5309,Alice,Smith,Alice Smith\n",
    )
    .unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0, 1, 2, 3],
            controls: vec![],
            deterministic: true,
            seed: "people-seed".to_string(),
            sample_count: 1,
        })
        .unwrap();

    let phone = &preview.previews[0].samples[0].anonymized;
    let first = &preview.previews[1].samples[0].anonymized;
    let last = &preview.previews[2].samples[0].anonymized;
    let full = &preview.previews[3].samples[0].anonymized;

    assert_eq!(phone.len(), "555-867-5309".len());
    assert_eq!(
        phone.chars().filter(|character| *character == '-').count(),
        2
    );
    assert!(first.chars().all(|character| character.is_alphabetic()));
    assert!(last.chars().all(|character| character.is_alphabetic()));
    assert_eq!(full.split_whitespace().count(), 2);
}

#[test]
fn preview_applies_per_column_type_and_strategy_controls() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("controls.csv");
    fs::write(&input_path, "value\n123\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::Email),
                strategy: AnonymizationStrategy::Mask,
            }],
            deterministic: true,
            seed: "control-seed".to_string(),
            sample_count: 1,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples[0].anonymized, "***");
    assert!(preview.warnings.is_empty());
}

#[test]
fn anonymize_applies_pass_through_control() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("pass-through.csv");
    let output_path = temp_dir.path().join("pass-through-output.csv");
    fs::write(&input_path, "email\nuser@example.com\n").unwrap();

    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: true,
            seed: "pass-through-seed".to_string(),
            force: false,
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], "user@example.com");
}

#[test]
fn anonymize_returns_privacy_report() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("privacy-report.csv");
    let output_path = temp_dir.path().join("privacy-report-output.csv");
    fs::write(&input_path, "email,country\nuser@example.com,US\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0, 1],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::Mask,
            }],
            deterministic: true,
            seed: "privacy-report-seed".to_string(),
            force: false,
        })
        .unwrap();

    assert_eq!(result.privacy_report.direct_identifiers, 1);
    assert_eq!(result.privacy_report.quasi_identifiers, 1);
    assert_eq!(result.privacy_report.masked_columns, 1);
    assert_eq!(result.privacy_report.generalized_columns, 0);
    assert_eq!(result.privacy_report.pass_through_columns, 1);
    assert!(!result.privacy_report.notes.is_empty());

    let json = serde_json::to_value(&result).unwrap();
    assert!(json.get("privacyReport").is_some());
    assert_eq!(json["privacyReport"]["directIdentifiers"], 1);
    assert_eq!(json["privacyReport"]["quasiIdentifiers"], 1);
    assert_eq!(json["privacyReport"]["pseudonymizedColumns"], 0);
    assert_eq!(json["privacyReport"]["maskedColumns"], 1);
    assert_eq!(json["privacyReport"]["generalizedColumns"], 0);
    assert_eq!(json["privacyReport"]["passThroughColumns"], 1);
    assert!(
        json["privacyReport"]["notes"][0]
            .as_str()
            .unwrap()
            .contains("pseudonymization")
    );
}

#[test]
fn preview_warns_for_pass_through_and_no_op_columns() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("warnings.csv");
    fs::write(&input_path, "country,email\nUS,user@example.com\n").unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0, 1],
            controls: vec![ColumnControl {
                column_index: 1,
                type_override: None,
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: true,
            seed: "warning-seed".to_string(),
            sample_count: 1,
        })
        .unwrap();

    assert_eq!(preview.warnings.len(), 2);
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.column_index == 0 && warning.message.contains("pass-through"))
    );
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.column_index == 1 && warning.message.contains("unchanged"))
    );
}
