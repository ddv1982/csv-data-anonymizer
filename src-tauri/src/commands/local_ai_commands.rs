use crate::local_ai::{
    LocalAiDownloadState, LocalAiDownloadStatus, LocalAiDownloadStore, LocalAiRequest,
    LocalAiStatus, ensure_ollama_runtime_available, local_ai_status, open_setup_url,
    start_download_job,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use tauri::State;

#[tauri::command]
pub async fn get_local_ai_status(request: LocalAiRequest) -> Result<LocalAiStatus, String> {
    super::shared::run_blocking(move || local_ai_status(request)).await
}

#[tauri::command]
pub async fn start_local_ai_model_download(
    downloads: State<'_, LocalAiDownloadStore>,
    request: LocalAiRequest,
) -> Result<LocalAiDownloadStatus, String> {
    // The runtime probe is a blocking HTTP call (up to 120s); keep it off the
    // main thread like the sibling status command.
    super::shared::run_blocking(ensure_ollama_runtime_available).await?;
    let job = downloads.create_job(request.model_name())?;
    let initial_status = job.snapshot()?;
    let worker_job = job.clone();
    let panic_job = job.clone();
    let _job_handle = tauri::async_runtime::spawn_blocking(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            start_download_job(worker_job, request);
        }));
        if result.is_err() {
            panic_job.finish_panic();
        }
    });
    Ok(initial_status)
}

#[tauri::command]
pub fn get_local_ai_model_download_status(
    downloads: State<'_, LocalAiDownloadStore>,
    job_id: String,
) -> Result<LocalAiDownloadStatus, String> {
    downloads.snapshot_job(&job_id)
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
