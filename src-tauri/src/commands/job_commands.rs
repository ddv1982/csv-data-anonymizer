use super::shared::authorize_or_confirm_output_file;
use crate::jobs::{AnonymizeJobStatus, AnonymizeJobStore, run_anonymize_job};
use crate::local_ai::LocalAiRequest;
use crate::path_access::PathAccess;
use crate::settings::SettingsStore;
use csv_anonymizer_core::{AnonymizeParams, ColumnControl, SmartReplacementEntry};
use serde::Deserialize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAnonymizeJobRequest {
    pub file_path: PathBuf,
    pub output_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub force: bool,
    pub sample_row_count: usize,
    pub total_row_count: Option<usize>,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
    pub local_ai: Option<LocalAiRequest>,
}

#[tauri::command]
pub async fn start_anonymize_job(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    settings: State<'_, Arc<SettingsStore>>,
    jobs: State<'_, AnonymizeJobStore>,
    request: StartAnonymizeJobRequest,
) -> Result<AnonymizeJobStatus, String> {
    let file_path = path_access.authorize_input_file(request.file_path)?;
    let output_path = authorize_or_confirm_output_file(&app, &path_access, request.output_path)?;
    let local_ai_enabled = settings
        .load_settings()
        .map(|settings| settings.local_ai_enabled)
        .map_err(|error| format!("Could not load settings: {error}"))?;
    let job = jobs.create_job(request.total_row_count)?;
    let initial_status = job.snapshot()?;
    let worker_job = job.clone();
    let panic_job = job.clone();

    let _job_handle = tauri::async_runtime::spawn_blocking(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            run_anonymize_job(
                worker_job,
                AnonymizeParams {
                    file_path,
                    output_path,
                    columns: request.columns,
                    controls: request.controls,
                    force: request.force,
                    preview_smart_replacements: request.preview_smart_replacements,
                },
                request.sample_row_count,
                request.local_ai,
                local_ai_enabled,
            );
        }));
        if result.is_err() {
            panic_job.finish_panic();
        }
    });

    Ok(initial_status)
}

#[tauri::command]
pub fn get_anonymize_job_status(
    jobs: State<'_, AnonymizeJobStore>,
    job_id: String,
) -> Result<AnonymizeJobStatus, String> {
    jobs.snapshot_job(&job_id)
}

#[tauri::command]
pub fn cancel_anonymize_job(
    jobs: State<'_, AnonymizeJobStore>,
    job_id: String,
) -> Result<AnonymizeJobStatus, String> {
    let job = jobs.get_job(&job_id)?;
    let status = job.snapshot()?;
    if status.state.is_terminal() {
        return Ok(status);
    }
    job.request_cancel()
}
