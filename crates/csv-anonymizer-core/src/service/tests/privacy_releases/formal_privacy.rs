use super::*;

#[test]
fn formal_tabular_release_generalizes_and_reports_k_l_t_models() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("formal.csv");
    let output_path = temp_dir.path().join("formal-output.csv");
    fs::write(
        &input_path,
        "email,age,diagnosis\nalice@example.com,34,cold\nbob@example.com,36,flu\ncarol@example.com,52,cancer\n",
    )
    .unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1, 2],
            controls: vec![ColumnControl {
                column_index: 1,
                type_override: Some(DataType::NumericValue),
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: false,
            seed: "formal-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::FormalTabular,
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
                formal: FormalPrivacyConfig {
                    k: 2,
                    l_diversity: Some(2),
                    t_closeness: Some(0.75),
                    suppress_small_classes: true,
                },
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(
        result.privacy_report.release_mode,
        ReleaseMode::FormalTabular
    );
    assert_eq!(result.privacy_report.direct_identifiers, 1);
    assert_eq!(result.privacy_report.quasi_identifiers, 1);
    assert_eq!(result.privacy_report.sensitive_columns, 1);
    assert_eq!(result.privacy_report.generalized_columns, 1);
    assert_eq!(result.privacy_report.suppressed_rows, 0);
    assert_eq!(result.privacy_report.dp_budget, None);
    assert!(
        result
            .privacy_report
            .formal_models
            .iter()
            .any(|model| model.model == crate::types::PrivacyModel::KAnonymity && model.satisfied)
    );
    assert_eq!(output.rows.len(), 3);
    assert_eq!(output.rows[0][0], "[redacted]");
    assert_ne!(output.rows[0][1], "34");
}

#[test]
fn formal_tabular_release_rejects_unselected_columns() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("formal-selected.csv");
    let output_path = temp_dir.path().join("formal-selected-output.csv");
    fs::write(
        &input_path,
        "email,age\nalice@example.com,34\nbob@example.com,36\n",
    )
    .unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![1],
            controls: vec![ColumnControl {
                column_index: 1,
                type_override: Some(DataType::NumericValue),
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: true,
            seed: "formal-selected-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::FormalTabular,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 1,
                    role: ColumnRole::QuasiIdentifier,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig {
                    k: 1,
                    l_diversity: None,
                    t_closeness: None,
                    suppress_small_classes: false,
                },
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("formal tabular releases require every column")
    );
}

#[test]
fn privacy_release_rejects_late_non_empty_fields_beyond_headers_without_output() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("formal-ragged.csv");
    let output_path = temp_dir.path().join("formal-ragged-output.csv");
    fs::write(
        &input_path,
        "email,age\nalice@example.com,34\nbob@example.com,36,hidden-diagnosis\n",
    )
    .unwrap();

    let error = service
        .anonymize_csv_with_sample_rows(
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0, 1],
                controls: vec![ColumnControl {
                    column_index: 1,
                    type_override: Some(DataType::NumericValue),
                    strategy: AnonymizationStrategy::PassThrough,
                }],
                deterministic: true,
                seed: "formal-ragged-seed".to_string(),
                force: false,
                preview_smart_replacements: vec![],
                privacy_config: Some(PrivacyConfig {
                    release_mode: ReleaseMode::FormalTabular,
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
                    formal: FormalPrivacyConfig {
                        k: 1,
                        l_diversity: None,
                        t_closeness: None,
                        suppress_small_classes: false,
                    },
                    differential_privacy: DifferentialPrivacyConfig::default(),
                    synthetic: SyntheticDataConfig::default(),
                }),
            },
            1,
        )
        .unwrap_err();

    assert!(error.to_string().contains("CSV privacy error"));
    assert!(error.to_string().contains("non-header field"));
    assert!(!output_path.exists());
}

#[test]
fn privacy_release_rejects_unselected_role_columns() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("unselected-role.csv");
    let output_path = temp_dir.path().join("unselected-role-output.csv");
    fs::write(&input_path, "email,country\nuser@example.com,US\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![],
            deterministic: true,
            seed: "unselected-role-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::FormalTabular,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 1,
                    role: ColumnRole::Sensitive,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig::default(),
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("unselected column 1"));
    assert!(error.to_string().contains("column role"));
}
