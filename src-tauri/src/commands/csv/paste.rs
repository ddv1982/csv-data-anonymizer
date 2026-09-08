use super::snapshot::{
    paste_format_name, register_prepared_analysis, require_prepared_analysis,
    require_snapshot_model, selected_candidate_ids, snapshot_detection_summary,
    validate_paste_snapshot,
};
use super::{
    load_local_ai_enabled, load_local_ner_settings, local_ner_unavailable_message,
    parse_tokenization_key,
};
use crate::command_error::CommandError;
use crate::commands::shared::run_blocking;
use crate::local_ai::candidate_detector::local_candidate_detector;
use crate::local_ai::{LocalAiRequest, smart_provider_for_request};
use crate::settings::SettingsStore;
use csv_anonymizer_core::{
    LocalNerRunStatus, PasteAnalyzeData, PasteAnalyzeParams, PastePreviewParams,
    PasteTransformData, PasteTransformParams, PreparedAnalysisSnapshot, PreviewData,
    SmartReplacementProvider, TransformRuntime,
};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PastePreviewRequest {
    #[serde(flatten)]
    pub params: PastePreviewParams,
    pub local_ai: Option<LocalAiRequest>,
    pub prepared_analysis: Option<PreparedAnalysisSnapshot>,
    #[serde(default)]
    pub tokenization_key: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteTransformRequest {
    #[serde(flatten)]
    pub params: PasteTransformParams,
    pub local_ai: Option<LocalAiRequest>,
    pub prepared_analysis: Option<PreparedAnalysisSnapshot>,
    #[serde(default)]
    pub tokenization_key: Option<String>,
}

#[tauri::command]
pub async fn analyze_pasted_data(
    settings: State<'_, Arc<SettingsStore>>,
    request: PasteAnalyzeParams,
) -> Result<PasteAnalyzeData, CommandError> {
    let (local_ner_enabled, local_ner_model) = load_local_ner_settings(&settings)?;
    run_blocking(move || {
        let content = request.content.clone();
        let sample_row_count = request.sample_row_count;
        let mut analysis = if local_ner_enabled {
            if let Some(message) = local_ner_unavailable_message(&local_ner_model)? {
                let mut analysis =
                    csv_anonymizer_core::direct_input::analyze_paste_data(request, None)
                        .map_err(|error| error.to_string())?;
                analysis.detection_run_summary.local_ner = LocalNerRunStatus::Unavailable;
                analysis.detection_run_summary.message = Some(message);
                analysis
            } else {
                let mut detector = local_candidate_detector(&local_ner_model)?;
                csv_anonymizer_core::direct_input::analyze_paste_data(request, Some(&mut detector))
                    .map_err(|error| error.to_string())?
            }
        } else {
            csv_anonymizer_core::direct_input::analyze_paste_data(request, None)
                .map_err(|error| error.to_string())?
        };
        let prepared_analysis = if local_ner_enabled {
            if analysis.prepared_analysis.is_some() {
                analysis.prepared_analysis.take()
            } else {
                Some(
                    PreparedAnalysisSnapshot::new(
                        "paste",
                        paste_format_name(analysis.format),
                        content.as_bytes(),
                        sample_row_count,
                        analysis.columns.clone(),
                        &analysis.detection_run_summary,
                    )
                    .map_err(|error| format!("Could not prepare analysis: {error}"))?,
                )
            }
        } else {
            None
        };
        analysis.prepared_analysis = prepared_analysis;
        if let Some(snapshot) = &analysis.prepared_analysis {
            register_prepared_analysis(snapshot)?;
        }
        Ok(analysis)
    })
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn preview_pasted_data(
    settings: State<'_, Arc<SettingsStore>>,
    request: PastePreviewRequest,
) -> Result<PreviewData, CommandError> {
    let (local_ner_enabled, local_ner_model) = load_local_ner_settings(&settings)?;
    require_prepared_analysis(local_ner_enabled, request.prepared_analysis.as_ref())
        .map_err(CommandError::stale_analysis)?;
    require_snapshot_model(request.prepared_analysis.as_ref(), &local_ner_model)
        .map_err(CommandError::stale_analysis)?;
    if let Some(snapshot) = &request.prepared_analysis {
        validate_paste_snapshot(
            snapshot,
            &request.params.content,
            request.params.format,
            request.params.sample_row_count,
            &request.params.columns,
        )
        .map_err(CommandError::stale_analysis)?;
    }
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
    let tokenization_key = parse_tokenization_key(request.tokenization_key.as_deref())
        .map_err(CommandError::invalid_input)?;
    run_blocking(move || {
        let mut provider = smart_provider_for_request(
            request.local_ai,
            &request.params.controls,
            &request.params.columns,
            local_ai_enabled,
        )?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        if let Some(snapshot) = request
            .prepared_analysis
            .as_ref()
            .filter(|snapshot| matches!(snapshot.format.as_str(), "plainText" | "logs"))
        {
            let confirmed = selected_candidate_ids(snapshot, &request.params.columns);
            csv_anonymizer_core::direct_input::preview_paste_text_candidate_evidence(
                &request.params,
                snapshot,
                &confirmed,
                TransformRuntime {
                    provider,
                    tokenization_key: tokenization_key.as_ref(),
                },
            )
            .map_err(|error| error.to_string())
        } else {
            csv_anonymizer_core::direct_input::preview_paste_data(
                request.params,
                TransformRuntime {
                    provider,
                    tokenization_key: tokenization_key.as_ref(),
                },
            )
            .map_err(|error| error.to_string())
        }
    })
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn anonymize_pasted_data(
    settings: State<'_, Arc<SettingsStore>>,
    request: PasteTransformRequest,
) -> Result<PasteTransformData, CommandError> {
    let (local_ner_enabled, local_ner_model) = load_local_ner_settings(&settings)?;
    require_prepared_analysis(local_ner_enabled, request.prepared_analysis.as_ref())
        .map_err(CommandError::stale_analysis)?;
    require_snapshot_model(request.prepared_analysis.as_ref(), &local_ner_model)
        .map_err(CommandError::stale_analysis)?;
    if let Some(snapshot) = &request.prepared_analysis {
        validate_paste_snapshot(
            snapshot,
            &request.params.content,
            request.params.format,
            request.params.sample_row_count,
            &request.params.columns,
        )
        .map_err(CommandError::stale_analysis)?;
    }
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
    let tokenization_key = parse_tokenization_key(request.tokenization_key.as_deref())
        .map_err(CommandError::invalid_input)?;
    run_blocking(move || {
        let mut provider = smart_provider_for_request(
            request.local_ai,
            &request.params.controls,
            &request.params.columns,
            local_ai_enabled,
        )?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        let mut result = if let Some(snapshot) = request
            .prepared_analysis
            .as_ref()
            .filter(|snapshot| matches!(snapshot.format.as_str(), "plainText" | "logs"))
        {
            let confirmed = selected_candidate_ids(snapshot, &request.params.columns);
            csv_anonymizer_core::direct_input::replay_paste_text_candidate_evidence(
                &request.params,
                snapshot,
                &confirmed,
                TransformRuntime {
                    provider,
                    tokenization_key: tokenization_key.as_ref(),
                },
            )
            .map_err(|error| error.to_string())
        } else {
            csv_anonymizer_core::direct_input::transform_paste_data(
                request.params,
                TransformRuntime {
                    provider,
                    tokenization_key: tokenization_key.as_ref(),
                },
            )
            .map_err(|error| error.to_string())
        }?;
        if let Some(snapshot) = &request.prepared_analysis {
            result.privacy_report.detection_run_summary =
                Some(snapshot_detection_summary(snapshot));
        }
        Ok(result)
    })
    .await
    .map_err(Into::into)
}
