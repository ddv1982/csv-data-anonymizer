use super::{
    apply_column_controls, ensure_output_differs_from_input, validate_column_indices,
    validate_output_path,
};
use crate::error::Result;
use crate::metadata::apply_column_selection;
use crate::release_report::{ReportContext, build_column_reports, build_evidence, build_readiness};
use crate::smart::{
    SmartReplacementMap, has_smart_replacement_columns, missing_smart_replacement_values_from_csv,
};
use crate::types::{
    ColumnMetadata, PreflightData, PreflightMode, PreflightParams, ReleaseEvidenceItem,
    ReleaseEvidenceStatus, ReleaseReadiness, ReleaseReadinessStatus, SmartReplacementEntry,
};
use std::path::Path;

pub(super) fn run_preflight(
    file_path: &Path,
    metadata: Vec<ColumnMetadata>,
    input: PreflightParams,
) -> Result<PreflightData> {
    let mut state = PreflightState::new(file_path, metadata.len());
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
        file_path,
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
        PreflightMode::Preview => state
            .verified_items
            .push("Preview does not require an output path.".to_string()),
        PreflightMode::Anonymize => match input.output_path.as_ref() {
            Some(output_path) => {
                match ensure_output_differs_from_input(&input.file_path, output_path)
                    .and_then(|()| validate_output_path(output_path, input.force))
                {
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
                }
            }
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
