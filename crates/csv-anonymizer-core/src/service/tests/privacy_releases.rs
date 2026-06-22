use super::*;

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
            preview_smart_replacements: vec![],
            privacy_config: None,
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
            preview_smart_replacements: vec![],
            privacy_config: None,
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
            deterministic: true,
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
fn differential_privacy_aggregate_release_writes_noisy_group_counts() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp.csv");
    let output_path = temp_dir.path().join("dp-output.csv");
    fs::write(&input_path, "region,amount\nA,10\nA,12\nB,5\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0, 1],
            controls: vec![],
            deterministic: true,
            seed: "dp-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 0,
                    role: ColumnRole::QuasiIdentifier,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: Some(0),
                    value_column: None,
                    lower_bound: None,
                    upper_bound: None,
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();

    assert_eq!(
        result.privacy_report.release_mode,
        ReleaseMode::DifferentialPrivacyAggregate
    );
    assert_eq!(result.privacy_report.dp_epsilon.as_deref(), Some("1"));
    assert_eq!(
        output.headers,
        vec!["region", "aggregate", "noisyValue", "epsilon"]
    );
    assert_eq!(output.rows.len(), 2);
    assert!(output.rows.iter().all(|row| row[1] == "count"));
}

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
    assert_eq!(output.rows.len(), 3);
    assert!(
        output
            .rows
            .iter()
            .all(|row| row[0].ends_with("@example.invalid"))
    );
}
