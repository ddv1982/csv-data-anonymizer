use crate::local_ai::{LocalAiRequest, smart_provider_for_request};
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerError, AnonymizerService, ProcessControl,
    ProcessProgress, SmartReplacementProvider,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AnonymizeJobState {
    Running,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnonymizeJobStatus {
    pub job_id: String,
    pub state: AnonymizeJobState,
    pub rows_processed: usize,
    pub total_rows: Option<usize>,
    pub cancel_requested: bool,
    pub result: Option<AnonymizeData>,
    pub error: Option<String>,
}

#[derive(Debug, Default)]
pub struct AnonymizeJobStore {
    next_id: AtomicU64,
    jobs: Mutex<HashMap<String, Arc<AnonymizeJob>>>,
}

#[derive(Debug)]
pub struct AnonymizeJob {
    cancel_requested: AtomicBool,
    status: Mutex<AnonymizeJobStatus>,
}

impl AnonymizeJobStore {
    pub fn create_job(&self, total_rows: Option<usize>) -> Result<Arc<AnonymizeJob>, String> {
        let id = format!(
            "job-{}-{}",
            std::process::id(),
            self.next_id.fetch_add(1, Ordering::Relaxed) + 1
        );
        let job = Arc::new(AnonymizeJob {
            cancel_requested: AtomicBool::new(false),
            status: Mutex::new(AnonymizeJobStatus {
                job_id: id.clone(),
                state: AnonymizeJobState::Running,
                rows_processed: 0,
                total_rows,
                cancel_requested: false,
                result: None,
                error: None,
            }),
        });

        self.lock_jobs()?.insert(id, job.clone());
        Ok(job)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Arc<AnonymizeJob>, String> {
        self.lock_jobs()?
            .get(job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown anonymization job: {job_id}"))
    }

    fn lock_jobs(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, Arc<AnonymizeJob>>>, String> {
        self.jobs
            .lock()
            .map_err(|_| "Anonymization job store is unavailable.".to_string())
    }
}

impl AnonymizeJob {
    pub fn snapshot(&self) -> Result<AnonymizeJobStatus, String> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| "Anonymization job status is unavailable.".to_string())
    }

    pub fn report_progress(&self, rows_processed: usize) {
        if let Ok(mut status) = self.status.lock()
            && status.state == AnonymizeJobState::Running
        {
            status.rows_processed = rows_processed;
        }
    }

    pub fn request_cancel(&self) -> Result<AnonymizeJobStatus, String> {
        self.cancel_requested.store(true, Ordering::SeqCst);
        {
            let mut status = self.lock_status()?;
            if status.state == AnonymizeJobState::Running {
                status.cancel_requested = true;
            }
        }
        self.snapshot()
    }

    pub fn should_cancel(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }

    pub fn finish(&self, result: Result<AnonymizeData, AnonymizerError>) {
        if let Ok(mut status) = self.status.lock() {
            match result {
                Ok(data) => {
                    status.rows_processed = data.row_count;
                    status.state = AnonymizeJobState::Succeeded;
                    status.result = Some(data);
                    status.error = None;
                }
                Err(AnonymizerError::Canceled) => {
                    status.state = AnonymizeJobState::Canceled;
                    status.cancel_requested = true;
                    status.error = None;
                }
                Err(error) => {
                    status.state = AnonymizeJobState::Failed;
                    status.error = Some(error.to_string());
                }
            }
        }
    }

    fn lock_status(&self) -> Result<std::sync::MutexGuard<'_, AnonymizeJobStatus>, String> {
        self.status
            .lock()
            .map_err(|_| "Anonymization job status is unavailable.".to_string())
    }
}

pub fn run_anonymize_job(
    job: Arc<AnonymizeJob>,
    input: AnonymizeParams,
    sample_row_count: usize,
    local_ai: Option<LocalAiRequest>,
) {
    let progress_job = job.clone();
    let mut on_progress = move |progress: ProcessProgress| {
        progress_job.report_progress(progress.rows_processed);
    };
    let cancel_job = job.clone();
    let should_cancel = move || cancel_job.should_cancel();
    let mut control = ProcessControl {
        on_progress: Some(&mut on_progress),
        should_cancel: Some(&should_cancel),
    };

    let result = match smart_provider_for_request(local_ai, &input.controls) {
        Ok(mut provider) => {
            let provider = provider
                .as_mut()
                .map(|provider| provider as &mut dyn SmartReplacementProvider);
            service().anonymize_csv_with_sample_rows_and_control_and_smart_provider(
                input,
                sample_row_count,
                Some(&mut control),
                provider,
            )
        }
        Err(error) => Err(AnonymizerError::SmartReplacement(error)),
    };
    job.finish(result);
}

fn service() -> AnonymizerService {
    AnonymizerService::new(env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv_anonymizer_core::{
        AnonymizationStrategy, ColumnControl, DataType, SmartReplacementEntry,
    };
    use std::fs;

    #[test]
    fn creates_running_job_snapshots() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job(Some(10)).unwrap();

        let status = job.snapshot().unwrap();

        assert_eq!(status.state, AnonymizeJobState::Running);
        assert_eq!(status.rows_processed, 0);
        assert_eq!(status.total_rows, Some(10));
    }

    #[test]
    fn cancel_request_updates_status_and_flag() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job(None).unwrap();

        let status = job.request_cancel().unwrap();

        assert!(job.should_cancel());
        assert_eq!(status.state, AnonymizeJobState::Running);
        assert!(status.cancel_requested);
    }

    #[test]
    fn job_writes_output_when_preview_replacements_cover_smart_values() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input_path = temp_dir.path().join("smart-covered.csv");
        let output_path = temp_dir.path().join("smart-covered-output.csv");
        fs::write(&input_path, "name\nAlice Smith\nBob Stone\n").unwrap();
        let store = AnonymizeJobStore::default();
        let job = store.create_job(Some(2)).unwrap();

        run_anonymize_job(
            job.clone(),
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls: vec![ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::FullName),
                    strategy: AnonymizationStrategy::LocalAi,
                }],
                deterministic: true,
                seed: "smart-covered-seed".to_string(),
                force: false,
                preview_smart_replacements: vec![
                    SmartReplacementEntry {
                        column_index: 0,
                        original: "Alice Smith".to_string(),
                        replacement: "Preview Alice".to_string(),
                    },
                    SmartReplacementEntry {
                        column_index: 0,
                        original: "Bob Stone".to_string(),
                        replacement: "Preview Bob".to_string(),
                    },
                ],
            },
            10,
            None,
        );

        let status = job.snapshot().unwrap();
        let output = fs::read_to_string(&output_path).unwrap();

        assert_eq!(status.state, AnonymizeJobState::Succeeded);
        assert!(status.result.is_some());
        assert!(output.contains("Preview Alice"));
        assert!(output.contains("Preview Bob"));
    }

    #[test]
    fn job_writes_output_for_standard_columns_without_preview() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input_path = temp_dir.path().join("standard.csv");
        let output_path = temp_dir.path().join("standard-output.csv");
        fs::write(
            &input_path,
            "email,name\nalice@example.com,Alice\nbob@example.com,Bob\n",
        )
        .unwrap();
        let store = AnonymizeJobStore::default();
        let job = store.create_job(Some(2)).unwrap();

        run_anonymize_job(
            job.clone(),
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls: vec![],
                deterministic: true,
                seed: "standard-seed".to_string(),
                force: false,
                preview_smart_replacements: vec![],
            },
            10,
            None,
        );

        let status = job.snapshot().unwrap();
        let output = fs::read_to_string(&output_path).unwrap();

        assert_eq!(status.state, AnonymizeJobState::Succeeded);
        assert!(status.result.is_some());
        assert!(output.contains("example.com"));
        assert!(!output.contains("alice@example.com"));
    }

    #[test]
    fn job_writes_output_for_standard_columns_with_preview_replacements_present() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input_path = temp_dir.path().join("standard-preview.csv");
        let output_path = temp_dir.path().join("standard-preview-output.csv");
        fs::write(
            &input_path,
            "email,name\nalice@example.com,Alice\nbob@example.com,Bob\n",
        )
        .unwrap();
        let store = AnonymizeJobStore::default();
        let job = store.create_job(Some(2)).unwrap();

        run_anonymize_job(
            job.clone(),
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls: vec![],
                deterministic: true,
                seed: "standard-preview-seed".to_string(),
                force: false,
                preview_smart_replacements: vec![SmartReplacementEntry {
                    column_index: 0,
                    original: "Alice Smith".to_string(),
                    replacement: "Preview Alice".to_string(),
                }],
            },
            10,
            None,
        );

        let status = job.snapshot().unwrap();
        let output = fs::read_to_string(&output_path).unwrap();

        assert_eq!(status.state, AnonymizeJobState::Succeeded);
        assert!(status.result.is_some());
        assert!(output.contains("example.com"));
        assert!(!output.contains("alice@example.com"));
    }

    #[test]
    fn job_fails_clearly_when_smart_generation_needs_unavailable_local_ai() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input_path = temp_dir.path().join("smart-missing-provider.csv");
        let output_path = temp_dir.path().join("smart-missing-provider-output.csv");
        fs::write(&input_path, "name\nAlice Smith\n").unwrap();
        let store = AnonymizeJobStore::default();
        let job = store.create_job(Some(1)).unwrap();

        run_anonymize_job(
            job.clone(),
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls: vec![ColumnControl {
                    column_index: 0,
                    type_override: Some(DataType::FullName),
                    strategy: AnonymizationStrategy::LocalAi,
                }],
                deterministic: true,
                seed: "smart-missing-provider-seed".to_string(),
                force: false,
                preview_smart_replacements: vec![],
            },
            10,
            None,
        );

        let status = job.snapshot().unwrap();

        assert_eq!(status.state, AnonymizeJobState::Failed);
        assert!(status.result.is_none());
        assert!(
            status
                .error
                .as_deref()
                .is_some_and(|error| error.contains("Smart replacement needs Local AI"))
        );
        assert!(!output_path.exists());
    }
}
