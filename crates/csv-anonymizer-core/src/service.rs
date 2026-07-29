use crate::csv_io::{count_csv_data_rows, read_detection_sample, read_sample};
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
    apply_column_controls, cardinality_warning_for_column, possible_person_name_warning_for_column,
    preview_warning_for_column, redaction_changes_structured_scalar_type, validate_column_indices,
};
pub use path_validation::generate_default_output_path;
use path_validation::{ensure_output_differs_from_input, normalize_path, validate_output_path};
use preflight::run_preflight;
pub(crate) use preview::preview_rows_with_smart_provider;
pub(crate) use privacy_report::{build_privacy_report, count_transforming_selected_columns};

const DEFAULT_SAMPLE_ROWS: usize = 100;

/// Rows to classify on, given what a caller asked for.
///
/// [`DEFAULT_SAMPLE_ROWS`] is a floor rather than a default: every entry point that
/// classifies a file routes its figure through here, so the "Sample rows" setting
/// can only ask for *more* evidence than the default, never less.
///
/// The floor is only half of what makes the four entry points agree, and the weaker
/// half. They are separate commands answering questions about one file — analyze
/// fills the column table the user selects from, preflight judges whether the run
/// may proceed, preview shows what it will do, and the run does it — so they also
/// have to be *given* the same figure. Detection votes on the ratio of matching
/// values in the sample, so two entry points sampling differently can genuinely land
/// on different types: measured on a column that is one-third email addresses, the
/// detected type moves through Email, String and Enum as the sample grows from 1 row
/// to 100. Whichever of those is right, the four commands disagreeing about it means
/// the table promises a classification the run does not apply. Preview used to reach
/// this floor with its *display* row count, which is capped below the floor, so at
/// any "Sample rows" above the default it classified on less than analyze had.
/// [`PreviewParams::sample_row_count`] is how it now hears the same figure.
///
/// A floor rather than a fixed value because a caller asking for more evidence is
/// always safe: the sample is drawn from the whole input either way, so a larger
/// figure refines the same estimate rather than shifting where it looks.
fn detection_sample_rows(requested: usize) -> usize {
    DEFAULT_SAMPLE_ROWS.max(requested).max(1)
}

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

    /// Detection reads the whole file in one streaming pass and keeps
    /// `sample_rows` values spread across it, so the exact row count falls out
    /// of the same pass that classifies the columns.
    ///
    /// `sample_rows` is a request, floored by `detection_sample_rows`.
    pub fn analyze_csv_with_sample_rows(
        &self,
        file_path: impl AsRef<Path>,
        sample_rows: usize,
    ) -> Result<HeadersData> {
        let file_path = normalize_path(file_path.as_ref())?;
        let sample = read_detection_sample(&file_path, detection_sample_rows(sample_rows))?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);

        Ok(HeadersData {
            file_path: file_path.clone(),
            row_count: sample.data_rows_scanned,
            row_count_is_complete: sample.scanned_entire_input,
            default_output_path: generate_default_output_path(&file_path),
            columns: metadata,
        })
    }

    /// Checks a pending run and reports what would block it.
    ///
    /// Classifies through `analyze_csv_with_sample_rows`, so preflight judges the
    /// run on exactly the types the run will use — see `detection_sample_rows`
    /// for why that has to be arranged rather than assumed.
    ///
    /// This costs a streaming pass, and the preview that follows costs another one
    /// on the same file. Detection cannot be correct without at least one pass, and
    /// the two commands are separate entry points with no shared state — the client
    /// calls preflight, reads the verdict, then calls preview — so the second pass
    /// is the price of that split rather than something to optimize away here.
    pub fn preflight_anonymization(&self, input: PreflightParams) -> Result<PreflightData> {
        let file_path = normalize_path(&input.file_path)?;
        let headers = self.analyze_csv_with_sample_rows(&file_path, input.sample_row_count)?;
        run_preflight(&file_path, headers.columns, input)
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
        // Detect on `sample_row_count`, the figure analyze and the run are given,
        // so the preview cannot show a different detected type — and therefore a
        // different strategy — than the final run. Not on `sample_count`: that is
        // how many rows to *display*, which says nothing about how much evidence
        // detection needs, and is capped well below what "Sample rows" allows.
        let detection_sample =
            read_detection_sample(&file_path, detection_sample_rows(input.sample_row_count))?;
        let metadata = build_column_metadata(&detection_sample.headers, &detection_sample.rows);
        // Displayed rows are a separate, head-anchored window: the user expects
        // the preview to show the file's opening rows, not the detection spread.
        let display_row_count = input.sample_count.saturating_mul(2).max(1);
        let display = read_sample(&file_path, display_row_count)?;
        preview::preview_rows_with_smart_provider(
            &metadata,
            &display.rows,
            &input.columns,
            &input.controls,
            input.sample_count,
            // The file's row count, from the pass that just classified it — not the
            // sample size, which is what made the cardinality warning miss columns
            // whose values repeat across a file far larger than the sample.
            detection_sample.data_rows_scanned,
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
        let sample = read_detection_sample(&input_path, detection_sample_rows(sample_rows))?;
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
        let result = crate::csv_io::process_file_with_control_and_overwrite(
            &input_path,
            &output_path,
            &selected_metadata,
            ProcessOptions {
                smart_replacements: smart_replacements.as_ref(),
                mapping_entry_ceiling: None,
            },
            control,
            input.force,
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
