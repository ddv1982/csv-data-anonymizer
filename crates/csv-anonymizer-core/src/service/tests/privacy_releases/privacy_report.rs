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
            deterministic: false,
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
            deterministic: false,
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
