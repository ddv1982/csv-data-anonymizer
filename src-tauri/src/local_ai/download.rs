use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use super::types::{LocalAiDownloadState, LocalAiDownloadStatus, LocalAiRequest};
use super::{DEFAULT_OLLAMA_ENDPOINT, OLLAMA_UNAVAILABLE_MESSAGE, download_client};

pub(super) const MAX_RETAINED_DOWNLOAD_JOBS: usize = 10;
const TERMINAL_DOWNLOAD_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Default)]
pub struct LocalAiDownloadStore {
    next_id: AtomicU64,
    jobs: Mutex<HashMap<String, Arc<LocalAiDownloadJob>>>,
}

#[derive(Debug)]
pub struct LocalAiDownloadJob {
    created_sequence: u64,
    cancel_requested: AtomicBool,
    status: Mutex<LocalAiDownloadStatus>,
    terminal_at: Mutex<Option<SystemTime>>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaPullProgress {
    status: Option<String>,
    completed: Option<u64>,
    total: Option<u64>,
    error: Option<String>,
}

impl LocalAiDownloadStore {
    pub fn create_job(&self, model: String) -> Result<Arc<LocalAiDownloadJob>, String> {
        let sequence = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let id = format!("ai-model-{}-{sequence}", std::process::id());
        let job = Arc::new(LocalAiDownloadJob {
            created_sequence: sequence,
            cancel_requested: AtomicBool::new(false),
            terminal_at: Mutex::new(None),
            status: Mutex::new(LocalAiDownloadStatus {
                job_id: id.clone(),
                state: LocalAiDownloadState::Running,
                model,
                status_message: "Starting model download...".to_string(),
                completed_bytes: None,
                total_bytes: None,
                cancel_requested: false,
                error: None,
            }),
        });
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "Local AI download store is unavailable.".to_string())?;
        jobs.insert(id, job.clone());
        prune_terminal_download_jobs(&mut jobs, None, MAX_RETAINED_DOWNLOAD_JOBS);
        Ok(job)
    }

    pub fn snapshot_job(&self, job_id: &str) -> Result<LocalAiDownloadStatus, String> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "Local AI download store is unavailable.".to_string())?;
        let job = jobs
            .get(job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown Local AI download job: {job_id}"))?;
        let status = job.snapshot()?;
        prune_terminal_download_jobs(&mut jobs, Some(job_id), MAX_RETAINED_DOWNLOAD_JOBS);
        if status.state.is_terminal() {
            jobs.remove(job_id);
        }
        Ok(status)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Arc<LocalAiDownloadJob>, String> {
        let mut jobs = self
            .jobs
            .lock()
            .map_err(|_| "Local AI download store is unavailable.".to_string())?;
        let job = jobs
            .get(job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown Local AI download job: {job_id}"))?;
        prune_terminal_download_jobs(&mut jobs, Some(job_id), MAX_RETAINED_DOWNLOAD_JOBS);
        Ok(job)
    }

    #[cfg(test)]
    fn job_count(&self) -> usize {
        self.jobs.lock().map(|jobs| jobs.len()).unwrap_or_default()
    }
}

impl LocalAiDownloadJob {
    pub fn snapshot(&self) -> Result<LocalAiDownloadStatus, String> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| "Local AI download status is unavailable.".to_string())
    }

    pub fn request_cancel(&self) -> Result<LocalAiDownloadStatus, String> {
        self.cancel_requested.store(true, Ordering::SeqCst);
        if let Ok(mut status) = self.status.lock()
            && status.state == LocalAiDownloadState::Running
        {
            status.cancel_requested = true;
            status.status_message = "Canceling model download...".to_string();
        }
        self.snapshot()
    }

    pub(super) fn should_cancel(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }

    fn report_progress(&self, progress: OllamaPullProgress) {
        if let Ok(mut status) = self.status.lock()
            && status.state == LocalAiDownloadState::Running
        {
            if let Some(message) = progress.status {
                status.status_message = message;
            }
            status.completed_bytes = progress.completed;
            status.total_bytes = progress.total;
        }
    }

    pub(super) fn finish_success(&self) {
        if let Ok(mut status) = self.status.lock() {
            status.state = LocalAiDownloadState::Succeeded;
            status.status_message = format!("{} is ready for Local AI.", status.model);
            status.cancel_requested = false;
            status.error = None;
        }
        self.mark_terminal();
    }

    pub(super) fn finish_canceled(&self) {
        if let Ok(mut status) = self.status.lock() {
            status.state = LocalAiDownloadState::Canceled;
            status.status_message = "Model download canceled.".to_string();
            status.cancel_requested = true;
            status.error = None;
        }
        self.mark_terminal();
    }

    pub(super) fn finish_error(&self, error: String) {
        if let Ok(mut status) = self.status.lock() {
            status.state = LocalAiDownloadState::Failed;
            status.error = Some(error.clone());
            status.status_message = error;
        }
        self.mark_terminal();
    }

    fn mark_terminal(&self) {
        if let Ok(mut terminal_at) = self.terminal_at.lock() {
            *terminal_at = Some(SystemTime::now());
        }
    }
}

