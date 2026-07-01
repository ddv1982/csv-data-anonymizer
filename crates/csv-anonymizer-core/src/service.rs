use crate::csv_io::{count_csv_data_rows, process_file_with_control, read_sample};
use crate::error::{AnonymizerError, Result};
use crate::metadata::{apply_column_selection, build_column_metadata};
use crate::preview::generate_column_preview;
use crate::release_report::{
    ReportContext, build_column_reports, build_evidence, build_readiness, build_utility_metrics,
    standard_notes,
};
use crate::smart::{
    SmartReplacementMap, SmartReplacementProvider, has_smart_replacement_columns,
    missing_smart_replacement_values_from_csv, prepare_smart_replacements_from_csv,
    prepare_smart_replacements_from_rows,
};
use crate::strategies::{STRUCTURED_SCALAR_REDACTION_WARNING, TransformState};
use crate::types::{
    AnonymizationStrategy, AnonymizeData, AnonymizeParams, ColumnControl, ColumnMetadata,
    HeadersData, PreflightData, PreflightMode, PreflightParams, PreviewData, PreviewParams,
    PreviewWarning, PrivacyReport, ProcessControl, ProcessOptions, ReleaseEvidenceItem,
    ReleaseEvidenceStatus, ReleaseReadiness, ReleaseReadinessStatus, ReportIdentifierClass,
    SmartReplacementEntry, WarningSeverity,
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

    pub fn preflight_anonymization(&self, input: PreflightParams) -> Result<PreflightData> {
        let file_path = normalize_path(&input.file_path)?;
        let headers = self.analyze_csv_sampled(&file_path, input.sample_row_count.max(1))?;
        let metadata = headers.columns;
        let mut state = PreflightState::new(&file_path, metadata.len());
        let selected_metadata = selected_preflight_metadata(&metadata, &input, &mut state);
        let selected_smart_columns = has_smart_replacement_columns(&selected_metadata);
        let existing_smart_replacements = existing_preflight_smart_replacements(
            selected_smart_columns,
            &input.preview_smart_replacements,
        );

        state.verified_items.push(
            "Replacements are randomized per run with in-run reuse for repeated source values."
                .to_string(),
        );
        add_preflight_output_evidence(&input, &mut state);

        let local_ai_required = local_ai_required_for_preflight(
            &file_path,
            &input,
            &selected_metadata,
            existing_smart_replacements.as_ref(),
            selected_smart_columns,
            &mut state,
        );
        add_preflight_local_ai_evidence(
            &input,
            selected_smart_columns,
            local_ai_required,
            &mut state,
        );
        add_release_readiness_evidence(&selected_metadata, &mut state);

        let (readiness, evidence) = state.into_readiness_and_evidence();
        Ok(PreflightData {
            mode: input.mode,
            readiness,
            evidence,
            column_reports: build_column_reports(&selected_metadata),
        })
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
        let smart_replacements =
            prepare_smart_replacements_from_rows(&sample.rows, &selected_metadata, None, provider)?;
        let smart_replacement_entries = smart_replacements.to_entries();
        let mut transform_state = if smart_replacements.has_activity() {
            TransformState::with_smart_replacements(smart_replacements)
        } else {
            TransformState::new()
        };
        let mut previews = Vec::new();
        for column in selected_metadata.iter().filter(|column| column.is_selected) {
            previews.push(generate_column_preview(
                column,
                &sample.rows,
                input.sample_count,
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

struct PreflightState {
    blockers: Vec<String>,
    review_items: Vec<String>,
    verified_items: Vec<String>,
    evidence: Vec<ReleaseEvidenceItem>,
}

impl PreflightState {
    fn new(file_path: &Path, column_count: usize) -> Self {
        Self {
            blockers: Vec::new(),
            review_items: Vec::new(),
            verified_items: vec![
                "Input file is readable.".to_string(),
                format!("{column_count} column(s) analyzed."),
            ],
            evidence: vec![ReleaseEvidenceItem {
                id: "input-file".to_string(),
                label: "Input file".to_string(),
                status: ReleaseEvidenceStatus::Verified,
                detail: format!("Read metadata from {}.", file_path.display()),
            }],
        }
    }

    fn into_readiness_and_evidence(self) -> (ReleaseReadiness, Vec<ReleaseEvidenceItem>) {
        (
            finish_readiness(self.blockers, self.review_items, self.verified_items),
            self.evidence,
        )
    }
}

fn selected_preflight_metadata(
    metadata: &[ColumnMetadata],
    input: &PreflightParams,
    state: &mut PreflightState,
) -> Vec<ColumnMetadata> {
    if input.columns.is_empty() {
        state
            .blockers
            .push("Select at least one column to transform or release.".to_string());
    }
    if let Err(error) = validate_column_indices(metadata, &input.columns) {
        state.blockers.push(error.to_string());
    } else if !input.columns.is_empty() {
        state
            .verified_items
            .push(format!("{} column(s) selected.", input.columns.len()));
    }

    let controlled_metadata = match apply_column_controls(metadata, &input.controls) {
        Ok(columns) => columns,
        Err(error) => {
            state.blockers.push(error.to_string());
            metadata.to_vec()
        }
    };
    apply_column_selection(&controlled_metadata, &input.columns)
}

fn existing_preflight_smart_replacements(
    selected_smart_columns: bool,
    preview_smart_replacements: &[SmartReplacementEntry],
) -> Option<SmartReplacementMap> {
    let replacements = SmartReplacementMap::from_entries(preview_smart_replacements);
    (selected_smart_columns && replacements.has_activity()).then_some(replacements)
}

fn add_preflight_output_evidence(input: &PreflightParams, state: &mut PreflightState) {
    match input.mode {
        PreflightMode::Preview => {
            state
                .verified_items
                .push("Preview does not require an output path.".to_string());
        }
        PreflightMode::Anonymize => match input.output_path.as_ref() {
            Some(output_path) => match validate_output_path(output_path, input.force) {
                Ok(path) => {
                    state
                        .verified_items
                        .push("Output path is writable.".to_string());
                    state.evidence.push(ReleaseEvidenceItem {
                        id: "output-path".to_string(),
                        label: "Output path".to_string(),
                        status: ReleaseEvidenceStatus::Verified,
                        detail: format!("Output can be written to {}.", path.display()),
                    });
                }
                Err(error) => {
                    state.blockers.push(error.to_string());
                    state.evidence.push(ReleaseEvidenceItem {
                        id: "output-path".to_string(),
                        label: "Output path".to_string(),
                        status: ReleaseEvidenceStatus::Blocked,
                        detail: error.to_string(),
                    });
                }
            },
            None => state.blockers.push("Choose an output path.".to_string()),
        },
    }
}

fn local_ai_required_for_preflight(
    file_path: &Path,
    input: &PreflightParams,
    selected_metadata: &[ColumnMetadata],
    existing_smart_replacements: Option<&SmartReplacementMap>,
    selected_smart_columns: bool,
    state: &mut PreflightState,
) -> bool {
    if !selected_smart_columns {
        return false;
    }

    match input.mode {
        PreflightMode::Preview => true,
        PreflightMode::Anonymize => match missing_smart_replacement_values_from_csv(
            file_path,
            selected_metadata,
            existing_smart_replacements,
        ) {
            Ok(has_missing_values) => has_missing_values,
            Err(error) => {
                state.blockers.push(error.to_string());
                true
            }
        },
    }
}

fn add_preflight_local_ai_evidence(
    input: &PreflightParams,
    selected_smart_columns: bool,
    local_ai_required: bool,
    state: &mut PreflightState,
) {
    if local_ai_required {
        if input.local_ai_ready {
            state
                .verified_items
                .push("Local AI is ready for Smart replacement columns.".to_string());
            state.evidence.push(ReleaseEvidenceItem {
                id: "local-ai".to_string(),
                label: "Local AI".to_string(),
                status: ReleaseEvidenceStatus::Verified,
                detail: input.local_ai_message.clone().unwrap_or_else(|| {
                    "Local AI is ready for selected Smart replacement columns.".to_string()
                }),
            });
        } else {
            let message = input.local_ai_message.clone().unwrap_or_else(|| {
                "Local AI is not ready for selected Smart replacement columns.".to_string()
            });
            state.blockers.push(message.clone());
            state.evidence.push(ReleaseEvidenceItem {
                id: "local-ai".to_string(),
                label: "Local AI".to_string(),
                status: ReleaseEvidenceStatus::Blocked,
                detail: message,
            });
        }
    } else if selected_smart_columns {
        state
            .verified_items
            .push("Preview Smart replacements cover selected Smart columns.".to_string());
    } else {
        state
            .verified_items
            .push("No selected column requires Local AI.".to_string());
    }
    state
        .verified_items
        .push("Transform settings passed backend validation.".to_string());
}

fn add_release_readiness_evidence(
    selected_metadata: &[ColumnMetadata],
    state: &mut PreflightState,
) {
    let context = ReportContext::default();
    let release_readiness = build_readiness(selected_metadata, &context);
    state.blockers.extend(release_readiness.blockers);
    state.review_items.extend(release_readiness.review_items);
    state
        .verified_items
        .extend(release_readiness.verified_items);
    state
        .evidence
        .extend(build_evidence(selected_metadata, &context));
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

pub(crate) fn validate_column_indices(
    metadata: &[ColumnMetadata],
    columns: &[usize],
) -> Result<()> {
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

pub(crate) fn apply_column_controls(
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

pub(crate) fn preview_warning_for_column(column: &ColumnMetadata) -> Option<PreviewWarning> {
    let message = match column.strategy {
        AnonymizationStrategy::PassThrough => {
            "Pass-through leaves selected values unchanged.".to_string()
        }
        AnonymizationStrategy::LocalAi => {
            "Smart replacement uses Local AI on your device. Review the preview before writing output."
                .to_string()
        }
        AnonymizationStrategy::Redact if redaction_changes_structured_scalar_type(column) => {
            STRUCTURED_SCALAR_REDACTION_WARNING.to_string()
        }
        AnonymizationStrategy::Redact => return None,
        AnonymizationStrategy::Mask | AnonymizationStrategy::Tokenize => return None,
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            if column.detected_type.uses_default_pass_through() {
                format!("{} currently uses pass-through behavior.", column.name)
            } else {
                return None;
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

pub(crate) fn redaction_changes_structured_scalar_type(column: &ColumnMetadata) -> bool {
    column.strategy == AnonymizationStrategy::Redact
        && is_json_or_yaml_source(column)
        && column
            .detected_type
            .redaction_changes_structured_scalar_type()
}

fn is_json_or_yaml_source(column: &ColumnMetadata) -> bool {
    column.source_path.as_deref().is_some_and(|path| {
        matches!(path, "json" | "yaml") || path.starts_with("json/") || path.starts_with("yaml/")
    })
}

fn finish_readiness(
    blockers: Vec<String>,
    review_items: Vec<String>,
    verified_items: Vec<String>,
) -> ReleaseReadiness {
    let status = if !blockers.is_empty() {
        ReleaseReadinessStatus::Blocked
    } else if !review_items.is_empty() {
        ReleaseReadinessStatus::Review
    } else {
        ReleaseReadinessStatus::Verified
    };

    ReleaseReadiness {
        status,
        blockers,
        review_items,
        verified_items,
    }
}

pub(crate) fn build_privacy_report(
    columns: &[ColumnMetadata],
    transform_report: crate::types::TransformReport,
) -> PrivacyReport {
    let mut report = PrivacyReport {
        direct_identifiers: 0,
        quasi_identifiers: 0,
        sensitive_columns: 0,
        pseudonymized_columns: 0,
        smart_replacement_columns: 0,
        opaque_token_columns: 0,
        masked_columns: 0,
        redacted_columns: 0,
        pass_through_columns: 0,
        unique_pseudonym_values: transform_report.unique_pseudonym_values,
        reused_pseudonym_values: transform_report.reused_pseudonym_values,
        collisions_avoided: transform_report.collisions_avoided,
        exhausted_pseudonym_pools: transform_report.exhausted_pseudonym_pools,
        opaque_token_values: transform_report.opaque_token_values,
        smart_replacement_values: transform_report.smart_replacement_values,
        smart_replacement_rejections: transform_report.smart_replacement_rejections,
        smart_replacement_rejection_reasons: transform_report
            .smart_replacement_rejection_reasons
            .clone(),
        smart_replacement_fallbacks: transform_report.smart_replacement_fallbacks,
        readiness: Default::default(),
        evidence: Vec::new(),
        column_reports: Vec::new(),
        utility_metrics: Vec::new(),
        notes: standard_notes(columns, transform_report.clone()),
    };

    for column in columns.iter().filter(|column| column.is_selected) {
        match column.detected_type.report_identifier_class() {
            Some(ReportIdentifierClass::Direct) => report.direct_identifiers += 1,
            Some(ReportIdentifierClass::Quasi) => report.quasi_identifiers += 1,
            None => {}
        }

        match column.strategy {
            AnonymizationStrategy::Mask => report.masked_columns += 1,
            AnonymizationStrategy::Redact => report.redacted_columns += 1,
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

    let context = ReportContext {
        transform_report: Some(&transform_report),
    };
    report.readiness = build_readiness(columns, &context);
    report.evidence = build_evidence(columns, &context);
    report.column_reports = build_column_reports(columns);
    report.utility_metrics = build_utility_metrics(columns, &context);

    report
}

pub(crate) fn count_transforming_selected_columns(columns: &[ColumnMetadata]) -> usize {
    columns
        .iter()
        .filter(|column| column.is_selected && strategy_changes_output(column))
        .count()
}

fn strategy_changes_output(column: &ColumnMetadata) -> bool {
    match column.strategy {
        AnonymizationStrategy::Mask
        | AnonymizationStrategy::Redact
        | AnonymizationStrategy::Tokenize
        | AnonymizationStrategy::LocalAi => true,
        AnonymizationStrategy::PassThrough => false,
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            !column.detected_type.uses_default_pass_through()
        }
    }
}

#[cfg(test)]
mod tests;
