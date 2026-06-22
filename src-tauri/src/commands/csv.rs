use super::shared::{
    authorize_or_confirm_input_file, authorize_or_confirm_output_file,
    default_output_path_with_suffix, run_blocking, service, should_auto_select,
};
use crate::local_ai::{LocalAiRequest, smart_provider_for_request};
use crate::path_access::PathAccess;
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, ColumnControl, HeadersData, PreviewData, PreviewParams,
    PrivacyConfig, SmartReplacementEntry, SmartReplacementProvider,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
pub struct AnonymizeRequest {
    pub file_path: PathBuf,
    pub output_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub deterministic: bool,
    pub seed: String,
    pub force: bool,
    pub sample_row_count: usize,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
    #[serde(default)]
    pub privacy_config: Option<PrivacyConfig>,
    pub local_ai: Option<LocalAiRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequest {
    pub file_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub deterministic: bool,
    pub seed: String,
    pub sample_count: usize,
    pub local_ai: Option<LocalAiRequest>,
}

#[tauri::command]
pub async fn analyze_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    file_path: PathBuf,
    sample_row_count: usize,
    output_suffix: String,
) -> Result<AnalyzeResponse, String> {
    let file_path = authorize_or_confirm_input_file(&app, &path_access, file_path)?;
    let mut response = run_blocking(move || {
        let service = service();
        let headers = service
            .analyze_csv_sampled(&file_path, sample_row_count)
            .map_err(|error| error.to_string())?;
        let selected_columns = headers
            .columns
            .iter()
            .filter(|column| should_auto_select(column))
            .map(|column| column.index)
            .collect::<Vec<_>>();
        let suggested_output_path =
            default_output_path_with_suffix(&headers.file_path, &output_suffix);

        Ok(AnalyzeResponse {
            headers,
            selected_columns,
            suggested_output_path,
        })
    })
    .await?;
    response.suggested_output_path =
        path_access.grant_output_file(&response.suggested_output_path)?;
    Ok(response)
}

#[tauri::command]
pub async fn preview_anonymization(
    path_access: State<'_, PathAccess>,
    request: PreviewRequest,
) -> Result<PreviewData, String> {
    let file_path = path_access.authorize_input_file(request.file_path)?;
    run_blocking(move || {
        let mut provider = smart_provider_for_request(request.local_ai, &request.controls)?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        service()
            .preview_anonymization_with_smart_provider(
                PreviewParams {
                    file_path,
                    columns: request.columns,
                    controls: request.controls,
                    deterministic: request.deterministic,
                    seed: request.seed,
                    sample_count: request.sample_count,
                },
                provider,
            )
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
pub async fn anonymize_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    request: AnonymizeRequest,
) -> Result<AnonymizeData, String> {
    let file_path = path_access.authorize_input_file(request.file_path)?;
    let output_path = authorize_or_confirm_output_file(&app, &path_access, request.output_path)?;
    run_blocking(move || {
        let mut provider = smart_provider_for_request(request.local_ai, &request.controls)?;
        let provider = provider
            .as_mut()
            .map(|provider| provider as &mut dyn SmartReplacementProvider);
        service()
            .anonymize_csv_with_sample_rows_and_control_and_smart_provider(
                AnonymizeParams {
                    file_path,
                    output_path,
                    columns: request.columns,
                    controls: request.controls,
                    deterministic: request.deterministic,
                    seed: request.seed,
                    force: request.force,
                    preview_smart_replacements: request.preview_smart_replacements,
                    privacy_config: request.privacy_config,
                },
                request.sample_row_count,
                None,
                provider,
            )
            .map_err(|error| error.to_string())
    })
    .await
}
