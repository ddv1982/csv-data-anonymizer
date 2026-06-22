use crate::csv_io::{count_csv_data_rows, process_file_with_control, read_sample};
use crate::detection::is_empty_value;
use crate::error::{AnonymizerError, Result};
use crate::metadata::{apply_column_selection, build_column_metadata};
use crate::privacy::process_privacy_release;
use crate::smart::{
    SmartReplacementMap, SmartReplacementProvider, has_smart_replacement_columns,
    prepare_smart_replacements_from_csv, prepare_smart_replacements_from_rows,
};
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{
    AnonymizationStrategy, AnonymizeData, AnonymizeParams, ColumnControl, ColumnMetadata,
    ColumnPreview, DataType, HeadersData, PiiRisk, PreviewData, PreviewParams, PreviewWarning,
    PrivacyReport, ProcessControl, ProcessOptions, ReleaseMode, SampleTransform, TransformContext,
    WarningSeverity,
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
        self.preview_anonymization_with_smart_provider(input, None)
    }

    pub fn preview_anonymization_with_smart_provider(
        &self,
        input: PreviewParams,
        provider: Option<&mut dyn SmartReplacementProvider>,
    ) -> Result<PreviewData> {
        let file_path = normalize_path(&input.file_path)?;
        let sample = read_sample(&file_path, input.sample_count.saturating_mul(2).max(1))?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);
        validate_column_indices(&metadata, &input.columns)?;
        let controlled_metadata = apply_column_controls(&metadata, &input.controls)?;
        let selected_metadata = apply_column_selection(&controlled_metadata, &input.columns);
        let smart_replacements = prepare_smart_replacements_from_rows(
            &sample.rows,
            &selected_metadata,
            input.deterministic,
            &input.seed,
            provider,
        )?;
        let smart_replacement_entries = smart_replacements.to_entries();
        let mut transform_state = if smart_replacements.is_empty() {
            TransformState::new(input.deterministic, &input.seed)
        } else {
            TransformState::with_smart_replacements(
                input.deterministic,
                &input.seed,
                smart_replacements,
            )
        };
        let mut previews = Vec::new();
        for column in selected_metadata.iter().filter(|column| column.is_selected) {
            previews.push(generate_column_preview(
                column,
                &sample.rows,
                input.sample_count,
                input.deterministic,
                &input.seed,
                &mut transform_state,
            ));
        }
        let warnings = selected_metadata
            .iter()
            .filter(|column| column.is_selected)
            .filter_map(preview_warning_for_column)
            .collect();

        Ok(PreviewData {
            previews,
            warnings,
            smart_replacements: smart_replacement_entries,
        })
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
        let output_path = validate_output_path(&input.output_path, input.force)?;
        let sample = read_sample(&input_path, sample_rows.max(1))?;
        let metadata = build_column_metadata(&sample.headers, &sample.rows);
        validate_column_indices(&metadata, &input.columns)?;
        let controlled_metadata = apply_column_controls(&metadata, &input.controls)?;
        let selected_metadata = apply_column_selection(&controlled_metadata, &input.columns);
        if let Some(privacy_config) = input.privacy_config.as_ref()
            && privacy_config.release_mode != ReleaseMode::Standard
        {
            let result = process_privacy_release(
                &input_path,
                &output_path,
                &selected_metadata,
                privacy_config,
                input.deterministic,
                &input.seed,
                control,
            )?;

            return Ok(AnonymizeData {
                output_path: result.output_path,
                row_count: result.row_count,
                columns_anonymized: result.columns_anonymized,
                duration_ms: result.duration_ms,
                privacy_report: result.privacy_report,
            });
        }
        let preview_smart_replacements =
            SmartReplacementMap::from_entries(&input.preview_smart_replacements);
        let existing_smart_replacements = (!preview_smart_replacements.is_empty()
            && has_smart_replacement_columns(&selected_metadata))
        .then_some(preview_smart_replacements);
        let smart_replacements = prepare_smart_replacements_from_csv(
            &input_path,
            &selected_metadata,
            input.deterministic,
            &input.seed,
            control.as_deref_mut(),
            existing_smart_replacements.as_ref(),
            provider,
        )?;
        let smart_replacements = (!smart_replacements.is_empty()).then_some(smart_replacements);
        let result = process_file_with_control(
            &input_path,
            &output_path,
            &selected_metadata,
            ProcessOptions {
                deterministic: input.deterministic,
                seed: &input.seed,
                smart_replacements: smart_replacements.as_ref(),
            },
            control,
        )?;

        Ok(AnonymizeData {
            output_path,
            row_count: result.row_count,
            columns_anonymized: count_transforming_selected_columns(&selected_metadata),
            duration_ms: result.duration_ms,
            privacy_report: build_privacy_report(
                &selected_metadata,
                result.transform_report,
                input.deterministic,
            ),
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
    let file_name = format!("{stem}_private_output.{extension}");
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

fn apply_column_controls(
    metadata: &[ColumnMetadata],
    controls: &[ColumnControl],
) -> Result<Vec<ColumnMetadata>> {
    let mut controlled = metadata.to_vec();
    for control in controls {
        let Some(column) = controlled.get_mut(control.column_index) else {
            return Err(AnonymizerError::ColumnOutOfRange {
                index: control.column_index,
                max_index: metadata.len().saturating_sub(1),
            });
        };

        if let Some(data_type) = control.type_override {
            column.detected_type = data_type;
        }
        column.strategy = control.strategy;
    }
    Ok(controlled)
}

fn preview_warning_for_column(column: &ColumnMetadata) -> Option<PreviewWarning> {
    let message = match column.strategy {
        AnonymizationStrategy::PassThrough => {
            "Pass-through leaves selected values unchanged.".to_string()
        }
        AnonymizationStrategy::LocalAi => {
            "Smart replacement uses Local AI on your device. Review the preview before writing output."
                .to_string()
        }
        AnonymizationStrategy::Mask | AnonymizationStrategy::Tokenize => return None,
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            match column.detected_type {
                DataType::CountryCode
                | DataType::Enum
                | DataType::Boolean
                | DataType::Currency
                | DataType::Percentage => {
                    format!("{} currently uses pass-through behavior.", column.name)
                }
                _ => return None,
            }
        }
    };

    Some(PreviewWarning {
        column_index: column.index,
        column_name: column.name.clone(),
        message,
        severity: WarningSeverity::Warning,
    })
}

fn build_privacy_report(
    columns: &[ColumnMetadata],
    transform_report: crate::types::TransformReport,
    deterministic: bool,
) -> PrivacyReport {
    let mut report = PrivacyReport {
        release_mode: ReleaseMode::Standard,
        direct_identifiers: 0,
        quasi_identifiers: 0,
        sensitive_columns: 0,
        pseudonymized_columns: 0,
        smart_replacement_columns: 0,
        opaque_token_columns: 0,
        masked_columns: 0,
        generalized_columns: 0,
        pass_through_columns: 0,
        suppressed_rows: 0,
        synthetic_rows: 0,
        dp_epsilon: None,
        dp_budget: None,
        unique_pseudonym_values: transform_report.unique_pseudonym_values,
        reused_pseudonym_values: transform_report.reused_pseudonym_values,
        collisions_avoided: transform_report.collisions_avoided,
        exhausted_pseudonym_pools: transform_report.exhausted_pseudonym_pools,
        opaque_token_values: transform_report.opaque_token_values,
        smart_replacement_values: transform_report.smart_replacement_values,
        smart_replacement_fallbacks: transform_report.smart_replacement_fallbacks,
        formal_models: Vec::new(),
        notes: vec![
            "This app performs local masking and pseudonymization, not formal anonymization."
                .to_string(),
            "Use an opt-in privacy release mode for k-anonymity, l-diversity, t-closeness, differential privacy aggregate releases, or synthetic data generation."
                .to_string(),
        ],
    };

    for column in columns.iter().filter(|column| column.is_selected) {
        match column.detected_type {
            DataType::Email
            | DataType::Phone
            | DataType::FullName
            | DataType::FirstName
            | DataType::LastName
            | DataType::TaxId
            | DataType::Address => report.direct_identifiers += 1,
            DataType::Uuid
            | DataType::NumericId
            | DataType::PostalCode
            | DataType::IpAddress
            | DataType::Url
            | DataType::MacAddress
            | DataType::Timestamp
            | DataType::CountryCode => report.quasi_identifiers += 1,
            _ => {}
        }

        match column.strategy {
            AnonymizationStrategy::Mask => report.masked_columns += 1,
            AnonymizationStrategy::PassThrough => report.pass_through_columns += 1,
            AnonymizationStrategy::Tokenize => report.opaque_token_columns += 1,
            AnonymizationStrategy::LocalAi => report.smart_replacement_columns += 1,
            AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
                if preview_warning_for_column(column).is_some() {
                    report.pass_through_columns += 1;
                } else {
                    report.pseudonymized_columns += 1;
                }
            }
        }
    }

    push_unselected_column_note(&mut report.notes, columns);

    if deterministic {
        report.notes.push(
            "Deterministic pseudonyms use keyed HMAC-SHA256 with the configured seed; treat that seed as sensitive."
                .to_string(),
        );
    } else {
        report.notes.push(
            "Random-mode pseudonyms are tracked within each run so repeated source values stay consistent while distinct readable names avoid reuse while capacity remains."
                .to_string(),
        );
    }
    if report.collisions_avoided > 0 {
        report.notes.push(format!(
            "{} pseudonym candidate collision(s) were avoided by assigning unused alternatives.",
            report.collisions_avoided
        ));
    }
    if report.exhausted_pseudonym_pools > 0 {
        report.notes.push(format!(
            "{} pseudonym pool exhaustion event(s) used generated fallback values.",
            report.exhausted_pseudonym_pools
        ));
    }
    if report.smart_replacement_columns > 0 {
        report.notes.push(
            "Smart replacement used Local AI on this device to generate realistic replacement values; review outputs because this is not a formal anonymization guarantee."
                .to_string(),
        );
    }
    if report.smart_replacement_fallbacks > 0 {
        report.notes.push(format!(
            "{} smart replacement value(s) fell back to rule-based pseudonymization after missing or invalid AI output.",
            report.smart_replacement_fallbacks
        ));
    }

    report
}