pub fn start_download_job(job: Arc<LocalAiDownloadJob>, request: LocalAiRequest) {
    let result = download_model(job.clone(), request.model_name());
    match result {
        Ok(()) if job.should_cancel() => job.finish_canceled(),
        Ok(()) => job.finish_success(),
        Err(_) if job.should_cancel() => job.finish_canceled(),
        Err(error) => job.finish_error(error),
    }
}

fn prune_terminal_download_jobs(
    jobs: &mut HashMap<String, Arc<LocalAiDownloadJob>>,
    protected_job_id: Option<&str>,
    max_retained: usize,
) {
    let now = SystemTime::now();
    jobs.retain(|job_id, job| {
        protected_job_id == Some(job_id.as_str())
            || !terminal_download_expired(job, now, TERMINAL_DOWNLOAD_TTL)
    });

    let mut terminal_jobs = jobs
        .iter()
        .filter(|(job_id, _)| protected_job_id != Some(job_id.as_str()))
        .filter_map(|(job_id, job)| {
            job.snapshot()
                .ok()
                .filter(|status| status.state.is_terminal())
                .map(|_| (job_id.clone(), job.created_sequence))
        })
        .collect::<Vec<_>>();
    if terminal_jobs.len() <= max_retained {
        return;
    }

    terminal_jobs.sort_by_key(|(_, sequence)| *sequence);
    let remove_count = terminal_jobs.len() - max_retained;
    for (job_id, _) in terminal_jobs.into_iter().take(remove_count) {
        jobs.remove(&job_id);
    }
}

fn terminal_download_expired(job: &LocalAiDownloadJob, now: SystemTime, ttl: Duration) -> bool {
    let Ok(terminal_at) = job.terminal_at.lock() else {
        return false;
    };
    let Some(terminal_at) = *terminal_at else {
        return false;
    };
    match now.duration_since(terminal_at) {
        Ok(age) => age >= ttl,
        Err(_) => false,
    }
}

