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
