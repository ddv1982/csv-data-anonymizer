use super::*;
use crate::error::AnonymizerError;
use crate::metadata::{apply_column_selection, build_column_metadata};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name)
}

#[test]
fn reads_sample_and_strips_bom() {
    let sample = read_sample(&fixture("bom-file.csv"), 10).unwrap();
    assert_eq!(sample.headers[0], "id");
}

#[test]
fn processes_selected_columns() {
    let input_path = fixture("sample.csv");
    let temp_dir = tempfile::tempdir().unwrap();
    let output_path = temp_dir.path().join("sample-output.csv");
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    let result = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            deterministic: true,
            seed: "service-seed",
            smart_replacements: None,
        },
    )
    .unwrap();

    assert_eq!(result.row_count, 5);
    let output = read_sample(&output_path, 100).unwrap();
    assert_eq!(output.headers, sample.headers);
    assert!(output.rows[0][1].ends_with("@example.com"));
    assert_ne!(output.rows[0][1], sample.rows[0][1]);
    assert_eq!(output.rows[0][0], sample.rows[0][0]);
}

#[test]
fn process_row_count_skips_blank_data_rows_but_preserves_them() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("blank-rows.csv");
    let output_path = temp_dir.path().join("blank-rows-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com\n,\n2,b@example.com\n   ,   \n3,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);

    let result = process_file(
        &input_path,
        &output_path,
        &columns,
        ProcessOptions {
            deterministic: true,
            seed: "service-seed",
            smart_replacements: None,
        },
    )
    .unwrap();

    assert_eq!(count_csv_data_rows(&input_path).unwrap(), 3);
    assert_eq!(result.row_count, 3);
    let output = fs::read_to_string(&output_path).unwrap();
    assert!(output.contains("\n,\n"));
    assert!(output.contains("\n   ,   \n"));
}

#[test]
fn process_control_reports_progress_and_cancels_before_next_row() {
    let temp_dir = tempfile::tempdir().unwrap();
    let input_path = temp_dir.path().join("cancel.csv");
    let output_path = temp_dir.path().join("cancel-output.csv");
    fs::write(
        &input_path,
        "id,email\n1,a@example.com\n,\n2,b@example.com\n3,c@example.com\n",
    )
    .unwrap();
    let sample = read_sample(&input_path, 100).unwrap();
    let columns =
        apply_column_selection(&build_column_metadata(&sample.headers, &sample.rows), &[1]);
    let last_progress = std::cell::Cell::new(0);
    let mut progress_events = Vec::new();
    let error = {
        let mut on_progress = |progress: ProcessProgress| {
            last_progress.set(progress.rows_processed);
            progress_events.push(progress.rows_processed);
        };
        let should_cancel = || last_progress.get() >= 2;
        let mut control = ProcessControl {
            on_progress: Some(&mut on_progress),
            should_cancel: Some(&should_cancel),
        };

        process_file_with_control(
            &input_path,
            &output_path,
            &columns,
            ProcessOptions {
                deterministic: true,
                seed: "service-seed",
                smart_replacements: None,
            },
            Some(&mut control),
        )
        .unwrap_err()
    };

    assert!(matches!(error, AnonymizerError::Canceled));
    assert_eq!(progress_events, vec![1, 2]);
    assert!(!output_path.exists());
}
