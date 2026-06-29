use super::*;

#[test]
fn synthetic_data_release_generates_new_rows_and_placeholders_for_direct_ids() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("synthetic.csv");
    let output_path = temp_dir.path().join("synthetic-output.csv");
    fs::write(
        &input_path,
        "email,country,status\nalice@example.com,US,active\nbob@example.com,NL,inactive\n",
    )
    .unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1, 2],
            controls: vec![],
            deterministic: true,
            seed: "synthetic-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::SyntheticData,
                column_roles: vec![
                    PrivacyColumnRole {
                        column_index: 0,
                        role: ColumnRole::DirectIdentifier,
                        generalization_level: 0,
                    },
                    PrivacyColumnRole {
                        column_index: 1,
                        role: ColumnRole::QuasiIdentifier,
                        generalization_level: 0,
                    },
                    PrivacyColumnRole {
                        column_index: 2,
                        role: ColumnRole::Sensitive,
                        generalization_level: 0,
                    },
                ],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig {
                    row_count: Some(3),
                    epsilon: None,
                },
            }),
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(
        result.privacy_report.release_mode,
        ReleaseMode::SyntheticData
    );
    assert_eq!(result.privacy_report.synthetic_rows, 3);
    assert_eq!(result.privacy_report.dp_budget, None);
    assert_eq!(result.privacy_report.sensitive_columns, 1);
    assert_eq!(result.privacy_report.pseudonymized_columns, 2);
    assert_eq!(result.privacy_report.generalized_columns, 1);
    assert_eq!(output.rows.len(), 3);
    assert!(
        output
            .rows
            .iter()
            .all(|row| row[0].ends_with("@example.invalid"))
    );
    assert!(
        output
            .rows
            .iter()
            .all(|row| row[2].starts_with("synthetic-sensitive-"))
    );
    assert!(
        output
            .rows
            .iter()
            .all(|row| row[2] != "active" && row[2] != "inactive")
    );
}

#[test]
fn synthetic_data_release_does_not_copy_attribute_source_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("synthetic-attribute.csv");
    let output_path = temp_dir.path().join("synthetic-attribute-output.csv");
    fs::write(
        &input_path,
        "email,status\nalice@example.com,raw-active\nbob@example.com,raw-inactive\n",
    )
    .unwrap();

    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1],
            controls: vec![ColumnControl {
                column_index: 1,
                type_override: Some(DataType::String),
                strategy: AnonymizationStrategy::Auto,
            }],
            deterministic: true,
            seed: "synthetic-attribute-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::SyntheticData,
                column_roles: vec![
                    PrivacyColumnRole {
                        column_index: 0,
                        role: ColumnRole::DirectIdentifier,
                        generalization_level: 0,
                    },
                    PrivacyColumnRole {
                        column_index: 1,
                        role: ColumnRole::Attribute,
                        generalization_level: 0,
                    },
                ],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig {
                    row_count: Some(4),
                    epsilon: None,
                },
            }),
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    let statuses = output
        .rows
        .iter()
        .map(|row| row[1].as_str())
        .collect::<Vec<_>>();

    assert!(
        statuses
            .iter()
            .all(|value| value.starts_with("synthetic-attribute-"))
    );
    assert!(!statuses.contains(&"raw-active"));
    assert!(!statuses.contains(&"raw-inactive"));
}

#[test]
fn synthetic_data_release_uses_column_identity_for_numeric_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("synthetic-numeric.csv");
    let output_path = temp_dir.path().join("synthetic-numeric-output.csv");
    fs::write(
        &input_path,
        "id,amount\n1,100\n2,200\n3,300\n4,400\n5,500\n6,600\n",
    )
    .unwrap();

    run_numeric_synthetic_release(
        &service,
        &input_path,
        &output_path,
        "synthetic-column-key-seed",
    );

    let output = read_sample(&output_path, 10).unwrap();
    let rows_with_amount_inside_id_bucket = output
        .rows
        .iter()
        .filter(|row| generalized_range_contains(&row[0], &row[1]))
        .count();

    assert_eq!(output.rows.len(), 6);
    assert!(
        output.rows.iter().all(|row| row[0].starts_with('[')),
        "expected numeric id quasi-identifiers to be generalized into ranges"
    );
    assert!(
        rows_with_amount_inside_id_bucket < output.rows.len(),
        "numeric columns should not share the same row-level synthetic key"
    );
}

