use crate::local_ai::{LocalAiRequest, smart_provider_for_request};
use crate::settings::DpBudgetLedger;
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerError, AnonymizerService, ProcessControl,
    ProcessProgress, SmartReplacementProvider,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

const MAX_RETAINED_TERMINAL_JOBS: usize = 20;
const TERMINAL_JOB_TTL: Duration = Duration::from_secs(30 * 60);

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
    created_sequence: u64,
    cancel_requested: AtomicBool,
    status: Mutex<AnonymizeJobStatus>,
    terminal_at: Mutex<Option<SystemTime>>,
}

impl AnonymizeJobStore {
    pub fn create_job(&self, total_rows: Option<usize>) -> Result<Arc<AnonymizeJob>, String> {
        let sequence = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let id = format!("job-{}-{sequence}", std::process::id());
        let job = Arc::new(AnonymizeJob {
            created_sequence: sequence,
            cancel_requested: AtomicBool::new(false),
            terminal_at: Mutex::new(None),
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

        let mut jobs = self.lock_jobs()?;
        jobs.insert(id, job.clone());
        prune_terminal_jobs(&mut jobs, None, MAX_RETAINED_TERMINAL_JOBS);
        Ok(job)
    }

    pub fn snapshot_job(&self, job_id: &str) -> Result<AnonymizeJobStatus, String> {
        let mut jobs = self.lock_jobs()?;
        let job = jobs
            .get(job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown anonymization job: {job_id}"))?;
        let status = job.snapshot()?;
        prune_terminal_jobs(&mut jobs, Some(job_id), MAX_RETAINED_TERMINAL_JOBS);
        if status.state.is_terminal() {
            jobs.remove(job_id);
        }
        Ok(status)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Arc<AnonymizeJob>, String> {
        let mut jobs = self.lock_jobs()?;
        let job = jobs
            .get(job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown anonymization job: {job_id}"))?;
        prune_terminal_jobs(&mut jobs, Some(job_id), MAX_RETAINED_TERMINAL_JOBS);
        Ok(job)
    }

    fn lock_jobs(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, HashMap<String, Arc<AnonymizeJob>>>, String> {
        self.jobs
            .lock()
            .map_err(|_| "Anonymization job store is unavailable.".to_string())
    }

    #[cfg(test)]
    fn job_count(&self) -> usize {
        self.jobs.lock().map(|jobs| jobs.len()).unwrap_or_default()
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
        if let Ok(mut terminal_at) = self.terminal_at.lock() {
            *terminal_at = Some(SystemTime::now());
        }
    }

    fn lock_status(&self) -> Result<std::sync::MutexGuard<'_, AnonymizeJobStatus>, String> {
        self.status
            .lock()
            .map_err(|_| "Anonymization job status is unavailable.".to_string())
    }
}

fn prune_terminal_jobs(
    jobs: &mut HashMap<String, Arc<AnonymizeJob>>,
    protected_job_id: Option<&str>,
    max_retained: usize,
) {
    let now = SystemTime::now();
    jobs.retain(|job_id, job| {
        protected_job_id == Some(job_id.as_str())
            || !terminal_job_expired(job, now, TERMINAL_JOB_TTL)
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

fn terminal_job_expired(job: &AnonymizeJob, now: SystemTime, ttl: Duration) -> bool {
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

impl AnonymizeJobState {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

pub fn run_anonymize_job(
    job: Arc<AnonymizeJob>,
    ledger: Arc<DpBudgetLedger>,
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
            ledger.run_with_budget(input, |input| {
                service().anonymize_csv_with_sample_rows_and_control_and_smart_provider(
                    input,
                    sample_row_count,
                    Some(&mut control),
                    provider,
                )
            })
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
        AnonymizationStrategy, ColumnControl, DataType, PrivacyReport, ReleaseMode,
        SmartReplacementEntry,
    };
    use std::fs;

    fn test_ledger(temp_dir: &tempfile::TempDir) -> Arc<DpBudgetLedger> {
        Arc::new(DpBudgetLedger::new(temp_dir.path().join("settings.json")))
    }

    fn age_terminal_job(job: &AnonymizeJob) {
        *job.terminal_at.lock().unwrap() =
            Some(SystemTime::now() - TERMINAL_JOB_TTL - Duration::from_secs(1));
    }

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
    fn store_prunes_old_terminal_jobs_but_retains_running_jobs() {
        let store = AnonymizeJobStore::default();
        let running_job = store.create_job(None).unwrap();

        for _ in 0..(MAX_RETAINED_TERMINAL_JOBS + 4) {
            let job = store.create_job(None).unwrap();
            job.finish(Err(AnonymizerError::Canceled));
        }
        let trigger_job = store.create_job(None).unwrap();

        assert_eq!(store.job_count(), MAX_RETAINED_TERMINAL_JOBS + 2);
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
    fn store_prunes_all_terminal_states_and_retains_newest_jobs() {
        let store = AnonymizeJobStore::default();
        let mut terminal_ids = Vec::new();

        for index in 0..(MAX_RETAINED_TERMINAL_JOBS + 4) {
            let job = store.create_job(None).unwrap();
            match index % 3 {
                0 => job.finish(Ok(result_fixture())),
                1 => job.finish(Err(AnonymizerError::Canceled)),
                _ => job.finish(Err(AnonymizerError::SmartReplacement("failed".to_string()))),
            }
            terminal_ids.push(job.snapshot().unwrap().job_id);
        }
        let trigger_job = store.create_job(None).unwrap();

        assert_eq!(store.job_count(), MAX_RETAINED_TERMINAL_JOBS + 1);
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
    fn store_protects_requested_terminal_job_during_prune() {
        let store = AnonymizeJobStore::default();
        for _ in 0..(MAX_RETAINED_TERMINAL_JOBS + 4) {
            let job = store.create_job(None).unwrap();
            job.finish(Err(AnonymizerError::Canceled));
        }
        let protected = store.create_job(None).unwrap();
        protected.finish(Ok(result_fixture()));
        let protected_id = protected.snapshot().unwrap().job_id;

        assert!(store.get_job(&protected_id).is_ok());
        assert_eq!(store.job_count(), MAX_RETAINED_TERMINAL_JOBS + 1);
    }

    #[test]
    fn snapshot_job_removes_terminal_job_after_status_retrieval() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job(None).unwrap();
        let job_id = job.snapshot().unwrap().job_id;
        job.finish(Ok(result_fixture()));

        let status = store.snapshot_job(&job_id).unwrap();

        assert_eq!(status.state, AnonymizeJobState::Succeeded);
        assert!(status.result.is_some());
        assert!(store.get_job(&job_id).is_err());
        assert_eq!(store.job_count(), 0);
    }

    #[test]
    fn store_prunes_terminal_jobs_after_ttl() {
        let store = AnonymizeJobStore::default();
        let old_job = store.create_job(None).unwrap();
        old_job.finish(Err(AnonymizerError::Canceled));
        age_terminal_job(&old_job);
        let old_job_id = old_job.snapshot().unwrap().job_id;

        let active_job = store.create_job(None).unwrap();

        assert!(store.get_job(&old_job_id).is_err());
        assert!(
            store
                .get_job(&active_job.snapshot().unwrap().job_id)
                .is_ok()
        );
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
            test_ledger(&temp_dir),
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
                privacy_config: None,
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
            test_ledger(&temp_dir),
            AnonymizeParams {
                file_path: input_path,
                output_path: output_path.clone(),
                columns: vec![0],
                controls: vec![],
                deterministic: true,
                seed: "standard-seed".to_string(),
                force: false,
                preview_smart_replacements: vec![],
                privacy_config: None,
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
            test_ledger(&temp_dir),
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
                privacy_config: None,
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
            test_ledger(&temp_dir),
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
                privacy_config: None,
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

    fn result_fixture() -> AnonymizeData {
        AnonymizeData {
            output_path: "output.csv".into(),
            row_count: 1,
            columns_anonymized: 1,
            duration_ms: 1,
            privacy_report: PrivacyReport {
                release_mode: ReleaseMode::Standard,
                direct_identifiers: 0,
                quasi_identifiers: 0,
                sensitive_columns: 0,
                pseudonymized_columns: 1,
                smart_replacement_columns: 0,
                opaque_token_columns: 0,
                masked_columns: 0,
                generalized_columns: 0,
                pass_through_columns: 0,
                suppressed_rows: 0,
                synthetic_rows: 0,
                dp_epsilon: None,
                dp_budget: None,
                unique_pseudonym_values: 1,
                reused_pseudonym_values: 0,
                collisions_avoided: 0,
                exhausted_pseudonym_pools: 0,
                opaque_token_values: 0,
                smart_replacement_values: 0,
                smart_replacement_fallbacks: 0,
                formal_models: Vec::new(),
                notes: Vec::new(),
            },
        }
    }
}