fn push_unselected_column_note(notes: &mut Vec<String>, columns: &[ColumnMetadata]) {
    let unselected_columns = columns.iter().filter(|column| !column.is_selected).count();
    if unselected_columns == 0 {
        return;
    }

    let unselected_detector_risk_columns = columns
        .iter()
        .filter(|column| {
            !column.is_selected && matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium)
        })
        .count();
    if unselected_detector_risk_columns > 0 {
        notes.push(format!(
            "{} unselected high/medium detector-risk {} written unchanged.",
            unselected_detector_risk_columns,
            plural(
                unselected_detector_risk_columns,
                "column was",
                "columns were"
            )
        ));
    } else {
        notes.push(format!(
            "{} unselected {} written unchanged.",
            unselected_columns,
            plural(unselected_columns, "column was", "columns were")
        ));
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

fn count_transforming_selected_columns(columns: &[ColumnMetadata]) -> usize {
    columns
        .iter()
        .filter(|column| column.is_selected && strategy_changes_output(column))
        .count()
}

fn strategy_changes_output(column: &ColumnMetadata) -> bool {
    match column.strategy {
        AnonymizationStrategy::Mask
        | AnonymizationStrategy::Tokenize
        | AnonymizationStrategy::LocalAi => true,
        AnonymizationStrategy::PassThrough => false,
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => !matches!(
            column.detected_type,
            DataType::CountryCode
                | DataType::Enum
                | DataType::Boolean
                | DataType::Currency
                | DataType::Percentage
        ),
    }
}

fn generate_column_preview(
    column: &ColumnMetadata,
    rows: &[Vec<String>],
    sample_count: usize,
    deterministic: bool,
    seed: &str,
    transform_state: &mut TransformState,
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
            anonymized: transform_value_with_state(value, column, &context, transform_state),
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