#[test]
fn synthetic_data_release_is_repeatable_with_same_schema_and_seed() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("synthetic-repeatable.csv");
    let first_output_path = temp_dir.path().join("synthetic-repeatable-a.csv");
    let second_output_path = temp_dir.path().join("synthetic-repeatable-b.csv");
    fs::write(&input_path, "id,amount\n1,100\n2,200\n3,300\n").unwrap();

    run_numeric_synthetic_release(
        &service,
        &input_path,
        &first_output_path,
        "synthetic-repeatable-seed",
    );
    run_numeric_synthetic_release(
        &service,
        &input_path,
        &second_output_path,
        "synthetic-repeatable-seed",
    );

    let first_output = read_sample(&first_output_path, 10).unwrap();
    let second_output = read_sample(&second_output_path, 10).unwrap();

    assert_eq!(first_output.headers, second_output.headers);
    assert_eq!(first_output.rows, second_output.rows);
}

#[test]
fn synthetic_data_release_generates_valid_mac_addresses_and_timestamps() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("synthetic-structured.csv");
    let output_path = temp_dir.path().join("synthetic-structured-output.csv");
    fs::write(
        &input_path,
        "mac,created_at\n00:11:22:33:44:55,2024-01-01T00:00:00Z\n66:77:88:99:aa:bb,2024-01-02T00:00:00Z\n",
    )
    .unwrap();

    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1],
            controls: vec![
                ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::MacAddress),
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 1,
                    type_override: Some(DataType::Timestamp),
                    strategy: AnonymizationStrategy::Auto,
                },
            ],
            deterministic: true,
            seed: "synthetic-structured-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::SyntheticData,
                column_roles: vec![
                    PrivacyColumnRole {
                        column_index: 0,
                        role: ColumnRole::Attribute,
                        generalization_level: 0,
                    },
                    PrivacyColumnRole {
                        column_index: 1,
                        role: ColumnRole::Attribute,
                        generalization_level: 0,
                    },
                ],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig {
                    row_count: None,
                    epsilon: None,
                },
            }),
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(output.rows.len(), 2);
    assert!(
        output
            .rows
            .iter()
            .all(|row| is_valid_synthetic_mac_address(&row[0])),
        "expected generated MAC addresses to use six two-digit hex octets"
    );
    assert!(
        output
            .rows
            .iter()
            .all(|row| is_valid_synthetic_timestamp(&row[1])),
        "expected generated timestamps to be valid RFC3339 placeholders"
    );
    assert!(
        output
            .rows
            .iter()
            .all(|row| row[1] != "2000-01-01T00:00:00Z"),
        "timestamps should be generated from the column-scoped synthetic key"
    );
}

#[test]
fn synthetic_data_release_rejects_unselected_columns() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("synthetic-unselected.csv");
    let output_path = temp_dir.path().join("synthetic-unselected-output.csv");
    fs::write(
        &input_path,
        "email,country\nalice@example.com,US\nbob@example.com,NL\n",
    )
    .unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![1],
            controls: vec![],
            deterministic: true,
            seed: "synthetic-unselected-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::SyntheticData,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 1,
                    role: ColumnRole::QuasiIdentifier,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig {
                    row_count: Some(2),
                    epsilon: None,
                },
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("every column"));
    assert!(error.to_string().contains("unselected source columns"));
}

