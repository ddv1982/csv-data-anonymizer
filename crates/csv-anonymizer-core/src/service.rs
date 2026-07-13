use crate::csv_io::{count_csv_data_rows, process_file_with_control, read_sample};
use crate::error::Result;
use crate::metadata::{apply_column_selection, build_column_metadata};
use crate::smart::{
    SmartReplacementMap, SmartReplacementProvider, has_smart_replacement_columns,
    prepare_smart_replacements_from_csv,
};
use crate::types::{
    AnonymizeData, AnonymizeParams, HeadersData, PreflightData, PreflightParams, PreviewData,
    PreviewParams, ProcessControl, ProcessOptions,
};
use std::path::Path;

mod controls;
mod path_validation;
mod preflight;
mod preview;
mod privacy_report;

pub(crate) use controls::{
    apply_column_controls, preview_warning_for_column, redaction_changes_structured_scalar_type,
    validate_column_indices,
};
pub use path_validation::generate_default_output_path;
use path_validation::{ensure_output_differs_from_input, normalize_path, validate_output_path};
use preflight::run_preflight;
pub(crate) use preview::preview_rows_with_smart_provider;
pub(crate) use privacy_report::{build_privacy_report, count_transforming_selected_columns};

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

    pub fn preflight_anonymization(&self, input: PreflightParams) -> Result<PreflightData> {
        let file_path = normalize_path(&input.file_path)?;
        let headers = self.analyze_csv_sampled(&file_path, input.sample_row_count.max(1))?;
        run_preflight(&file_path, headers.columns, input)
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
        self.preview_anonymization_with_smart_provider(input, None)
    }

    pub fn preview_anonymization_with_smart_provider(
        &self,
        input: PreviewParams,
        provider: Option<&mut dyn SmartReplacementProvider>,
    ) -> Result<PreviewData> {
        let file_path = normalize_path(&input.file_path)?;
        // Detect on the same sample basis as analyze/anonymize (DEFAULT_SAMPLE_ROWS)
        // so the preview cannot show a different detected type than the final run.
        let sample = read_sample(
            &file_path,
            DEFAULT_SAMPLE_ROWS.max(input.sample_count).max(1),
        )?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);
        // Smart replacement and preview samples stay limited to the display
        // window; only type detection above uses the larger sample.
        let display_row_count = input.sample_count.saturating_mul(2).max(1);
        let display_rows = &sample.rows[..sample.rows.len().min(display_row_count)];
        preview::preview_rows_with_smart_provider(
            &metadata,
            display_rows,
            &input.columns,
            &input.controls,
            input.sample_count,
            provider,
        )
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
        self.anonymize_csv_with_sample_rows_and_control_and_smart_provider(
            input,
            sample_rows,
            control,
            None,
        )
    }

    pub fn anonymize_csv_with_sample_rows_and_control_and_smart_provider(
        &self,
        input: AnonymizeParams,
        sample_rows: usize,
        mut control: Option<&mut ProcessControl<'_>>,
        provider: Option<&mut dyn SmartReplacementProvider>,
    ) -> Result<AnonymizeData> {
        let input_path = normalize_path(&input.file_path)?;
        ensure_output_differs_from_input(&input_path, &input.output_path)?;
        let output_path = validate_output_path(&input.output_path, input.force)?;
        let sample = read_sample(&input_path, sample_rows.max(1))?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);
        validate_column_indices(&metadata, &input.columns)?;
        let controlled_metadata = apply_column_controls(&metadata, &input.controls)?;
        let selected_metadata = apply_column_selection(&controlled_metadata, &input.columns);
        let preview_smart_replacements =
            SmartReplacementMap::from_entries(&input.preview_smart_replacements);
        let existing_smart_replacements = (preview_smart_replacements.has_activity()
            && has_smart_replacement_columns(&selected_metadata))
        .then_some(preview_smart_replacements);
        let smart_replacements = prepare_smart_replacements_from_csv(
            &input_path,
            &selected_metadata,
            control.as_deref_mut(),
            existing_smart_replacements.as_ref(),
            provider,
        )?;
        let smart_replacements = smart_replacements
            .has_activity()
            .then_some(smart_replacements);
        let result = process_file_with_control(
            &input_path,
            &output_path,
            &selected_metadata,
            ProcessOptions {
                smart_replacements: smart_replacements.as_ref(),
            },
            control,
        )?;

        Ok(AnonymizeData {
            output_path,
            row_count: result.row_count,
            columns_anonymized: count_transforming_selected_columns(&selected_metadata),
            duration_ms: result.duration_ms,
            privacy_report: build_privacy_report(&selected_metadata, result.transform_report),
        })
    }
}

#[cfg(test)]
mod tests;
