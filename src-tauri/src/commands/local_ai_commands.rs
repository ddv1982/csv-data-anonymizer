use crate::local_ai::{
    LocalAiDownloadState, LocalAiDownloadStatus, LocalAiDownloadStore, LocalAiRequest,
    LocalAiStatus, local_ai_status, open_setup_url, start_download_job,
};
use tauri::State;

#[tauri::command]
pub async fn get_local_ai_status(request: LocalAiRequest) -> Result<LocalAiStatus, String> {
    super::shared::run_blocking(move || local_ai_status(request)).await
}

#[tauri::command]
pub fn start_local_ai_model_download(
    downloads: State<'_, LocalAiDownloadStore>,
    request: LocalAiRequest,
) -> Result<LocalAiDownloadStatus, String> {
    let job = downloads.create_job(request.model_name())?;
    let initial_status = job.snapshot()?;
    let worker_job = job.clone();
    let _job_handle = tauri::async_runtime::spawn_blocking(move || {
        start_download_job(worker_job, request);
    });
    Ok(initial_status)
}

#[tauri::command]
pub fn get_local_ai_model_download_status(
    downloads: State<'_, LocalAiDownloadStore>,
    job_id: String,
) -> Result<LocalAiDownloadStatus, String> {
    downloads.get_job(&job_id)?.snapshot()
}

#[tauri::command]
pub fn cancel_local_ai_model_download(
    downloads: State<'_, LocalAiDownloadStore>,
    job_id: String,
) -> Result<LocalAiDownloadStatus, String> {
    let job = downloads.get_job(&job_id)?;
    let status = job.snapshot()?;
    if matches!(
        status.state,
        LocalAiDownloadState::Succeeded
            | LocalAiDownloadState::Failed
            | LocalAiDownloadState::Canceled
    ) {
        return Ok(status);
    }
    job.request_cancel()
}

#[tauri::command]
pub fn open_local_ai_setup_url() -> Result<(), String> {
    open_setup_url()
}
