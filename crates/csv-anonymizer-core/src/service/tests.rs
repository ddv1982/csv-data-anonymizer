use super::*;
use crate::smart::{SmartReplacement, SmartReplacementProvider, SmartReplacementRequest};
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
fn people_names_fixture_previews_name_like_full_names() {
    let service = AnonymizerService::new("test-version");

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: fixture("people-names.csv"),
            columns: vec![2],
            controls: vec![],
            deterministic: true,
            seed: "people-name-quality-seed".to_string(),
            sample_count: 5,
        })
        .unwrap();

    assert_eq!(preview.previews[0].column_name, "full_name");
    assert_eq!(preview.previews[0].samples.len(), 5);
    for sample in &preview.previews[0].samples {
        assert_ne!(sample.anonymized, sample.original);
        assert_eq!(
            sample.anonymized.split_whitespace().count(),
            sample.original.split_whitespace().count()
        );
        assert!(
            sample
                .anonymized
                .chars()
                .all(|character| character.is_alphabetic() || character.is_whitespace())
        );
        assert!(
            !sample
                .anonymized
                .chars()
                .any(|character| character.is_ascii_digit() || matches!(character, '_' | '-'))
        );
    }
}

#[test]
fn people_names_fixture_treats_single_token_name_column_as_name() {
    let service = AnonymizerService::new("test-version");

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: fixture("people-names.csv"),
            columns: vec![0, 1, 2, 3],
            controls: vec![],
            deterministic: true,
            seed: "people-name-quality-seed".to_string(),
            sample_count: 5,
        })
        .unwrap();

    assert_eq!(preview.previews[3].column_name, "name");

    for row_index in 0..preview.previews[3].samples.len() {
        let first = &preview.previews[0].samples[row_index].anonymized;
        let last = &preview.previews[1].samples[row_index].anonymized;
        let full = &preview.previews[2].samples[row_index].anonymized;
        let name = &preview.previews[3].samples[row_index].anonymized;
        let original_first = &preview.previews[0].samples[row_index].original;
        let original_last = &preview.previews[1].samples[row_index].original;
        let original_name = &preview.previews[3].samples[row_index].original;
        let original_tokens: Vec<&str> = original_first
            .split_whitespace()
            .chain(original_last.split_whitespace())
            .collect();

        assert_eq!(name, first);
        assert_eq!(full, &format!("{first} {last}"));
        assert_ne!(name, original_name);
        assert!(!full.split_whitespace().any(|token| {
            original_tokens
                .iter()
                .any(|original| token.eq_ignore_ascii_case(original))
        }));
        assert!(name.chars().all(|character| character.is_alphabetic()));
        assert!(
            !name
                .chars()
                .any(|character| character.is_ascii_digit() || matches!(character, '_' | '-'))
        );
    }
}

#[test]
fn anonymize_reuses_repeated_name_sources_in_random_mode() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("repeated-names.csv");
    let output_path = temp_dir.path().join("repeated-names-output.csv");
    fs::write(&input_path, "first_name\nAlice\nAlice\nBianca\n").unwrap();

    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::FirstName),
                strategy: AnonymizationStrategy::Auto,
            }],
            deterministic: false,
            seed: "random-repeat-seed".to_string(),
            force: false,
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], output.rows[1][0]);
    assert_ne!(output.rows[0][0], output.rows[2][0]);
}

#[test]
fn anonymize_random_mode_avoids_duplicate_names_for_distinct_sources() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("distinct-random-names.csv");
    let output_path = temp_dir.path().join("distinct-random-names-output.csv");
    fs::write(
        &input_path,
        "first_name\nAlice\nBianca\nCeline\nDaphne\nElise\nFreya\nGemma\nHelena\nIris\nJenna\nKeira\nLena\n",
    )
    .unwrap();

    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::FirstName),
                strategy: AnonymizationStrategy::Auto,
            }],
            deterministic: false,
            seed: "random-unique-seed".to_string(),
            force: false,
        })
        .unwrap();

    let output = read_sample(&output_path, 20).unwrap();
    let names = output
        .rows
        .iter()
        .map(|row| row[0].clone())
        .collect::<Vec<_>>();
    let unique_names = names.iter().collect::<std::collections::HashSet<_>>();

    assert_eq!(unique_names.len(), names.len());
}

