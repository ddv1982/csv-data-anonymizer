use super::*;
use crate::smart::SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN;
use crate::types::{
    SmartReplacementEntry, SmartReplacementRejectionCount, SmartReplacementRejectionReason,
};

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

struct RecordingSmartProvider {
    prefix: &'static str,
    requests: Vec<Vec<String>>,
}

impl RecordingSmartProvider {
    fn new(prefix: &'static str) -> Self {
        Self {
            prefix,
            requests: Vec::new(),
        }
    }
}

#[derive(Default)]
struct RejectingSmartProvider;

impl SmartReplacementProvider for RejectingSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        Ok(request
            .values
            .iter()
            .map(|value| SmartReplacement {
                original: value.clone(),
                replacement: value.clone(),
            })
            .collect())
    }
}

impl SmartReplacementProvider for RecordingSmartProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> Result<Vec<SmartReplacement>> {
        self.requests.push(request.values.to_vec());
        Ok(request
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| SmartReplacement {
                original: value.clone(),
                replacement: format!("{} {} {}", self.prefix, request.column.index, index + 1),
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
                force: false,
                preview_smart_replacements: vec![],
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
fn anonymize_reuses_preview_smart_replacements_and_generates_missing_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-preview-reuse.csv");
    let output_path = temp_dir.path().join("smart-preview-reuse-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\nCharlie Ray\n").unwrap();
    let controls = vec![ColumnControl {
        column_index: 0,
        type_override: Some(DataType::FullName),
        strategy: AnonymizationStrategy::LocalAi,
    }];
    let mut preview_provider = RecordingSmartProvider::new("Preview");

    let preview = service
        .preview_anonymization_with_smart_provider(
            PreviewParams {
                file_path: input_path.clone(),
                columns: vec![0],
                controls: controls.clone(),
                sample_count: 1,
            },
            Some(&mut preview_provider),
        )
        .unwrap();
    let mut final_provider = RecordingSmartProvider::new("Final");

    service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls,
                force: false,
                preview_smart_replacements: preview.smart_replacements.clone(),
            },
            10,
            None,
            Some(&mut final_provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(preview.smart_replacements.len(), 2);
    assert_eq!(preview.previews[0].samples[0].anonymized, output.rows[0][0]);
    assert_eq!(output.rows[1][0], "Preview 0 2");
    assert_eq!(output.rows[2][0], "Final 0 1");
    assert_eq!(
        final_provider.requests,
        vec![vec!["Charlie Ray".to_string()]]
    );
}

#[test]
fn anonymize_rejects_invalid_preview_smart_replacements_and_generates_missing_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-invalid-preview.csv");
    let output_path = temp_dir.path().join("smart-invalid-preview-output.csv");
    fs::write(&input_path, "name\nAlice Smith\n").unwrap();
    let mut provider = RecordingSmartProvider::new("Generated");

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
                force: false,
                preview_smart_replacements: vec![SmartReplacementEntry {
                    column_index: 0,
                    original: "Alice Smith".to_string(),
                    replacement: "Alice Smith".to_string(),
                }],
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_eq!(output.rows[0][0], "Generated 0 1");
    assert_eq!(provider.requests, vec![vec!["Alice Smith".to_string()]]);
    assert_eq!(result.privacy_report.smart_replacement_values, 1);
    assert_eq!(result.privacy_report.smart_replacement_rejections, 1);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::SameAsOriginal,
            count: 1,
        }]
    );
}

#[test]
fn anonymize_reports_all_rejected_smart_replacement_batches() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-all-rejected.csv");
    let output_path = temp_dir.path().join("smart-all-rejected-output.csv");
    fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();
    let mut provider = RejectingSmartProvider;

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
                force: false,
                preview_smart_replacements: vec![],
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();

    let output = read_sample(&output_path, 10).unwrap();
    assert_ne!(output.rows[0][0], "Alice Smith");
    assert_ne!(output.rows[1][0], "Bob Stone");
    assert_eq!(result.privacy_report.smart_replacement_values, 0);
    assert_eq!(result.privacy_report.smart_replacement_rejections, 2);
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 2);
    assert_eq!(
        result.privacy_report.smart_replacement_rejection_reasons,
        vec![SmartReplacementRejectionCount {
            reason: SmartReplacementRejectionReason::SameAsOriginal,
            count: 2,
        }]
    );
}

#[test]
fn anonymize_caps_local_ai_unique_values_and_falls_back_for_excess_values() {
    let service = AnonymizerService::new("test-version");
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("smart-high-cardinality.csv");
    let output_path = temp_dir.path().join("smart-high-cardinality-output.csv");
    let mut csv = String::from("name\n");
    for index in 0..(SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN + 2) {
        csv.push_str(&format!("Person {index}\n"));
    }
    fs::write(&input_path, csv).unwrap();
    let mut provider = RecordingSmartProvider::new("Capped");

    let result = service
        .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            AnonymizeParams {
                file_path: input_path,
                output_path,
                columns: vec![0],
                controls: vec![ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::FullName),
                    strategy: AnonymizationStrategy::LocalAi,
                }],
                force: false,
                preview_smart_replacements: vec![],
            },
            10,
            None,
            Some(&mut provider),
        )
        .unwrap();
    let requested_values = provider.requests.iter().map(Vec::len).sum::<usize>();

    assert_eq!(requested_values, SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN);
    assert_eq!(
        result.privacy_report.smart_replacement_values,
        SMART_REPLACEMENT_VALUE_CAP_PER_COLUMN
    );
    assert_eq!(result.privacy_report.smart_replacement_fallbacks, 2);
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
            sample_count: 1,
        })
        .unwrap_err();

    assert!(error.to_string().contains("Local AI"));
}
