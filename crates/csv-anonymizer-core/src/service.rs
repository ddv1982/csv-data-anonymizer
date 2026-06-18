use crate::csv_io::{count_csv_data_rows, process_file_with_control, read_sample};
use crate::detection::is_empty_value;
use crate::error::{AnonymizerError, Result};
use crate::metadata::{apply_column_selection, build_column_metadata};
use crate::strategies::transform_value;
use crate::types::{
    AnonymizeData, AnonymizeParams, ColumnMetadata, ColumnPreview, HeadersData, PreviewData,
    PreviewParams, ProcessControl, ProcessOptions, SampleTransform, TransformContext,
};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_SAMPLE_ROWS: usize = 100;

#[derive(Debug, Clone)]
pub struct AnonymizerService {
    version: String,
}

impl AnonymizerService {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn analyze_csv(&self, file_path: impl AsRef<Path>) -> Result<HeadersData> {
        self.analyze_csv_with_sample_rows(file_path, DEFAULT_SAMPLE_ROWS)
    }

    pub fn analyze_csv_with_sample_rows(
        &self,
        file_path: impl AsRef<Path>,
        sample_rows: usize,
    ) -> Result<HeadersData> {
        self.analyze_csv_with_options(file_path, sample_rows, true)
    }

    pub fn analyze_csv_sampled(
        &self,
        file_path: impl AsRef<Path>,
        sample_rows: usize,
    ) -> Result<HeadersData> {
        self.analyze_csv_with_options(file_path, sample_rows, false)
    }

    fn analyze_csv_with_options(
        &self,
        file_path: impl AsRef<Path>,
        sample_rows: usize,
        count_all_rows: bool,
    ) -> Result<HeadersData> {
        let file_path = normalize_path(file_path.as_ref())?;
        let sample = read_sample(&file_path, sample_rows.max(1))?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);
        let counted_rows = if count_all_rows {
            count_csv_data_rows(&file_path).ok()
        } else {
            None
        };
        let row_count = counted_rows.unwrap_or(sample.rows.len());
        let row_count_is_complete = counted_rows.is_some() || sample.is_complete;

        Ok(HeadersData {
            file_path: file_path.clone(),
            row_count,
            row_count_is_complete,
            default_output_path: generate_default_output_path(&file_path),
            columns: metadata,
        })
    }

    pub fn preview_anonymization(&self, input: PreviewParams) -> Result<PreviewData> {
        let file_path = normalize_path(&input.file_path)?;
        let sample = read_sample(&file_path, input.sample_count.saturating_mul(2).max(1))?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);
        validate_column_indices(&metadata, &input.columns)?;
        let selected_metadata = apply_column_selection(&metadata, &input.columns);
        let previews = selected_metadata
            .iter()
            .filter(|column| column.is_selected)
            .map(|column| {
                generate_column_preview(
                    column,
                    &sample.rows,
                    input.sample_count,
                    input.deterministic,
                    &input.seed,
                )
            })
            .collect();

        Ok(PreviewData { previews })
    }

    pub fn count_csv_rows(&self, file_path: impl AsRef<Path>) -> Result<usize> {
        let file_path = normalize_path(file_path.as_ref())?;
        count_csv_data_rows(&file_path)
    }

    pub fn anonymize_csv(&self, input: AnonymizeParams) -> Result<AnonymizeData> {
        self.anonymize_csv_with_sample_rows(input, DEFAULT_SAMPLE_ROWS)
    }

    pub fn anonymize_csv_with_sample_rows(
        &self,
        input: AnonymizeParams,
        sample_rows: usize,
    ) -> Result<AnonymizeData> {
        self.anonymize_csv_with_sample_rows_and_control(input, sample_rows, None)
    }

    pub fn anonymize_csv_with_control(
        &self,
        input: AnonymizeParams,
        control: &mut ProcessControl<'_>,
    ) -> Result<AnonymizeData> {
        self.anonymize_csv_with_sample_rows_and_control(input, DEFAULT_SAMPLE_ROWS, Some(control))
    }

    pub fn anonymize_csv_with_sample_rows_and_control(
        &self,
        input: AnonymizeParams,
        sample_rows: usize,
        control: Option<&mut ProcessControl<'_>>,
    ) -> Result<AnonymizeData> {
        let input_path = normalize_path(&input.file_path)?;
        let output_path = validate_output_path(&input.output_path, input.force)?;
        let sample = read_sample(&input_path, sample_rows.max(1))?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);
        validate_column_indices(&metadata, &input.columns)?;
        let selected_metadata = apply_column_selection(&metadata, &input.columns);
        let result = process_file_with_control(
            &input_path,
            &output_path,
            &selected_metadata,
            ProcessOptions {
                deterministic: input.deterministic,
                seed: &input.seed,
            },
            control,
        )?;

        Ok(AnonymizeData {
            output_path,
            row_count: result.row_count,
            columns_anonymized: input.columns.len(),
            duration_ms: result.duration_ms,
        })
    }
}

pub fn generate_default_output_path(input_path: &Path) -> PathBuf {
    let extension = input_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("csv");
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let file_name = format!("{stem}_anonymized.{extension}");
    input_path.with_file_name(file_name)
}

fn normalize_path(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(AnonymizerError::FileNotFound(path.to_path_buf()));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn validate_output_path(output_path: &Path, force: bool) -> Result<PathBuf> {
    let normalized = normalize_path(output_path)?;
    if normalized.exists() && !force {
        return Err(AnonymizerError::OutputExists(normalized));
    }

    let output_dir = normalized
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if !output_dir.is_dir() {
        return Err(AnonymizerError::OutputDirectoryNotWritable(output_dir));
    }

    let probe = output_dir.join(format!(".csv-anonymizer-write-test-{}", std::process::id()));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
        }
        Err(_) => return Err(AnonymizerError::OutputDirectoryNotWritable(output_dir)),
    }

    Ok(normalized)
}

fn validate_column_indices(metadata: &[ColumnMetadata], columns: &[usize]) -> Result<()> {
    let max_index = metadata.len().saturating_sub(1);
    for index in columns {
        if *index >= metadata.len() {
            return Err(AnonymizerError::ColumnOutOfRange {
                index: *index,
                max_index,
            });
        }
    }
    Ok(())
}

fn generate_column_preview(
    column: &ColumnMetadata,
    rows: &[Vec<String>],
    sample_count: usize,
    deterministic: bool,
    seed: &str,
) -> ColumnPreview {
    let mut samples = Vec::new();

    for (row_index, row) in rows.iter().enumerate() {
        if samples.len() >= sample_count {
            break;
        }

        let Some(value) = row.get(column.index) else {
            continue;
        };
        if is_empty_value(value) {
            continue;
        }

        let context = TransformContext {
            column_name: &column.name,
            column_index: column.index,
            row_index,
            seed,
            deterministic,
            empty_format: column.empty_format,
        };
        samples.push(SampleTransform {
            original: value.clone(),
            anonymized: transform_value(value, column, &context),
        });
    }

    ColumnPreview {
        column_index: column.index,
        column_name: column.name.clone(),
        samples,
    }
}

#[cfg(test)]
mod tests;
