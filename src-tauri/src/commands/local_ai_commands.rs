use crate::command_error::CommandError;
use crate::local_ai::{
    LocalAiDownloadStatus, LocalAiDownloadStore, LocalAiRequest, LocalAiStatus,
    ensure_obviously_local_model, ensure_ollama_runtime_available, local_ai_status, open_setup_url,
    start_download_job,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
use tauri::State;

#[tauri::command]
pub async fn get_local_ai_status(request: LocalAiRequest) -> Result<LocalAiStatus, CommandError> {
    super::shared::run_blocking(move || local_ai_status(request))
        .await
        .map_err(Into::into)
}

#[tauri::command]
pub async fn start_local_ai_model_download(
    downloads: State<'_, LocalAiDownloadStore>,
    request: LocalAiRequest,
) -> Result<LocalAiDownloadStatus, CommandError> {
    ensure_obviously_local_model(&request.model_name())?;
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
) -> Result<LocalAiDownloadStatus, CommandError> {
    downloads.snapshot_job(&job_id).map_err(Into::into)
}

#[tauri::command]
pub fn cancel_local_ai_model_download(
    downloads: State<'_, LocalAiDownloadStore>,
    job_id: String,
) -> Result<LocalAiDownloadStatus, CommandError> {
    let job = downloads.get_job(&job_id)?;
    let status = job.snapshot()?;
    if status.state.is_terminal() {
        return Ok(status);
    }
    job.request_cancel().map_err(Into::into)
}

#[tauri::command]
pub fn open_local_ai_setup_url() -> Result<(), CommandError> {
    open_setup_url().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_request(value: serde_json::Value) -> LocalAiRequest {
        serde_json::from_value(value).expect("local ai request")
    }

    /// The download state enum is not nameable outside its module, so tests compare the
    /// wire form the frontend actually receives.
    fn state_name(status: &LocalAiDownloadStatus) -> String {
        serde_json::to_value(status.state)
            .expect("download state")
            .as_str()
            .expect("download state string")
            .to_string()
    }

    /// The download commands accept the Local AI request the frontend sends.
    ///
    /// `enabled` is the persisted consent flag travelling with every Local AI call; reading
    /// it under the wrong name would leave it false and refuse work the user opted into, or
    /// leave it true and start work the user did not.
    #[test]
    fn local_ai_request_accepts_the_payload_the_frontend_sends() {
        let request = parse_request(json!({ "enabled": true, "model": "gemma3:4b" }));

        assert!(request.enabled);
        assert_eq!(request.model, "gemma3:4b");
        assert_eq!(request.model_name(), "gemma3:4b");
    }

    /// A blank model name resolves to the default before a download job is created under it.
    ///
    /// `start_local_ai_model_download` labels the job with `model_name()`, so an unnormalized
    /// blank would leave the user watching a download for a model with no name.
    #[test]
    fn blank_model_names_resolve_before_a_download_job_is_labelled() {
        let downloads = LocalAiDownloadStore::default();
        let request = parse_request(json!({ "enabled": true, "model": "   " }));

        let job = downloads.create_job(request.model_name()).expect("job");

        let model = job.snapshot().expect("status").model;
        assert!(!model.trim().is_empty());
        assert_eq!(model, request.model_name());
    }

    /// Polling or cancelling a download id this app never issued fails instead of reporting one.
    ///
    /// Both commands do nothing but forward the id to the store, so this is the whole of their
    /// error handling.
    #[test]
    fn download_status_and_cancel_refuse_an_id_the_store_never_issued() {
        let downloads = LocalAiDownloadStore::default();
        let _live_job = downloads.create_job("gemma3:4b".to_string()).expect("job");

        let status_error = downloads.snapshot_job("ai-model-0-999").unwrap_err();
        let cancel_error = downloads.get_job("ai-model-0-999").unwrap_err();

        assert!(status_error.contains("Local AI download job"));
        assert!(status_error.contains("ai-model-0-999"));
        assert_eq!(status_error, cancel_error);
    }

    /// A failed download keeps reporting the failure, and never claims the model is ready.
    ///
    /// The UI enables Smart replacement off the terminal state of this job; a failure that
    /// read as success would offer a model that is not installed.
    #[test]
    fn a_failed_download_keeps_reporting_the_failure() {
        let downloads = LocalAiDownloadStore::default();
        let job = downloads.create_job("gemma3:4b".to_string()).expect("job");
        job.finish_panic();
        let job_id = job.snapshot().expect("status").job_id;

        let first = downloads.snapshot_job(&job_id).expect("first poll");
        let repeat = downloads.snapshot_job(&job_id).expect("repeat poll");

        assert_eq!(state_name(&first), "failed");
        assert!(first.error.is_some());
        assert!(first.state.is_terminal());
        assert_eq!(state_name(&repeat), "failed");
    }

    /// A cancel on a running download is recorded without yet declaring the download over.
    ///
    /// The stream is only stopped at the next chunk boundary, so the status has to say
    /// "canceling" rather than "canceled" until the worker actually finishes winding down.
    #[test]
    fn cancelling_a_running_download_records_the_request_before_the_worker_stops() {
        let downloads = LocalAiDownloadStore::default();
        let job = downloads.create_job("gemma3:4b".to_string()).expect("job");

        let status = job.request_cancel().expect("cancel");

        assert_eq!(state_name(&status), "running");
        assert!(!status.state.is_terminal());
        assert!(status.cancel_requested);
        assert!(status.status_message.contains("Canceling"));
    }

    /// The download status serializes under the names the frontend polls for.
    ///
    /// The progress bar reads the byte counters and the state off this object.
    #[test]
    fn download_status_serializes_the_names_the_frontend_polls_for() {
        let downloads = LocalAiDownloadStore::default();
        let job = downloads.create_job("gemma3:4b".to_string()).expect("job");
        let status = job.snapshot().expect("status");

        let value = serde_json::to_value(&status).expect("download status");

        assert_eq!(value["state"], json!("running"));
        assert_eq!(value["model"], json!("gemma3:4b"));
        assert_eq!(value["completedBytes"], json!(null));
        assert_eq!(value["totalBytes"], json!(null));
        assert_eq!(value["cancelRequested"], json!(false));
        assert!(
            value["statusMessage"]
                .as_str()
                .is_some_and(|m| !m.is_empty())
        );
        assert!(value["jobId"].as_str().is_some_and(|id| !id.is_empty()));
    }
}