fn download_model(job: Arc<LocalAiDownloadJob>, model: String) -> Result<(), String> {
    let client = download_client()?;
    let response = client
        .post(format!("{DEFAULT_OLLAMA_ENDPOINT}/api/pull"))
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .map_err(|error| {
            if error.is_connect() {
                OLLAMA_UNAVAILABLE_MESSAGE.to_string()
            } else {
                format!("Could not start Ollama model download: {error}")
            }
        })?
        .error_for_status()
        .map_err(|error| format!("Ollama model download failed: {error}"))?;
    let reader = BufReader::new(response);
    for line in reader.lines() {
        if job.should_cancel() {
            return Ok(());
        }
        let line =
            line.map_err(|error| format!("Could not read Ollama download progress: {error}"))?;
        if line.trim().is_empty() {
            continue;
        }
        let progress = serde_json::from_str::<OllamaPullProgress>(&line)
            .map_err(|error| format!("Ollama returned invalid download progress: {error}"))?;
        if let Some(error) = progress.error {
            return Err(format!("Ollama model download failed: {error}"));
        }
        job.report_progress(progress);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn age_terminal_download(job: &LocalAiDownloadJob) {
        *job.terminal_at.lock().unwrap() =
            Some(SystemTime::now() - TERMINAL_DOWNLOAD_TTL - Duration::from_secs(1));
    }

    #[test]
    fn download_store_prunes_old_terminal_jobs_but_keeps_running_jobs() {
        let store = LocalAiDownloadStore::default();
        let running_job = store.create_job("gemma3:4b".to_string()).unwrap();

        for index in 0..(MAX_RETAINED_DOWNLOAD_JOBS + 3) {
            let job = store.create_job(format!("model-{index}")).unwrap();
            job.finish_error("failed".to_string());
        }
        let trigger_job = store.create_job("trigger".to_string()).unwrap();

        assert_eq!(store.job_count(), MAX_RETAINED_DOWNLOAD_JOBS + 2);
        assert!(
            store
                .get_job(&running_job.snapshot().unwrap().job_id)
                .is_ok()
        );
        assert!(
            store
                .get_job(&trigger_job.snapshot().unwrap().job_id)
                .is_ok()
        );
    }

    #[test]
    fn download_store_prunes_all_terminal_states_and_retains_newest() {
        let store = LocalAiDownloadStore::default();
        let mut terminal_ids = Vec::new();

        for index in 0..(MAX_RETAINED_DOWNLOAD_JOBS + 3) {
            let job = store.create_job(format!("model-{index}")).unwrap();
            match index % 3 {
                0 => job.finish_success(),
                1 => job.finish_canceled(),
                _ => job.finish_error("failed".to_string()),
            }
            terminal_ids.push(job.snapshot().unwrap().job_id);
        }
        let trigger_job = store.create_job("trigger".to_string()).unwrap();

        assert_eq!(store.job_count(), MAX_RETAINED_DOWNLOAD_JOBS + 1);
        assert!(store.get_job(&terminal_ids[0]).is_err());
        assert!(
            store
                .get_job(terminal_ids.last().expect("terminal id should exist"))
                .is_ok()
        );
        assert!(
            store
                .get_job(&trigger_job.snapshot().unwrap().job_id)
                .is_ok()
        );
    }

    #[test]
    fn download_store_protects_requested_terminal_job_during_prune() {
        let store = LocalAiDownloadStore::default();
        for index in 0..(MAX_RETAINED_DOWNLOAD_JOBS + 3) {
            let job = store.create_job(format!("model-{index}")).unwrap();
            job.finish_error("failed".to_string());
        }
        let protected = store.create_job("protected".to_string()).unwrap();
        protected.finish_success();
        let protected_id = protected.snapshot().unwrap().job_id;

        assert!(store.get_job(&protected_id).is_ok());
        assert_eq!(store.job_count(), MAX_RETAINED_DOWNLOAD_JOBS + 1);
    }

    #[test]
    fn download_store_removes_terminal_job_after_status_retrieval() {
        let store = LocalAiDownloadStore::default();
        let job = store.create_job("gemma3:4b".to_string()).unwrap();
        let job_id = job.snapshot().unwrap().job_id;
        job.finish_success();

        let status = store.snapshot_job(&job_id).unwrap();

        assert_eq!(status.state, LocalAiDownloadState::Succeeded);
        assert!(store.get_job(&job_id).is_err());
        assert_eq!(store.job_count(), 0);
    }

    #[test]
    fn download_store_prunes_terminal_jobs_after_ttl() {
        let store = LocalAiDownloadStore::default();
        let old_job = store.create_job("old".to_string()).unwrap();
        old_job.finish_error("failed".to_string());
        age_terminal_download(&old_job);
        let old_job_id = old_job.snapshot().unwrap().job_id;

        let active_job = store.create_job("active".to_string()).unwrap();

        assert!(store.get_job(&old_job_id).is_err());
        assert!(
            store
                .get_job(&active_job.snapshot().unwrap().job_id)
                .is_ok()
        );
    }
}
