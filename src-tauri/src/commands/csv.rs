use super::shared::{
    authorize_or_confirm_input_file, default_output_path_with_suffix, run_blocking, service,
    should_auto_select,
};
use crate::local_ai::{LocalAiRequest, smart_provider_for_request};
use crate::path_access::PathAccess;
use csv_anonymizer_core::{
    ColumnControl, HeadersData, PasteAnalyzeData, PasteAnalyzeParams, PastePreviewParams,
    PasteTransformData, PasteTransformParams, PreviewData, PreviewParams, QuickGenerateParams,
    QuickTransformData, SmartReplacementProvider,
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
pub async fn analyze_pasted_data(request: PasteAnalyzeParams) -> Result<PasteAnalyzeData, String> {
    run_blocking(move || {
        csv_anonymizer_core::direct_input::analyze_paste_data(request)
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn preview_pasted_data(request: PastePreviewParams) -> Result<PreviewData, String> {
    run_blocking(move || {
        csv_anonymizer_core::direct_input::preview_paste_data(request)
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn anonymize_pasted_data(
    request: PasteTransformParams,
) -> Result<PasteTransformData, String> {
    run_blocking(move || {
        csv_anonymizer_core::direct_input::transform_paste_data(request)
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn generate_quick_values(
    request: QuickGenerateParams,
) -> Result<QuickTransformData, String> {
    run_blocking(move || {
        csv_anonymizer_core::direct_input::generate_quick_values(request)
            .map_err(|error| error.to_string())
    })
    .await
}
