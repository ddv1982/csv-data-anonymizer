use super::*;

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
            deterministic: false,
            seed: "dp-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 0,
                    role: ColumnRole::Attribute,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: Some(0),
                    group_labels_public: true,
                    public_group_values: vec![
                        "A".to_string(),
                        "B".to_string(),
                        "C".to_string(),
                        "D".to_string(),
                    ],
                    value_column: None,
                    lower_bound: None,
                    upper_bound: None,
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig::default(),
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
    assert_eq!(result.privacy_report.dp_budget, None);
    assert_eq!(
        output.headers,
        vec!["region", "aggregate", "noisyValue", "epsilon"]
    );
    assert_eq!(result.row_count, 4);
    assert_eq!(output.rows.len(), 4);
    assert!(output.rows.iter().all(|row| row[1] == "count"));
    assert!(
        result
            .privacy_report
            .formal_models
            .iter()
            .any(|model| { model.message.contains("no local release history") })
    );
    assert!(
        result
            .privacy_report
            .notes
            .iter()
            .any(|note| note.contains("No local release history was provided"))
    );
}

#[test]
fn differential_privacy_release_rejects_unselected_group_by_column() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-unselected.csv");
    let output_path = temp_dir.path().join("dp-unselected-output.csv");
    fs::write(&input_path, "region,amount\nA,10\nB,5\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![1],
            controls: vec![],
            deterministic: false,
            seed: "dp-unselected-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: Some(0),
                    group_labels_public: true,
                    public_group_values: vec!["A".to_string(), "B".to_string()],
                    value_column: None,
                    lower_bound: None,
                    upper_bound: None,
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig::default(),
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("group_by_column"));
    assert!(error.to_string().contains("unselected column 0"));
}

#[test]
fn differential_privacy_budget_reports_cumulative_epsilon_when_enabled() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-budget.csv");
    let output_path = temp_dir.path().join("dp-budget-output.csv");
    fs::write(&input_path, "amount\n10\n12\n5\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::NumericValue),
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: false,
            seed: "dp-budget-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Sum,
                    group_by_column: None,
                    group_labels_public: false,
                    public_group_values: vec![],
                    value_column: Some(0),
                    lower_bound: Some(0.0),
                    upper_bound: Some(20.0),
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig {
                        enabled: true,
                        limit_epsilon: Some(3.0),
                        spent_epsilon: 1.25,
                        action: DpBudgetAction::Block,
                    },
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap();

    let budget = result.privacy_report.dp_budget.as_ref().unwrap();
    assert_eq!(budget.limit_epsilon, "3");
    assert_eq!(budget.spent_epsilon_before, "1.25");
    assert_eq!(budget.release_epsilon, "1");
    assert_eq!(budget.spent_epsilon_after, "2.25");
    assert_eq!(budget.remaining_epsilon, "0.75");
    assert_eq!(budget.status, DpBudgetStatus::WithinBudget);
    assert_eq!(budget.action, DpBudgetAction::Block);
    assert!(
        result
            .privacy_report
            .notes
            .iter()
            .any(|note| note.contains("Local DP budget"))
    );
}

#[test]
fn differential_privacy_budget_blocks_releases_over_limit() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-budget-block.csv");
    let output_path = temp_dir.path().join("dp-budget-block-output.csv");
    fs::write(&input_path, "amount\n10\n12\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0],
            controls: vec![],
            deterministic: false,
            seed: "dp-budget-block-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: None,
                    group_labels_public: false,
                    public_group_values: vec![],
                    value_column: None,
                    lower_bound: None,
                    upper_bound: None,
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig {
                        enabled: true,
                        limit_epsilon: Some(1.5),
                        spent_epsilon: 1.0,
                        action: DpBudgetAction::Block,
                    },
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("DP budget would be exceeded"));
    assert!(!output_path.exists());
}

#[test]
fn differential_privacy_budget_can_warn_over_limit() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-budget-warn.csv");
    let output_path = temp_dir.path().join("dp-budget-warn-output.csv");
    fs::write(&input_path, "amount\n10\n12\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![],
            deterministic: false,
            seed: "dp-budget-warn-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: None,
                    group_labels_public: false,
                    public_group_values: vec![],
                    value_column: None,
                    lower_bound: None,
                    upper_bound: None,
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig {
                        enabled: true,
                        limit_epsilon: Some(1.5),
                        spent_epsilon: 1.0,
                        action: DpBudgetAction::Warn,
                    },
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap();

    let budget = result.privacy_report.dp_budget.as_ref().unwrap();
    assert_eq!(budget.status, DpBudgetStatus::OverBudget);
    assert_eq!(budget.remaining_epsilon, "-0.5");
    assert!(
        result
            .privacy_report
            .notes
            .iter()
            .any(|note| note.contains("allowed because the budget action is warn"))
    );
}

#[test]
fn differential_privacy_release_requires_public_group_label_acknowledgement() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-private-group.csv");
    let output_path = temp_dir.path().join("dp-private-group-output.csv");
    fs::write(&input_path, "region,amount\nA,10\nB,5\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0, 1],
            controls: vec![],
            deterministic: false,
            seed: "dp-private-group-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 0,
                    role: ColumnRole::Attribute,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: Some(0),
                    group_labels_public: false,
                    public_group_values: vec!["A".to_string(), "B".to_string()],
                    value_column: None,
                    lower_bound: None,
                    upper_bound: None,
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig::default(),
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("group labels"));
    assert!(error.to_string().contains("public"));
}

