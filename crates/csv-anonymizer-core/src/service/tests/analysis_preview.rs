use super::*;

#[test]
fn analyzes_csv_headers_and_default_output_path() {
    let service = AnonymizerService::new("test-version");
    let result = service.analyze_csv(fixture("sample.csv")).unwrap();

    assert_eq!(result.row_count, 5);
    assert!(result.row_count_is_complete);
    assert!(
        result
            .default_output_path
            .ends_with("sample_private_output.csv")
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
fn preview_reuses_repeated_values_within_one_run() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("repeated-preview-values.csv");
    fs::write(
        &input_path,
        "email\nada@example.com\nada@example.com\ngrace@example.com\n",
    )
    .unwrap();

    let preview = service
        .preview_anonymization(PreviewParams {
            file_path: input_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::Email),
                strategy: AnonymizationStrategy::Auto,
            }],
            sample_count: 3,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples.len(), 3);
    assert_eq!(
        preview.previews[0].samples[0].anonymized,
        preview.previews[0].samples[1].anonymized
    );
    assert_ne!(
        preview.previews[0].samples[0].anonymized,
        preview.previews[0].samples[2].anonymized
    );
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
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::Auto,
            }],
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
            controls: vec![
                ColumnControl {
                    column_index: 0,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 1,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 2,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 3,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
            ],
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
            controls: vec![ColumnControl {
                column_index: 2,
                type_override: None,
                strategy: AnonymizationStrategy::Auto,
            }],
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
            controls: vec![
                ColumnControl {
                    column_index: 0,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 1,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 2,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 3,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
            ],
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
fn preview_name_mappings_are_consistent_within_previewed_rows() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("preview-full-names.csv");
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
            sample_count: 2,
        })
        .unwrap();

    for row_index in 0..2 {
        assert_eq!(
            preview.previews[2].samples[row_index].anonymized,
            format!(
                "{} {}",
                preview.previews[0].samples[row_index].anonymized,
                preview.previews[1].samples[row_index].anonymized
            )
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
            sample_count: 1,
        })
        .unwrap();

    assert_eq!(preview.previews[0].samples[0].anonymized, "***");
    assert!(preview.warnings.is_empty());
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
