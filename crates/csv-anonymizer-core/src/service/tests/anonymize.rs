use super::*;

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
            force: false,
            preview_smart_replacements: vec![],
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
                    force: false,
                    preview_smart_replacements: vec![],
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
                force: false,
                preview_smart_replacements: vec![],
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
                ColumnControl {
                    column_index: 4,
                    type_override: None,
                    strategy: AnonymizationStrategy::Auto,
                },
            ],
            force: false,
            preview_smart_replacements: vec![],
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
    assert_eq!(output.rows[0][3].len(), "'-12.50".len());
    assert!(output.rows[0][3].starts_with("'-"));
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
            force: false,
            preview_smart_replacements: vec![],
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
            force: false,
            preview_smart_replacements: vec![],
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
fn anonymize_reuses_repeated_values_in_single_output() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("repeated-values.csv");
    let output_path = temp_dir.path().join("repeated-values-output.csv");
    fs::write(
        &input_path,
        "first_name,last_name,email\nAlice,Smith,alice@example.com\nBianca,Jones,bianca@example.com\nAlice,Smith,alice@example.com\n",
    )
    .unwrap();

    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path.clone(),
            output_path: output_path.clone(),
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
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], output.rows[2][0]);
    assert_eq!(output.rows[0][1], output.rows[2][1]);
    assert_eq!(output.rows[0][2], output.rows[2][2]);
    assert_ne!(output.rows[0][0], output.rows[1][0]);
    assert_ne!(output.rows[0][1], output.rows[1][1]);
    assert_ne!(output.rows[0][2], output.rows[1][2]);
}

#[test]
fn anonymize_applies_pass_through_control() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("pass-through.csv");
    let output_path = temp_dir.path().join("pass-through-output.csv");
    fs::write(&input_path, "email\nuser@example.com\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::PassThrough,
            }],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    assert_eq!(result.columns_anonymized, 0);
    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], "user@example.com");
}

#[test]
fn anonymize_does_not_count_auto_noop_selected_columns() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("noop-count.csv");
    let output_path = temp_dir.path().join("noop-count-output.csv");
    fs::write(
        &input_path,
        "email,country,status\nuser@example.com,US,active\n",
    )
    .unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1, 2],
            controls: vec![
                ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::Email),
                    strategy: AnonymizationStrategy::PassThrough,
                },
                ColumnControl {
                    column_index: 1,
                    type_override: Some(DataType::CountryCode),
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 2,
                    type_override: Some(DataType::String),
                    strategy: AnonymizationStrategy::Mask,
                },
            ],
            force: false,
            preview_smart_replacements: vec![],
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(result.columns_anonymized, 1);
    assert_eq!(output.rows[0][0], "user@example.com");
    assert_eq!(output.rows[0][1], "US");
    assert_ne!(output.rows[0][2], "active");
}
