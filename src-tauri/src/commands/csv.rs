use super::shared::{
    authorize_or_confirm_input_file, authorize_or_confirm_output_file,
    default_output_path_with_suffix, run_blocking, service,
};
use crate::local_ai::{
    LocalAiRequest, local_ai_status, smart_provider_for_request, smart_provider_for_strategy,
};
use crate::path_access::PathAccess;
use crate::settings::{
    MAX_PREVIEW_SAMPLE_COUNT, MAX_SAMPLE_ROW_COUNT, SettingsStore, validate_sample_count,
};
use csv_anonymizer_core::{
    AnonymizationStrategy, ColumnControl, HeadersData, PasteAnalyzeData, PasteAnalyzeParams,
    PastePreviewParams, PasteTransformData, PasteTransformParams, PreflightData, PreflightMode,
    PreflightParams, PreviewData, PreviewParams, QuickGenerateParams, QuickTransformData,
    SmartReplacementEntry, SmartReplacementProvider, should_auto_select_column,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    pub headers: HeadersData,
    pub selected_columns: Vec<usize>,
    pub suggested_output_path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequest {
    pub file_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub sample_count: usize,
    pub local_ai: Option<LocalAiRequest>,
}

#[derive(Debug, Clone, Deserialize)]
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PastePreviewRequest {
    #[serde(flatten)]
    pub params: PastePreviewParams,
    pub local_ai: Option<LocalAiRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteTransformRequest {
    #[serde(flatten)]
    pub params: PasteTransformParams,
    pub local_ai: Option<LocalAiRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickGenerateRequest {
    #[serde(flatten)]
    pub params: QuickGenerateParams,
    pub local_ai: Option<LocalAiRequest>,
}

fn load_local_ai_enabled(settings: &State<'_, Arc<SettingsStore>>) -> Result<bool, String> {
    settings
        .load_settings()
        .map(|settings| settings.local_ai_enabled)
        .map_err(|error| format!("Could not load settings: {error}"))
}

#[tauri::command]
pub async fn analyze_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    file_path: PathBuf,
    sample_row_count: usize,
    output_suffix: String,
) -> Result<AnalyzeResponse, String> {
    validate_sample_count(sample_row_count, MAX_SAMPLE_ROW_COUNT, "Sample row count")?;
    let file_path = authorize_or_confirm_input_file(&app, &path_access, file_path)?;
    // The suggested output path is only a suggestion: write access is granted
    // later through the explicit confirm/save-dialog flow, never silently here.
    run_blocking(move || {
        let service = service();
        let headers = service
            .analyze_csv_sampled(&file_path, sample_row_count)
            .map_err(|error| error.to_string())?;
        let selected_columns = headers
            .columns
            .iter()
            .filter(|column| should_auto_select_column(column))
            .map(|column| column.index)
            .collect::<Vec<_>>();
        let suggested_output_path =
            default_output_path_with_suffix(&headers.file_path, &output_suffix)?;

        Ok(AnalyzeResponse {
            headers,
            selected_columns,
            suggested_output_path,
        })
    })
    .await
}

#[tauri::command]
pub async fn preview_anonymization(
    path_access: State<'_, PathAccess>,
    settings: State<'_, Arc<SettingsStore>>,
    request: PreviewRequest,
) -> Result<PreviewData, String> {
    validate_sample_count(
        request.sample_count,
        MAX_PREVIEW_SAMPLE_COUNT,
        "Preview sample count",
    )?;
    let file_path = path_access.authorize_input_file(request.file_path)?;
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
    run_blocking(move || {
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
            .preview_anonymization_with_smart_provider(
                PreviewParams {
                    file_path,
                    columns: request.columns,
                    controls: request.controls,
                    sample_count: request.sample_count,
                },
                provider,
            )
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn preflight_anonymization(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    settings: State<'_, Arc<SettingsStore>>,
    request: PreflightRequest,
) -> Result<PreflightData, String> {
    validate_sample_count(
        request.sample_row_count,
        MAX_SAMPLE_ROW_COUNT,
        "Sample row count",
    )?;
    let mode = request.mode;
    let file_path = authorize_or_confirm_input_file(&app, &path_access, request.file_path.clone())?;
    let output_path = match (mode, request.output_path.clone()) {
        (PreflightMode::Anonymize, Some(path)) => {
            Some(authorize_or_confirm_output_file(&app, &path_access, path)?)
        }
        (_, output_path) => output_path,
    };
    let local_ai_enabled = load_local_ai_enabled(&settings)?;

    run_blocking(move || {
        let local_ai_required = request.controls.iter().any(|control| {
            request.columns.contains(&control.column_index)
                && control.strategy == AnonymizationStrategy::LocalAi
        });
        let (local_ai_ready, local_ai_message) = if local_ai_required && !local_ai_enabled {
            (
                false,
                Some(
                    "Local AI is off. Enable it in Settings before choosing Smart replacement."
                        .to_string(),
                ),
            )
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
                file_path,
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
}

#[tauri::command]
pub async fn count_csv_rows(
    path_access: State<'_, PathAccess>,
    file_path: PathBuf,
) -> Result<usize, String> {
    let file_path = path_access.authorize_input_file(file_path)?;
    run_blocking(move || {
        service()
            .count_csv_rows(&file_path)
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn analyze_pasted_data(request: PasteAnalyzeParams) -> Result<PasteAnalyzeData, String> {
    run_blocking(move || {
        csv_anonymizer_core::direct_input::analyze_paste_data(request)
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn preview_pasted_data(
    settings: State<'_, Arc<SettingsStore>>,
    request: PastePreviewRequest,
) -> Result<PreviewData, String> {
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
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
        csv_anonymizer_core::direct_input::preview_paste_data_with_smart_provider(
            request.params,
            provider,
        )
        .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn anonymize_pasted_data(
    settings: State<'_, Arc<SettingsStore>>,
    request: PasteTransformRequest,
) -> Result<PasteTransformData, String> {
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
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
        csv_anonymizer_core::direct_input::transform_paste_data_with_smart_provider(
            request.params,
            provider,
        )
        .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn generate_quick_values(
    settings: State<'_, Arc<SettingsStore>>,
    request: QuickGenerateRequest,
) -> Result<QuickTransformData, String> {
    let local_ai_enabled = load_local_ai_enabled(&settings)?;
    run_blocking(move || {
        let mut provider = smart_provider_for_strategy(
            request.local_ai,
            request.params.strategy,
            local_ai_enabled,
        )?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        csv_anonymizer_core::direct_input::generate_quick_values_with_smart_provider(
            request.params,
            provider,
        )
        .map_err(|error| error.to_string())
    })
    .await
}