#[test]
fn anonymize_deterministic_output_is_reproducible() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("deterministic-names.csv");
    let first_output_path = temp_dir.path().join("deterministic-names-output-a.csv");
    let second_output_path = temp_dir.path().join("deterministic-names-output-b.csv");
    fs::write(
        &input_path,
        "first_name,last_name,email\nAlice,Smith,alice@example.com\nBianca,Jones,bianca@example.com\nAlice,Smith,alice@example.com\n",
    )
    .unwrap();

    let params = |output_path: PathBuf| AnonymizeParams {
        file_path: input_path.clone(),
        output_path,
        columns: vec![0, 1, 2],
        controls: vec![
            ColumnControl {
                column_index: 0,
                type_override: Some(DataType::FirstName),
                strategy: AnonymizationStrategy::Auto,
            },
            ColumnControl {
                column_index: 1,
                type_override: Some(DataType::LastName),
                strategy: AnonymizationStrategy::Auto,
            },
            ColumnControl {
                column_index: 2,
                type_override: Some(DataType::Email),
                strategy: AnonymizationStrategy::Auto,
            },
        ],
        deterministic: true,
        seed: "deterministic-output-seed".to_string(),
        force: false,
    };

    service
        .anonymize_csv(params(first_output_path.clone()))
        .unwrap();
    service
        .anonymize_csv(params(second_output_path.clone()))
        .unwrap();

    assert_eq!(
        fs::read_to_string(first_output_path).unwrap(),
        fs::read_to_string(second_output_path).unwrap()
    );
}

#[test]
fn preview_name_mappings_match_full_output_for_previewed_rows() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("preview-full-names.csv");
    let output_path = temp_dir.path().join("preview-full-names-output.csv");
    fs::write(
        &input_path,
        "first_name,last_name,full_name\nAlice,Smith,Alice Smith\nBianca,Jones,Bianca Jones\n",
    )
    .unwrap();
    let controls = vec![
        ColumnControl {
            column_index: 0,
            type_override: Some(DataType::FirstName),
            strategy: AnonymizationStrategy::Auto,
        },
        ColumnControl {
            column_index: 1,
            type_override: Some(DataType::LastName),
            strategy: AnonymizationStrategy::Auto,
        },
        ColumnControl {
            column_index: 2,
            type_override: Some(DataType::FullName),
            strategy: AnonymizationStrategy::Auto,
        },
    ];

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path.clone(),
            columns: vec![0, 1, 2],
            controls: controls.clone(),
            deterministic: true,
            seed: "preview-full-seed".to_string(),
            sample_count: 2,
        })
        .unwrap();
    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1, 2],
            controls,
            deterministic: true,
            seed: "preview-full-seed".to_string(),
            force: false,
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    for row_index in 0..2 {
        assert_eq!(
            preview.previews[0].samples[row_index].anonymized,
            output.rows[row_index][0]
        );
        assert_eq!(
            preview.previews[1].samples[row_index].anonymized,
            output.rows[row_index][1]
        );
        assert_eq!(
            preview.previews[2].samples[row_index].anonymized,
            output.rows[row_index][2]
        );
    }
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
    assert_eq!(result.privacy_report.opaque_token_columns, 0);
    assert_eq!(result.privacy_report.unique_pseudonym_values, 0);
    assert_eq!(result.privacy_report.reused_pseudonym_values, 0);
    assert_eq!(result.privacy_report.collisions_avoided, 0);
    assert_eq!(result.privacy_report.exhausted_pseudonym_pools, 0);
    assert_eq!(result.privacy_report.opaque_token_values, 0);
    assert!(!result.privacy_report.notes.is_empty());

    let json = serde_json::to_value(&result).unwrap();
    assert!(json.get("privacyReport").is_some());
    assert_eq!(json["privacyReport"]["directIdentifiers"], 1);
    assert_eq!(json["privacyReport"]["quasiIdentifiers"], 1);
    assert_eq!(json["privacyReport"]["pseudonymizedColumns"], 0);
    assert_eq!(json["privacyReport"]["opaqueTokenColumns"], 0);
    assert_eq!(json["privacyReport"]["maskedColumns"], 1);
    assert_eq!(json["privacyReport"]["generalizedColumns"], 0);
    assert_eq!(json["privacyReport"]["passThroughColumns"], 1);
    assert_eq!(json["privacyReport"]["uniquePseudonymValues"], 0);
    assert_eq!(json["privacyReport"]["reusedPseudonymValues"], 0);
    assert_eq!(json["privacyReport"]["collisionsAvoided"], 0);
    assert_eq!(json["privacyReport"]["exhaustedPseudonymPools"], 0);
    assert_eq!(json["privacyReport"]["opaqueTokenValues"], 0);
    assert!(
        json["privacyReport"]["notes"][0]
            .as_str()
            .unwrap()
            .contains("pseudonymization")
    );
}

