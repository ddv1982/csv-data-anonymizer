use super::*;
use crate::types::{PreflightMode, PreflightParams, ReleaseReadinessStatus, SmartReplacementEntry};

#[test]
fn preflight_preview_does_not_require_output_path() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("preview.csv");
    fs::write(&input_path, "email\nada@example.com\n").unwrap();

    let result = service
        .preflight_anonymization(PreflightParams {
            file_path: input_path,
            mode: PreflightMode::Preview,
            output_path: None,
            columns: vec![0],
            controls: vec![],
            force: false,
            sample_row_count: 10,
            preview_smart_replacements: vec![],
            local_ai_ready: false,
            local_ai_message: None,
        })
        .unwrap();

    assert!(result.readiness.blockers.is_empty());
    assert!(
        result
            .readiness
            .verified_items
            .iter()
            .any(|item| item.contains("Preview does not require an output path"))
    );
}

#[test]
fn preflight_anonymize_blocks_missing_output_path() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("missing-output.csv");
    fs::write(&input_path, "email\nada@example.com\n").unwrap();

    let result = service
        .preflight_anonymization(PreflightParams {
            file_path: input_path,
            mode: PreflightMode::Anonymize,
            output_path: None,
            columns: vec![0],
            controls: vec![],
            force: false,
            sample_row_count: 10,
            preview_smart_replacements: vec![],
            local_ai_ready: false,
            local_ai_message: None,
        })
        .unwrap();

    assert_eq!(result.readiness.status, ReleaseReadinessStatus::Blocked);
    assert!(
        result
            .readiness
            .blockers
            .iter()
            .any(|item| item.contains("Choose an output path"))
    );
}

#[test]
fn preflight_allows_local_ai_anonymize_when_preview_replacements_cover_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-covered.csv");
    let output_path = temp_dir.path().join("smart-covered-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();

    let result = service
        .preflight_anonymization(PreflightParams {
            file_path: input_path,
            mode: PreflightMode::Anonymize,
            output_path: Some(output_path),
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::FullName),
                strategy: AnonymizationStrategy::LocalAi,
            }],
            force: false,
            sample_row_count: 10,
            preview_smart_replacements: vec![
                SmartReplacementEntry {
                    column_index: 0,
                    original: "Alice Smith".to_string(),
                    replacement: "Preview Alice".to_string(),
                },
                SmartReplacementEntry {
                    column_index: 0,
                    original: "Bob Stone".to_string(),
                    replacement: "Preview Bob".to_string(),
                },
            ],
            local_ai_ready: false,
            local_ai_message: Some("Local AI is unavailable.".to_string()),
        })
        .unwrap();

    assert!(
        !result
            .readiness
            .blockers
            .iter()
            .any(|item| item.contains("Local AI"))
    );
    assert!(
        result
            .readiness
            .verified_items
            .iter()
            .any(|item| item.contains("Preview Smart replacements cover"))
    );
}

#[test]
fn preflight_blocks_local_ai_anonymize_when_preview_replacements_are_incomplete() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-incomplete.csv");
    let output_path = temp_dir.path().join("smart-incomplete-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();

    let result = service
        .preflight_anonymization(PreflightParams {
            file_path: input_path,
            mode: PreflightMode::Anonymize,
            output_path: Some(output_path),
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: Some(DataType::FullName),
                strategy: AnonymizationStrategy::LocalAi,
            }],
            force: false,
            sample_row_count: 10,
            preview_smart_replacements: vec![SmartReplacementEntry {
                column_index: 0,
                original: "Alice Smith".to_string(),
                replacement: "Preview Alice".to_string(),
            }],
            local_ai_ready: false,
            local_ai_message: Some("Local AI is unavailable.".to_string()),
        })
        .unwrap();

    assert_eq!(result.readiness.status, ReleaseReadinessStatus::Blocked);
    assert!(
        result
            .readiness
            .blockers
            .iter()
            .any(|item| item.contains("Local AI is unavailable"))
    );
}
