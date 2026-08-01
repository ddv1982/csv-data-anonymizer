use super::shared::{
    authorize_or_confirm_input_file, authorize_or_confirm_output_file,
    default_output_path_with_suffix, run_blocking, service,
};
mod snapshot;

use crate::command_error::CommandError;
use crate::local_ai::candidate_detector::local_candidate_detector;
use crate::local_ai::{
    LOCAL_AI_DISABLED_MESSAGE, LocalAiRequest, local_ai_status, selection_requires_local_ai,
    smart_provider_for_request, smart_provider_for_strategy,
};
use crate::path_access::PathAccess;
use crate::settings::{
    MAX_PREVIEW_SAMPLE_COUNT, MAX_SAMPLE_ROW_COUNT, SettingsStore, validate_sample_count,
};
#[cfg(test)]
use csv_anonymizer_core::SourceFingerprint;
use csv_anonymizer_core::{
    ColumnControl, HeadersData, LocalNerRunStatus, PasteAnalyzeData, PasteAnalyzeParams,
    PastePreviewParams, PasteTransformData, PasteTransformParams, PreflightData, PreflightMode,
    PreflightParams, PreparedAnalysisSnapshot, PreviewData, PreviewParams, QuickGenerateParams,
    QuickTransformData, SmartReplacementEntry, SmartReplacementProvider, should_auto_select_column,
};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use snapshot::stage_validated_file_snapshot;
pub(crate) use snapshot::{
    ValidatedFileInput, require_prepared_analysis, require_snapshot_model,
    snapshot_detection_summary,
};
use snapshot::{
    paste_format_name, register_prepared_analysis, selected_candidate_ids, stage_private_csv_file,
    validate_paste_snapshot,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    pub headers: HeadersData,
    pub selected_columns: Vec<usize>,
    pub suggested_output_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_analysis: Option<PreparedAnalysisSnapshot>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequest {
    pub file_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub sample_count: usize,
    pub sample_row_count: usize,
    pub local_ai: Option<LocalAiRequest>,
    pub prepared_analysis: Option<PreparedAnalysisSnapshot>,
    #[serde(default)]
    pub tokenization_key: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightRequest {
    pub mode: PreflightMode,
    pub file_path: PathBuf,
    pub output_path: Option<PathBuf>,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub force: bool,
    pub sample_row_count: usize,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
    pub local_ai: Option<LocalAiRequest>,
    pub prepared_analysis: Option<PreparedAnalysisSnapshot>,
}

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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickGenerateRequest {
    #[serde(flatten)]
    pub params: QuickGenerateParams,
    pub local_ai: Option<LocalAiRequest>,
    #[serde(default)]
    pub tokenization_key: Option<String>,
}

fn load_local_ai_enabled(settings: &State<'_, Arc<SettingsStore>>) -> Result<bool, String> {
    settings
        .load_settings()
        .map(|settings| settings.local_ai_enabled)
        .map_err(|error| format!("Could not load settings: {error}"))
}

fn parse_tokenization_key(
    value: Option<&str>,
) -> Result<Option<csv_anonymizer_core::TokenizationKey>, String> {
    value
        .map(csv_anonymizer_core::TokenizationKey::parse_hex)
        .transpose()
        .map_err(|error| error.to_string())
}

fn load_local_ner_settings(
    settings: &State<'_, Arc<SettingsStore>>,
) -> Result<(bool, String), String> {
    settings
        .load_settings()
        .map(|settings| (settings.local_ner_enabled, settings.local_ai_model))
        .map_err(|error| format!("Could not load settings: {error}"))
}

fn local_ner_unavailable_message(model: &str) -> Result<Option<String>, String> {
    let status = local_ai_status(LocalAiRequest {
        enabled: true,
        model: model.to_string(),
    })?;
    Ok((!status.ready).then_some(status.message))
}

fn analyze_csv_data(
    file_path: PathBuf,
    sample_row_count: usize,
    output_suffix: &str,
    local_ner_enabled: bool,
    local_ner_model: &str,
) -> Result<AnalyzeResponse, String> {
    let service = service();
    let prepared_source = if local_ner_enabled {
        Some(stage_private_csv_file(&file_path)?)
    } else {
        None
    };
    let analysis_path = prepared_source
        .as_ref()
        .map_or(file_path.as_path(), |(staged, _)| staged.as_ref());
    let mut headers = if local_ner_enabled {
        if let Some(message) = local_ner_unavailable_message(local_ner_model)? {
            let mut headers = service
                .analyze_csv_with_sample_rows(analysis_path, sample_row_count)
                .map_err(|error| error.to_string())?;
            headers.detection_run_summary.local_ner = LocalNerRunStatus::Unavailable;
            headers.detection_run_summary.message = Some(message);
            headers
        } else {
            let mut detector = local_candidate_detector(local_ner_model)?;
            service
                .analyze_csv_with_sample_rows_and_candidate_detector(
                    analysis_path,
                    sample_row_count,
                    Some(&mut detector),
                )
                .map_err(|error| error.to_string())?
        }
    } else {
        service
            .analyze_csv_with_sample_rows(analysis_path, sample_row_count)
            .map_err(|error| error.to_string())?
    };
    // The staged path is an implementation detail. Keep the public analysis and
    // suggested destination tied to the user-authorized original source.
    headers.file_path = file_path.clone();
    let selected_columns = headers
        .columns
        .iter()
        .filter(|column| should_auto_select_column(column))
        .map(|column| column.index)
        .collect::<Vec<_>>();
    let suggested_output_path = default_output_path_with_suffix(&file_path, output_suffix)?;
    headers.default_output_path = default_output_path_with_suffix(&file_path, "_private_output")?;
    let prepared_analysis = prepared_source
        .as_ref()
        .map(|(_, source_fingerprint)| {
            PreparedAnalysisSnapshot::new_with_source_fingerprint(
                file_path.to_string_lossy(),
                "csv",
                source_fingerprint.clone(),
                sample_row_count,
                headers.columns.clone(),
                &headers.detection_run_summary,
            )
            .map_err(|error| format!("Could not prepare analysis: {error}"))
        })
        .transpose()?;
    if let Some(snapshot) = &prepared_analysis {
        register_prepared_analysis(snapshot)?;
    }

    Ok(AnalyzeResponse {
        headers,
        selected_columns,
        suggested_output_path,
        prepared_analysis,
    })
}

#[tauri::command]
pub async fn analyze_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    settings: State<'_, Arc<SettingsStore>>,
    file_path: PathBuf,
    sample_row_count: usize,
    output_suffix: String,
) -> Result<AnalyzeResponse, CommandError> {
    validate_sample_count(sample_row_count, MAX_SAMPLE_ROW_COUNT, "Sample row count")
        .map_err(CommandError::invalid_input)?;
    let file_path = authorize_or_confirm_input_file(&app, &path_access, file_path)?;
    // Persisted consent is authoritative; invoke payloads cannot override it.
    let (local_ner_enabled, local_ner_model) = load_local_ner_settings(&settings)?;
    // The suggested output path is only a suggestion: write access is granted
    // later through the explicit confirm/save-dialog flow, never silently here.
    run_blocking(move || {
        analyze_csv_data(
            file_path,
            sample_row_count,
            &output_suffix,
            local_ner_enabled,
            &local_ner_model,
        )
    })
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn preview_anonymization(
    path_access: State<'_, PathAccess>,
    settings: State<'_, Arc<SettingsStore>>,
    request: PreviewRequest,
) -> Result<PreviewData, CommandError> {
    validate_sample_count(
        request.sample_count,
        MAX_PREVIEW_SAMPLE_COUNT,
        "Preview sample count",
    )
    .map_err(CommandError::invalid_input)?;
    // The preview classifies on the same figure analyze and the run are given, so
    // it is bounded by that limit rather than by the display one.
    validate_sample_count(
        request.sample_row_count,
        MAX_SAMPLE_ROW_COUNT,
        "Sample row count",
    )
    .map_err(CommandError::invalid_input)?;
    let file_path = path_access
        .authorize_input_file(request.file_path)
        .map_err(CommandError::path_not_authorized)?;
    let (local_ner_enabled, local_ner_model) = load_local_ner_settings(&settings)?;
    require_prepared_analysis(local_ner_enabled, request.prepared_analysis.as_ref())
        .map_err(CommandError::stale_analysis)?;
    require_snapshot_model(request.prepared_analysis.as_ref(), &local_ner_model)
        .map_err(CommandError::stale_analysis)?;
    let validated_input = ValidatedFileInput::prepare(
        request.prepared_analysis.as_ref(),
        file_path,
        request.sample_row_count,
        &request.columns,
    )
    .map_err(CommandError::stale_analysis)?;
    let processing_path = validated_input.processing_path();
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
    let tokenization_key = parse_tokenization_key(request.tokenization_key.as_deref())
        .map_err(CommandError::invalid_input)?;
    run_blocking(move || {
        let _validated_input = validated_input;
        let mut provider = smart_provider_for_request(
            request.local_ai,
            &request.controls,
            &request.columns,
            local_ai_enabled,
        )?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        service()
            .preview_anonymization_with_run_secrets(
                PreviewParams {
                    file_path: processing_path,
                    columns: request.columns,
                    controls: request.controls,
                    sample_count: request.sample_count,
                    sample_row_count: request.sample_row_count,
                },
                provider,
                tokenization_key.as_ref(),
            )
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn preflight_anonymization(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    settings: State<'_, Arc<SettingsStore>>,
    request: PreflightRequest,
) -> Result<PreflightData, CommandError> {
    validate_sample_count(
        request.sample_row_count,
        MAX_SAMPLE_ROW_COUNT,
        "Sample row count",
    )
    .map_err(CommandError::invalid_input)?;
    let mode = request.mode;
    let file_path = authorize_or_confirm_input_file(&app, &path_access, request.file_path.clone())?;
    let (local_ner_enabled, local_ner_model) = load_local_ner_settings(&settings)?;
    require_prepared_analysis(local_ner_enabled, request.prepared_analysis.as_ref())
        .map_err(CommandError::stale_analysis)?;
    require_snapshot_model(request.prepared_analysis.as_ref(), &local_ner_model)
        .map_err(CommandError::stale_analysis)?;
    let output_path = match (mode, request.output_path.clone()) {
        (PreflightMode::Anonymize, Some(path)) => {
            Some(authorize_or_confirm_output_file(&app, &path_access, path)?)
        }
        (_, output_path) => output_path,
    };
    if output_path.as_deref() == Some(file_path.as_path()) {
        return Err("Output path must differ from the input path.".into());
    }
    let validated_input = ValidatedFileInput::prepare(
        request.prepared_analysis.as_ref(),
        file_path,
        request.sample_row_count,
        &request.columns,
    )?;
    let processing_path = validated_input.processing_path();
    let local_ai_enabled = load_local_ai_enabled(&settings)?;

    run_blocking(move || {
        let _validated_input = validated_input;
        let local_ai_required = selection_requires_local_ai(&request.controls, &request.columns);
        let (local_ai_ready, local_ai_message) = if local_ai_required && !local_ai_enabled {
            (false, Some(LOCAL_AI_DISABLED_MESSAGE.to_string()))
        } else if local_ai_required {
            match request.local_ai.clone() {
                Some(local_ai) => match local_ai_status(local_ai) {
                    Ok(status) => (status.ready, Some(status.message)),
                    Err(error) => (false, Some(error)),
                },
                None => (
                    false,
                    Some(
                        "Local AI is not configured for selected Smart replacement columns."
                            .to_string(),
                    ),
                ),
            }
        } else {
            (false, None)
        };

        service()
            .preflight_anonymization(PreflightParams {
                mode: request.mode,
                file_path: processing_path,
                output_path,
                columns: request.columns,
                controls: request.controls,
                force: request.force,
                sample_row_count: request.sample_row_count,
                preview_smart_replacements: request.preview_smart_replacements,
                local_ai_ready,
                local_ai_message,
            })
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(Into::into)
}

#[tauri::command]
pub async fn count_csv_rows(
    path_access: State<'_, PathAccess>,
    file_path: PathBuf,
) -> Result<usize, CommandError> {
    let file_path = path_access
        .authorize_input_file(file_path)
        .map_err(CommandError::path_not_authorized)?;
    run_blocking(move || {
        service()
            .count_csv_rows(&file_path)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(Into::into)
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
                let mut analysis = csv_anonymizer_core::direct_input::analyze_paste_data(request)
                    .map_err(|error| error.to_string())?;
                analysis.detection_run_summary.local_ner = LocalNerRunStatus::Unavailable;
                analysis.detection_run_summary.message = Some(message);
                analysis
            } else {
                let mut detector = local_candidate_detector(&local_ner_model)?;
                csv_anonymizer_core::direct_input::analyze_paste_data_with_candidate_detector(
                    request,
                    &mut detector,
                )
                .map_err(|error| error.to_string())?
            }
        } else {
            csv_anonymizer_core::direct_input::analyze_paste_data(request)
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
            csv_anonymizer_core::direct_input::preview_paste_text_candidate_evidence_with_run_secrets(
                &request.params,
                snapshot,
                &confirmed,
                provider,
                tokenization_key.as_ref(),
            )
            .map_err(|error| error.to_string())
        } else {
            csv_anonymizer_core::direct_input::preview_paste_data_with_run_secrets(
                request.params,
                provider,
                tokenization_key.as_ref(),
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
            csv_anonymizer_core::direct_input::replay_paste_text_candidate_evidence_with_run_secrets(
                &request.params,
                snapshot,
                &confirmed,
                provider,
                tokenization_key.as_ref(),
            )
            .map_err(|error| error.to_string())
        } else {
            csv_anonymizer_core::direct_input::transform_paste_data_with_run_secrets(
                request.params,
                provider,
                tokenization_key.as_ref(),
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

#[tauri::command]
pub async fn generate_quick_values(
    settings: State<'_, Arc<SettingsStore>>,
    request: QuickGenerateRequest,
) -> Result<QuickTransformData, CommandError> {
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
    let tokenization_key = parse_tokenization_key(request.tokenization_key.as_deref())
        .map_err(CommandError::invalid_input)?;
    run_blocking(move || {
        let mut provider = smart_provider_for_strategy(
            request.local_ai,
            request.params.strategy,
            local_ai_enabled,
        )?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        csv_anonymizer_core::direct_input::generate_quick_values_with_run_secrets(
            request.params,
            provider,
            tokenization_key.as_ref(),
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::AppSettings;
    use csv_anonymizer_core::{
        AnonymizationStrategy, ColumnMetadata, DataType, DetectionRunSummary, LocalNerRunStatus,
        PasteDataFormat,
    };
    use serde_json::{Value, json};
    use tauri::Manager;

    /// Runs a real command body, not a payload shape.
    ///
    /// `tauri::test::mock_app` (the `test` feature, dev-only) supplies the managed state a
    /// `#[tauri::command]` takes, which is what makes these functions callable at all — every
    /// other test here stops at deserializing the request. The check that matters is the first
    /// line of every file command: a path the user never chose through a dialog has to be
    /// refused before it is read. A command that dropped that call would still pass its
    /// payload test, and would read whatever path the frontend asked for.
    #[test]
    fn count_csv_rows_refuses_a_path_the_user_never_granted() {
        // A readable file that simply was never chosen through a dialog. A path that does not
        // exist would be refused by canonicalization before the grant is ever consulted, which
        // would pass this test while proving nothing about access control.
        let temp_dir = tempfile::tempdir().unwrap();
        let input_path = temp_dir.path().join("ungranted.csv");
        std::fs::write(&input_path, "id,email\n1,a@example.com\n").unwrap();

        let app = tauri::test::mock_app();
        app.manage(PathAccess::default());

        let error =
            tauri::async_runtime::block_on(count_csv_rows(app.state::<PathAccess>(), input_path))
                .expect_err("an ungranted path must not be readable");

        assert!(
            error.message.contains("has not been granted"),
            "refusal should name the missing grant, got {error:?}"
        );
    }

    /// The same command on a granted path reaches the reader, which is what stops the test
    /// above from passing for the wrong reason — a command that refused everything would
    /// satisfy it just as well.
    #[test]
    fn count_csv_rows_counts_data_rows_once_the_path_is_granted() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input_path = temp_dir.path().join("granted.csv");
        std::fs::write(&input_path, "id,email\n1,a@example.com\n2,b@example.com\n").unwrap();

        let app = tauri::test::mock_app();
        app.manage(PathAccess::default());
        app.state::<PathAccess>()
            .grant_input_file(&input_path)
            .unwrap();

        let count = tauri::async_runtime::block_on(count_csv_rows(
            app.state::<PathAccess>(),
            input_path.clone(),
        ))
        .unwrap();

        assert_eq!(count, 2);
    }

    fn settings_store_with_local_ner(enabled: bool) -> Arc<SettingsStore> {
        let temp_dir = tempfile::tempdir().expect("settings temp dir");
        let path = temp_dir.keep().join("settings.json");
        let store = Arc::new(SettingsStore::new(path));
        store
            .save_settings(&AppSettings {
                local_ner_enabled: enabled,
                ..AppSettings::default()
            })
            .expect("save settings");
        store
    }

    #[test]
    fn pasted_analysis_reports_disabled_when_persisted_local_ner_is_off() {
        let app = tauri::test::mock_app();
        app.manage(settings_store_with_local_ner(false));
        // Even if a caller sends this extra field, Serde ignores it because persisted
        // backend settings, not an invoke payload, own consent.
        let request: PasteAnalyzeParams = serde_json::from_value(json!({
            "content": "name\nAlice\n",
            "format": "csv",
            "sampleRowCount": 100,
            "localNerEnabled": true,
        }))
        .expect("frontend paste analyze payload");

        let result = tauri::async_runtime::block_on(analyze_pasted_data(
            app.state::<Arc<SettingsStore>>(),
            request,
        ))
        .expect("paste analysis");

        assert_eq!(
            result.detection_run_summary.local_ner,
            LocalNerRunStatus::Disabled
        );
        assert!(result.detection_run_summary.message.is_none());
    }

    #[test]
    fn prepared_file_analysis_must_be_backend_issued_and_match_current_bytes() {
        let temp_dir = tempfile::tempdir().expect("data temp dir");
        let input_path = temp_dir.path().join("data.csv");
        std::fs::write(&input_path, "name\nAlice\n").expect("write fixture");
        let snapshot = PreparedAnalysisSnapshot::new(
            input_path.to_string_lossy(),
            "csv",
            b"name\nAlice\n",
            100,
            Vec::new(),
            &DetectionRunSummary::default(),
        )
        .expect("snapshot");

        assert!(stage_validated_file_snapshot(&snapshot, &input_path, 100, &[]).is_err());
        register_prepared_analysis(&snapshot).expect("register snapshot");
        stage_validated_file_snapshot(&snapshot, &input_path, 100, &[]).expect("issued snapshot");

        std::fs::write(&input_path, "name\nGrace\n").expect("change fixture");
        assert!(stage_validated_file_snapshot(&snapshot, &input_path, 100, &[]).is_err());
    }

    #[test]
    fn private_file_staging_streams_and_fingerprints_the_exact_source() {
        let temp_dir = tempfile::tempdir().expect("data temp dir");
        let input_path = temp_dir.path().join("large.csv");
        let content = "value\n".to_string() + &"abcdefghij\n".repeat(20_000);
        std::fs::write(&input_path, &content).expect("write fixture");

        let (staged, fingerprint) =
            stage_private_csv_file(&input_path).expect("stream staged source");
        let mut expected = SourceFingerprint::default();
        for chunk in content.as_bytes().chunks(17) {
            expected.update(chunk);
        }

        assert_eq!(fingerprint, expected.finish());
        assert_eq!(
            std::fs::read_to_string(staged).expect("read staged source"),
            content
        );
    }

    #[test]
    fn detector_setting_and_snapshot_presence_must_match() {
        let snapshot = PreparedAnalysisSnapshot::new(
            "paste",
            "csv",
            b"name\nAlice\n",
            100,
            Vec::new(),
            &DetectionRunSummary::default(),
        )
        .expect("snapshot");

        assert!(require_prepared_analysis(true, None).is_err());
        assert!(require_prepared_analysis(false, Some(&snapshot)).is_err());
        assert!(require_prepared_analysis(false, None).is_ok());
        assert!(require_prepared_analysis(true, Some(&snapshot)).is_ok());
    }

    #[test]
    fn staged_source_remains_the_exact_validated_content() {
        let temp_dir = tempfile::tempdir().expect("data temp dir");
        let input_path = temp_dir.path().join("data.csv");
        std::fs::write(&input_path, "name\nAlice\n").expect("write fixture");
        let snapshot = PreparedAnalysisSnapshot::new(
            input_path.to_string_lossy(),
            "csv",
            b"name\nAlice\n",
            100,
            Vec::new(),
            &DetectionRunSummary::default(),
        )
        .expect("snapshot");
        register_prepared_analysis(&snapshot).expect("register snapshot");

        let staged = stage_validated_file_snapshot(&snapshot, &input_path, 100, &[])
            .expect("stage validated source");
        std::fs::write(&input_path, "name\nGrace\n").expect("replace original");

        assert_eq!(
            std::fs::read_to_string(&staged).expect("read staged source"),
            "name\nAlice\n"
        );
    }

    #[test]
    fn validated_input_keeps_preview_on_the_analyzed_bytes() {
        let temp_dir = tempfile::tempdir().expect("data temp dir");
        let input_path = temp_dir.path().join("data.csv");
        std::fs::write(&input_path, "name\nAlice\n").expect("write fixture");
        let snapshot = PreparedAnalysisSnapshot::new(
            input_path.to_string_lossy(),
            "csv",
            b"name\nAlice\n",
            100,
            Vec::new(),
            &DetectionRunSummary::default(),
        )
        .expect("snapshot");
        register_prepared_analysis(&snapshot).expect("register snapshot");
        let validated = ValidatedFileInput::prepare(Some(&snapshot), input_path.clone(), 100, &[0])
            .expect("validated input");
        std::fs::write(&input_path, "name\nGrace\n").expect("replace original");

        let preview = service()
            .preview_anonymization(PreviewParams {
                file_path: validated.processing_path(),
                columns: vec![0],
                controls: Vec::new(),
                sample_count: 1,
                sample_row_count: 100,
            })
            .expect("preview staged input");

        assert_eq!(preview.previews[0].samples[0].original, "Alice");
    }

    #[test]
    fn analysis_never_exposes_its_private_staged_path() {
        let temp_dir = tempfile::tempdir().expect("data temp dir");
        let input_path = temp_dir.path().join("data.csv");
        std::fs::write(&input_path, "name\nAlice\n").expect("write fixture");

        let response = analyze_csv_data(input_path.clone(), 100, "_safe", false, "gemma3:4b")
            .expect("analysis");

        assert_eq!(response.headers.file_path, input_path);
        assert_eq!(
            response.headers.default_output_path,
            temp_dir.path().join("data_private_output.csv")
        );
        assert!(
            !response
                .headers
                .default_output_path
                .to_string_lossy()
                .contains("csv-anonymizer-validated-")
        );
    }

    /// Local AI gating turns on this pairing: a control naming Local AI for a column that
    /// is actually selected. These tests feed the command layer the exact payload the
    /// frontend sends so the pairing survives the trip across the IPC boundary intact.
    fn local_ai_control(column_index: usize) -> Value {
        json!({
            "columnIndex": column_index,
            "typeOverride": "fullName",
            "strategy": "localAi",
        })
    }

    fn local_ai_request() -> Value {
        json!({ "enabled": true, "model": "gemma3:4b" })
    }

    fn preview_payload(columns: Value, controls: Value) -> Value {
        json!({
            "filePath": "/tmp/data.csv",
            "columns": columns,
            "controls": controls,
            "sampleCount": 5,
            "sampleRowCount": 200,
            "localAi": local_ai_request(),
        })
    }

    fn preflight_payload(columns: Value, controls: Value) -> Value {
        json!({
            "mode": "anonymize",
            "filePath": "/tmp/data.csv",
            "outputPath": "/tmp/data_private.csv",
            "columns": columns,
            "controls": controls,
            "force": false,
            "sampleRowCount": 200,
            "previewSmartReplacements": [],
            "localAi": local_ai_request(),
        })
    }

    fn parse_preflight(payload: Value) -> PreflightRequest {
        serde_json::from_value(payload).expect("preflight request")
    }

    /// Feeds the real gating predicate the operands as the command sees them.
    ///
    /// It calls [`selection_requires_local_ai`] rather than restating it, so these tests
    /// cover what `preflight_anonymization` actually asks. What they add over the predicate's
    /// own tests is the IPC leg: that the selected column list and each control's column and
    /// strategy survive deserialization from the frontend's payload unchanged, so a column
    /// marked Smart replacement is still recognisable as one by the time the predicate runs.
    fn local_ai_required(request: &PreflightRequest) -> bool {
        selection_requires_local_ai(&request.controls, &request.columns)
    }

    /// A selected column set to Smart replacement is carried across IPC as needing Local AI.
    #[test]
    fn selected_local_ai_column_survives_the_trip_across_the_command_boundary() {
        let request = parse_preflight(preflight_payload(json!([0]), json!([local_ai_control(0)])));

        assert_eq!(request.columns, vec![0]);
        assert_eq!(request.controls.len(), 1);
        assert_eq!(request.controls[0].column_index, 0);
        assert_eq!(request.controls[0].strategy, AnonymizationStrategy::LocalAi);
        assert!(local_ai_required(&request));
    }

    /// A Local AI control on an unselected column does not drag Local AI into the run.
    ///
    /// The column is not being anonymized at all, so demanding a running model for it would
    /// block runs that never touch it.
    #[test]
    fn unselected_local_ai_column_does_not_pull_local_ai_into_the_run() {
        let request = parse_preflight(preflight_payload(json!([1]), json!([local_ai_control(0)])));

        assert!(!local_ai_required(&request));
    }

    /// A selected column on any other strategy does not pull Local AI into the run.
    #[test]
    fn selected_column_on_another_strategy_does_not_pull_local_ai_into_the_run() {
        let request = parse_preflight(preflight_payload(
            json!([0]),
            json!([{ "columnIndex": 0, "typeOverride": null, "strategy": "mask" }]),
        ));

        assert!(!local_ai_required(&request));
        assert_eq!(request.controls[0].strategy, AnonymizationStrategy::Mask);
    }

    /// One selected Smart replacement column among several is enough to require Local AI.
    #[test]
    fn any_single_selected_local_ai_column_requires_local_ai() {
        let request = parse_preflight(preflight_payload(
            json!([0, 2]),
            json!([
                { "columnIndex": 0, "typeOverride": null, "strategy": "pseudonymize" },
                local_ai_control(2),
            ]),
        ));

        assert!(local_ai_required(&request));
    }

    /// A strategy spelled in snake_case is refused rather than quietly read as something else.
    ///
    /// Falling back to a default strategy would anonymize the column by a rule the user never
    /// chose, and the run would report success while doing it.
    #[test]
    fn misspelled_strategy_names_are_refused_instead_of_defaulted() {
        let payload = preflight_payload(
            json!([0]),
            json!([{ "columnIndex": 0, "typeOverride": null, "strategy": "local_ai" }]),
        );

        assert!(serde_json::from_value::<PreflightRequest>(payload).is_err());
    }

    /// The preview command accepts the payload `previewAnonymization` sends, field for field.
    ///
    /// The frontend suite mocks this boundary, so nothing else pins these wire names; a
    /// rename would surface only as a failing command at runtime.
    #[test]
    fn preview_request_accepts_the_payload_the_frontend_sends() {
        let request: PreviewRequest =
            serde_json::from_value(preview_payload(json!([0, 1]), json!([local_ai_control(1)])))
                .expect("preview request");

        assert_eq!(request.file_path, PathBuf::from("/tmp/data.csv"));
        assert_eq!(request.columns, vec![0, 1]);
        assert_eq!(request.sample_count, 5);
        assert_eq!(request.sample_row_count, 200);
        assert_eq!(request.controls[0].type_override, Some(DataType::FullName));
        let local_ai = request.local_ai.expect("local ai request");
        assert!(local_ai.enabled);
        assert_eq!(local_ai.model, "gemma3:4b");
    }

    /// The preflight command accepts the payload `preflightAnonymization` sends.
    ///
    /// Both modes and the absent output path matter: preview runs send no destination, and
    /// only `anonymize` mode is allowed to ask for write access to one.
    #[test]
    fn preflight_request_accepts_both_modes_and_an_absent_output_path() {
        let anonymize = parse_preflight(preflight_payload(json!([0]), json!([])));
        let mut preview_payload = preflight_payload(json!([0]), json!([]));
        preview_payload["mode"] = json!("preview");
        preview_payload["outputPath"] = Value::Null;
        let preview = parse_preflight(preview_payload);

        assert_eq!(anonymize.mode, PreflightMode::Anonymize);
        assert_eq!(
            anonymize.output_path,
            Some(PathBuf::from("/tmp/data_private.csv"))
        );
        assert_eq!(preview.mode, PreflightMode::Preview);
        assert!(preview.output_path.is_none());
    }

    /// Preview smart replacements reach preflight under the name the frontend sends them.
    ///
    /// These are the values already shown to the user; losing them makes preflight judge a
    /// run on replacements the actual run will not use.
    #[test]
    fn preflight_request_carries_preview_smart_replacements() {
        let mut payload = preflight_payload(json!([0]), json!([local_ai_control(0)]));
        payload["previewSmartReplacements"] = json!([
            { "columnIndex": 0, "original": "Alice Smith", "replacement": "Preview Alice" },
        ]);

        let request = parse_preflight(payload);

        assert_eq!(request.preview_smart_replacements.len(), 1);
        assert_eq!(request.preview_smart_replacements[0].column_index, 0);
        assert_eq!(
            request.preview_smart_replacements[0].replacement,
            "Preview Alice"
        );
    }

    /// Omitting controls entirely leaves the list empty rather than failing the request.
    ///
    /// No control means no per-column override, which is the same thing an empty list means.
    #[test]
    fn omitted_controls_are_read_as_no_overrides() {
        let mut payload = preflight_payload(json!([0]), json!([]));
        payload
            .as_object_mut()
            .expect("payload object")
            .remove("controls");

        let request = parse_preflight(payload);

        assert!(request.controls.is_empty());
        assert!(!local_ai_required(&request));
    }

    /// Paste preview flattens the core paste params alongside the Local AI request.
    ///
    /// The frontend sends one flat object; a nested `params` key would leave every paste
    /// field unset.
    #[test]
    fn paste_preview_request_flattens_paste_params_beside_local_ai() {
        let request: PastePreviewRequest = serde_json::from_value(json!({
            "content": "name\nAlice\n",
            "format": "csv",
            "columns": [0],
            "controls": [local_ai_control(0)],
            "sampleCount": 5,
            "sampleRowCount": 200,
            "localAi": local_ai_request(),
        }))
        .expect("paste preview request");

        assert_eq!(request.params.content, "name\nAlice\n");
        assert_eq!(request.params.format, PasteDataFormat::Csv);
        assert_eq!(request.params.columns, vec![0]);
        assert_eq!(
            request.params.controls[0].strategy,
            AnonymizationStrategy::LocalAi
        );
        assert!(request.local_ai.is_some());
    }

    /// Paste transform flattens the core paste params alongside the Local AI request.
    #[test]
    fn paste_transform_request_flattens_paste_params_beside_local_ai() {
        let request: PasteTransformRequest = serde_json::from_value(json!({
            "content": "name\nAlice\n",
            "format": "plainText",
            "columns": [0],
            "controls": [local_ai_control(0)],
            "sampleRowCount": 200,
            "previewSmartReplacements": [
                { "columnIndex": 0, "original": "Alice", "replacement": "Preview Alice" },
            ],
            "localAi": local_ai_request(),
        }))
        .expect("paste transform request");

        assert_eq!(request.params.format, PasteDataFormat::PlainText);
        assert_eq!(request.params.preview_smart_replacements.len(), 1);
        assert!(request.local_ai.is_some());
    }

    /// Quick generation flattens the core quick params alongside the Local AI request.
    ///
    /// `strategy` here is the whole gate for quick values: it is what
    /// `smart_provider_for_strategy` reads to decide whether Local AI is involved at all.
    #[test]
    fn quick_generate_request_flattens_quick_params_beside_local_ai() {
        let request: QuickGenerateRequest = serde_json::from_value(json!({
            "dataType": "fullName",
            "strategy": "localAi",
            "count": 3,
            "localAi": local_ai_request(),
        }))
        .expect("quick generate request");

        assert_eq!(request.params.data_type, DataType::FullName);
        assert_eq!(request.params.strategy, AnonymizationStrategy::LocalAi);
        assert_eq!(request.params.count, 3);
        assert!(request.local_ai.is_some());
    }

    /// A request may leave Local AI out entirely, which is not the same as disabling it.
    ///
    /// Preflight distinguishes the two: an absent request on a Smart replacement column is
    /// reported as "not configured", never as ready.
    #[test]
    fn absent_local_ai_request_is_distinct_from_a_disabled_one() {
        let mut absent = preflight_payload(json!([0]), json!([local_ai_control(0)]));
        absent["localAi"] = Value::Null;
        let mut disabled = preflight_payload(json!([0]), json!([local_ai_control(0)]));
        disabled["localAi"] = json!({ "enabled": false, "model": "gemma3:4b" });

        let absent = parse_preflight(absent);
        let disabled = parse_preflight(disabled);

        assert!(local_ai_required(&absent));
        assert!(absent.local_ai.is_none());
        assert!(local_ai_required(&disabled));
        assert!(!disabled.local_ai.expect("local ai request").enabled);
    }

    /// The analyze response serializes under the names the frontend reads.
    ///
    /// The auto-selection and the suggested destination are both read straight off this
    /// object; a rename leaves the UI with no preselected columns and no output path.
    #[test]
    fn analyze_response_serializes_the_names_the_frontend_reads() {
        let response = AnalyzeResponse {
            headers: HeadersData {
                file_path: PathBuf::from("/tmp/data.csv"),
                row_count: 2,
                row_count_is_complete: true,
                default_output_path: PathBuf::from("/tmp/data_private_output.csv"),
                detection_run_summary: DetectionRunSummary::default(),
                columns: Vec::<ColumnMetadata>::new(),
            },
            selected_columns: vec![0, 2],
            suggested_output_path: PathBuf::from("/tmp/data_private.csv"),
            prepared_analysis: None,
        };

        let value = serde_json::to_value(&response).expect("analyze response");

        assert_eq!(value["selectedColumns"], json!([0, 2]));
        assert_eq!(value["suggestedOutputPath"], json!("/tmp/data_private.csv"));
        assert_eq!(value["headers"]["rowCountIsComplete"], json!(true));
        assert_eq!(
            value["headers"]["detectionRunSummary"]["localNer"],
            json!("disabled")
        );
        assert!(value.get("detectionRunSummary").is_none());
    }
}
