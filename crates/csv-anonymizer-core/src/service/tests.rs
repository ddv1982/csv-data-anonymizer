use super::*;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn analyzes_csv_headers_and_default_output_path() {
    let service = AnonymizerService::new("test-version");
    let result = service.analyze_csv(fixture("sample.csv")).unwrap();

    assert_eq!(result.row_count, 5);
    assert!(result.row_count_is_complete);
    assert!(
        result
            .default_output_path
            .ends_with("sample_anonymized.csv")
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
fn previews_are_deterministic() {
    let service = AnonymizerService::new("test-version");
    let params = PreviewParams {
        file_path: fixture("sample.csv"),
        columns: vec![1],
        deterministic: true,
        seed: "preview-seed".to_string(),
        sample_count: 2,
    };

    let first = service.preview_anonymization(params.clone()).unwrap();
    let second = service.preview_anonymization(params).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.previews[0].samples.len(), 2);
}

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
            deterministic: true,
            seed: "service-seed".to_string(),
            force: false,
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
                    deterministic: true,
                    seed: "service-seed".to_string(),
                    force: false,
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
                deterministic: true,
                seed: "sparse-seed".to_string(),
                force: false,
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