#[test]
fn synthetic_data_release_rejects_requested_epsilon_without_dp_generator() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("synthetic-epsilon.csv");
    let output_path = temp_dir.path().join("synthetic-epsilon-output.csv");
    fs::write(
        &input_path,
        "email,country\nalice@example.com,US\nbob@example.com,NL\n",
    )
    .unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0, 1],
            controls: vec![],
            deterministic: true,
            seed: "synthetic-epsilon-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::SyntheticData,
                column_roles: vec![
                    PrivacyColumnRole {
                        column_index: 0,
                        role: ColumnRole::DirectIdentifier,
                        generalization_level: 0,
                    },
                    PrivacyColumnRole {
                        column_index: 1,
                        role: ColumnRole::QuasiIdentifier,
                        generalization_level: 0,
                    },
                ],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig {
                    row_count: Some(2),
                    epsilon: Some(0.75),
                },
            }),
        })
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("synthetic DP epsilon is not supported")
    );
    assert!(error.to_string().contains("DP synthetic-data generator"));
}

fn run_numeric_synthetic_release(
    service: &AnonymizerService,
    input_path: &std::path::Path,
    output_path: &std::path::Path,
    seed: &str,
) {
    service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path.to_path_buf(),
            output_path: output_path.to_path_buf(),
            columns: vec![0, 1],
            controls: vec![
                ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::NumericId),
                    strategy: AnonymizationStrategy::Auto,
                },
                ColumnControl {
                    column_index: 1,
                    type_override: Some(DataType::NumericValue),
                    strategy: AnonymizationStrategy::Auto,
                },
            ],
            deterministic: true,
            seed: seed.to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::SyntheticData,
                column_roles: vec![
                    PrivacyColumnRole {
                        column_index: 0,
                        role: ColumnRole::QuasiIdentifier,
                        generalization_level: 0,
                    },
                    PrivacyColumnRole {
                        column_index: 1,
                        role: ColumnRole::Attribute,
                        generalization_level: 0,
                    },
                ],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig {
                    row_count: None,
                    epsilon: None,
                },
            }),
        })
        .unwrap();
}

fn generalized_range_contains(range: &str, value: &str) -> bool {
    let Some(range) = range
        .strip_prefix('[')
        .and_then(|range| range.strip_suffix(']'))
    else {
        return false;
    };
    let Some((lower, upper)) = range.split_once('-') else {
        return false;
    };
    let (Ok(lower), Ok(upper), Ok(value)) = (
        lower.parse::<i64>(),
        upper.parse::<i64>(),
        value.parse::<i64>(),
    ) else {
        return false;
    };
    value >= lower && value <= upper
}

fn is_valid_synthetic_mac_address(value: &str) -> bool {
    let parts = value.split(':').collect::<Vec<_>>();
    parts.len() == 6
        && parts[0] == "02"
        && parts[1] == "00"
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.chars().all(|char| char.is_ascii_hexdigit()))
}

fn is_valid_synthetic_timestamp(value: &str) -> bool {
    if value.len() != 20
        || value.get(4..5) != Some("-")
        || value.get(7..8) != Some("-")
        || value.get(10..11) != Some("T")
        || value.get(13..14) != Some(":")
        || value.get(16..17) != Some(":")
        || value.get(19..20) != Some("Z")
    {
        return false;
    }

    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        parse_timestamp_part(value, 0, 4),
        parse_timestamp_part(value, 5, 7),
        parse_timestamp_part(value, 8, 10),
        parse_timestamp_part(value, 11, 13),
        parse_timestamp_part(value, 14, 16),
        parse_timestamp_part(value, 17, 19),
    ) else {
        return false;
    };

    (2000..=2029).contains(&year)
        && (1..=12).contains(&month)
        && (1..=28).contains(&day)
        && (0..=23).contains(&hour)
        && (0..=59).contains(&minute)
        && (0..=59).contains(&second)
}

fn parse_timestamp_part(value: &str, start: usize, end: usize) -> Option<i64> {
    value.get(start..end)?.parse().ok()
}
