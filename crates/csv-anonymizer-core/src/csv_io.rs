use crate::error::{AnonymizerError, Result, csv_error};
use crate::strategies::transform_row;
use crate::types::{ColumnMetadata, ParsedSample, ProcessOptions, ProcessResult};
use csv::{ReaderBuilder, StringRecord, Trim, WriterBuilder};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub fn validate_file(file_path: &Path) -> Result<()> {
    let metadata = fs::metadata(file_path)
        .map_err(|_| AnonymizerError::FileNotFound(file_path.to_path_buf()))?;
    if !metadata.is_file() {
        return Err(AnonymizerError::FileNotFound(file_path.to_path_buf()));
    }
    Ok(())
}

pub fn read_sample(file_path: &Path, row_count: usize) -> Result<ParsedSample> {
    validate_file(file_path)?;

    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::All)
        .from_path(file_path)
        .map_err(csv_error)?;

    let mut headers: Vec<String> = Vec::new();
    let mut rows: Vec<Vec<String>> = Vec::new();

    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        let mut row = record_to_vec(&record);

        if headers.is_empty() {
            if let Some(first) = row.first_mut() {
                *first = strip_bom(first).to_string();
            }
            if row.is_empty() {
                return Err(AnonymizerError::csv_parse(
                    "CSV file is empty or has no valid headers",
                    None,
                ));
            }
            headers = row;
            continue;
        }

        if row.iter().all(|value| value.is_empty()) {
            continue;
        }

        rows.push(row);
        if rows.len() >= row_count {
            break;
        }
    }

    if headers.is_empty() {
        return Err(AnonymizerError::csv_parse(
            "CSV file is empty or has no valid headers",
            None,
        ));
    }

    Ok(ParsedSample { headers, rows })
}

pub fn count_csv_data_rows(file_path: &Path) -> Result<usize> {
    validate_file(file_path)?;
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .trim(Trim::All)
        .from_path(file_path)
        .map_err(csv_error)?;
    let mut header_processed = false;
    let mut row_count = 0;

    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        let row = record_to_vec(&record);
        if !header_processed {
            header_processed = true;
            continue;
        }
        if row.iter().all(|value| value.is_empty()) {
            continue;
        }
        row_count += 1;
    }

    Ok(row_count)
}

pub fn process_file(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
) -> Result<ProcessResult> {
    validate_file(input_path)?;
    let start_time = Instant::now();
    let temporary_output_path = temporary_output_path(output_path);

    let process_result = process_file_to_temporary_output(
        input_path,
        &temporary_output_path,
        columns,
        options,
        start_time,
    );

    match process_result {
        Ok(mut result) => {
            fs::rename(&temporary_output_path, output_path)?;
            result.output_path = output_path.to_path_buf();
            Ok(result)
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary_output_path);
            Err(error)
        }
    }
}

fn process_file_to_temporary_output(
    input_path: &Path,
    temporary_output_path: &Path,
    columns: &[ColumnMetadata],
    options: ProcessOptions<'_>,
    start_time: Instant,
) -> Result<ProcessResult> {
    let mut reader = ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_path(input_path)
        .map_err(csv_error)?;
    let mut writer = WriterBuilder::new()
        .has_headers(false)
        .from_path(temporary_output_path)
        .map_err(csv_error)?;

    let mut header_processed = false;
    let mut row_count = 0;

    for result in reader.records() {
        let record = result.map_err(csv_error)?;
        let mut row = record_to_vec(&record);

        if !header_processed {
            if let Some(first) = row.first_mut() {
                *first = strip_bom(first).to_string();
            }
            writer.write_record(&row).map_err(csv_error)?;
            header_processed = true;
            continue;
        }

        let transformed_row = transform_row(
            &row,
            columns,
            row_count,
            options.seed,
            options.deterministic,
        );
        writer.write_record(&transformed_row).map_err(csv_error)?;
        row_count += 1;
    }

    writer.flush()?;

    Ok(ProcessResult {
        row_count,
        success: true,
        output_path: temporary_output_path.to_path_buf(),
        duration_ms: start_time.elapsed().as_millis(),
    })
}

fn record_to_vec(record: &StringRecord) -> Vec<String> {
    record.iter().map(ToString::to_string).collect()
}

fn strip_bom(value: &str) -> &str {
    value.strip_prefix('\u{feff}').unwrap_or(value)
}

fn temporary_output_path(output_path: &Path) -> PathBuf {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output.csv");
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    parent.join(format!(".{file_name}.{suffix}.tmp"))
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