#[test]
fn tokenize_strategy_updates_privacy_report() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("token-report.csv");
    let output_path = temp_dir.path().join("token-report-output.csv");
    fs::write(&input_path, "email\nuser@example.com\nuser@example.com\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::Email),
                strategy: AnonymizationStrategy::Tokenize,
            }],
            deterministic: true,
            seed: "token-report-seed".to_string(),
            force: false,
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], output.rows[1][0]);
    assert!(output.rows[0][0].starts_with("tok_"));
    assert_eq!(result.privacy_report.opaque_token_columns, 1);
    assert_eq!(result.privacy_report.opaque_token_values, 1);
    assert_eq!(result.privacy_report.unique_pseudonym_values, 1);
    assert_eq!(result.privacy_report.reused_pseudonym_values, 1);

    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(json["privacyReport"]["opaqueTokenColumns"], 1);
    assert_eq!(json["privacyReport"]["opaqueTokenValues"], 1);
    assert_eq!(json["privacyReport"]["uniquePseudonymValues"], 1);
    assert_eq!(json["privacyReport"]["reusedPseudonymValues"], 1);
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

#[derive(Default)]
struct MockSmartProvider;

impl SmartReplacementProvider for MockSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        Ok(request
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| SmartReplacement {
                original: value.clone(),
                replacement: format!("Local AI {} {}", request.column.index, index + 1),
            })
            .collect())
    }
}

#[test]
fn preview_uses_local_ai_provider_for_smart_replacement_columns() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-preview.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();
    let mut provider = MockSmartProvider;

    let preview = service
        .preview_anonymization_with_smart_provider(
            PreviewParams {
                file_path: input_path,
                columns: vec![0],
                controls: vec![ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::FullName),
                    strategy: AnonymizationStrategy::LocalAi,
                }],
                deterministic: true,
                seed: "smart-preview-seed".to_string(),
                sample_count: 2,
            },
            Some(&mut provider),
        )
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 2);
    assert_eq!(preview.previews[0].samples[0].anonymized, "Local AI 0 1");
    assert!(
        preview
            .warnings
            .iter()
            .any(|warning| warning.message.contains("Local AI"))
    );
}

#[test]
fn anonymize_uses_local_ai_provider_and_reports_smart_replacements() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart.csv");
    let output_path = temp_dir.path().join("smart-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nAlice Smith\nBob Stone\n").unwrap();
    let mut provider = MockSmartProvider;

    let result = service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls: vec![ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::FullName),
                    strategy: AnonymizationStrategy::LocalAi,
                }],
                deterministic: true,
                seed: "smart-run-seed".to_string(),
                force: false,
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], output.rows[1][0]);
    assert_ne!(output.rows[0][0], output.rows[2][0]);
    assert_eq!(result.privacy_report.smart_replacement_columns, 1);
    assert_eq!(result.privacy_report.smart_replacement_values, 2);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 0);
}

#[test]
fn local_ai_strategy_requires_provider_before_processing() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-missing-provider.csv");
    fs::write(&input_path, "name\nAlice Smith\n").unwrap();

    let error = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::FullName),
                strategy: AnonymizationStrategy::LocalAi,
            }],
            deterministic: true,
            seed: "smart-error-seed".to_string(),
            sample_count: 1,
        })
        .unwrap_err();

    assert!(error.to_string().contains("Local AI"));
}