#[test]
fn differential_privacy_release_rejects_non_attribute_group_role() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-quasi-group.csv");
    let output_path = temp_dir.path().join("dp-quasi-group-output.csv");
    fs::write(&input_path, "region,amount\nA,10\nB,5\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0, 1],
            controls: vec![],
            deterministic: false,
            seed: "dp-quasi-group-seed".to_string(),
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
                    group_labels_public: true,
                    public_group_values: vec!["A".to_string(), "B".to_string()],
                    value_column: None,
                    lower_bound: None,
                    upper_bound: None,
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig::default(),
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("role to Attribute"));
}

#[test]
fn differential_privacy_sum_rejects_non_numeric_value_column() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-invalid-number.csv");
    let output_path = temp_dir.path().join("dp-invalid-number-output.csv");
    fs::write(&input_path, "amount\n10\noops\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::NumericValue),
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: false,
            seed: "dp-invalid-number-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Sum,
                    group_by_column: None,
                    group_labels_public: false,
                    public_group_values: vec![],
                    value_column: Some(0),
                    lower_bound: Some(0.0),
                    upper_bound: Some(20.0),
                    privacy_unit_column: None,
                    max_contributions_per_unit: None,
                    budget: DpBudgetConfig::default(),
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("requires numeric values"));
    assert!(error.to_string().contains("oops"));
}

#[test]
fn differential_privacy_release_rejects_deterministic_noise() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-deterministic.csv");
    let output_path = temp_dir.path().join("dp-deterministic-output.csv");
    fs::write(&input_path, "amount\n10\n12\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![],
            deterministic: true,
            seed: "dp-deterministic-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    ..DifferentialPrivacyConfig::default()
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("deterministic output is not supported")
    );
}

#[test]
fn differential_privacy_count_rejects_value_column() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-count-value.csv");
    let output_path = temp_dir.path().join("dp-count-value-output.csv");
    fs::write(&input_path, "amount\nnot-a-number\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![],
            deterministic: false,
            seed: "dp-count-value-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    value_column: Some(0),
                    ..DifferentialPrivacyConfig::default()
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("count releases do not use a value column")
    );
}

#[test]
fn differential_privacy_grouped_release_requires_allowed_group_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-no-domain.csv");
    let output_path = temp_dir.path().join("dp-no-domain-output.csv");
    fs::write(&input_path, "region\nA\nB\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![],
            deterministic: false,
            seed: "dp-no-domain-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 0,
                    role: ColumnRole::Attribute,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: Some(0),
                    group_labels_public: true,
                    ..DifferentialPrivacyConfig::default()
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(error.to_string().contains("allowed group values"));
}

#[test]
fn differential_privacy_grouped_release_rejects_groups_outside_allowed_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-domain-miss.csv");
    let output_path = temp_dir.path().join("dp-domain-miss-output.csv");
    fs::write(&input_path, "region\nA\nB\n").unwrap();

    let error = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0],
            controls: vec![],
            deterministic: false,
            seed: "dp-domain-miss-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 0,
                    role: ColumnRole::Attribute,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: Some(0),
                    group_labels_public: true,
                    public_group_values: vec!["A".to_string()],
                    ..DifferentialPrivacyConfig::default()
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("not in the configured allowed group values")
    );
}

#[test]
fn differential_privacy_grouped_blank_input_releases_allowed_group_values_only() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-blank-groups.csv");
    let output_path = temp_dir.path().join("dp-blank-groups-output.csv");
    fs::write(&input_path, "region\n\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path: output_path.clone(),
            columns: vec![0],
            controls: vec![],
            deterministic: false,
            seed: "dp-blank-groups-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![PrivacyColumnRole {
                    column_index: 0,
                    role: ColumnRole::Attribute,
                    generalization_level: 0,
                }],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    group_by_column: Some(0),
                    group_labels_public: true,
                    public_group_values: vec!["A".to_string(), "B".to_string()],
                    ..DifferentialPrivacyConfig::default()
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    let groups = output
        .rows
        .iter()
        .map(|row| row[0].as_str())
        .collect::<Vec<_>>();

    assert_eq!(result.row_count, 2);
    assert_eq!(groups, vec!["A", "B"]);
}

#[test]
fn differential_privacy_release_reports_privacy_unit_contribution_bound() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("dp-unit-bound.csv");
    let output_path = temp_dir.path().join("dp-unit-bound-output.csv");
    fs::write(&input_path, "person,amount\np1,10\np1,12\np2,5\n").unwrap();

    let result = service
        .anonymize_csv(AnonymizeParams {
            file_path: input_path,
            output_path,
            columns: vec![0, 1],
            controls: vec![ColumnControl {
                column_index: 1,
                type_override: Some(DataType::NumericValue),
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: false,
            seed: "dp-unit-bound-seed".to_string(),
            force: false,
            preview_smart_replacements: vec![],
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                column_roles: vec![],
                formal: FormalPrivacyConfig::default(),
                differential_privacy: DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Sum,
                    value_column: Some(1),
                    lower_bound: Some(0.0),
                    upper_bound: Some(20.0),
                    privacy_unit_column: Some(0),
                    max_contributions_per_unit: Some(1),
                    ..DifferentialPrivacyConfig::default()
                },
                synthetic: SyntheticDataConfig::default(),
            }),
        })
        .unwrap();

    assert_eq!(result.columns_anonymized, 2);
    assert!(
        result
            .privacy_report
            .notes
            .iter()
            .any(|note| note.contains("at most 1 row contribution"))
    );
}
