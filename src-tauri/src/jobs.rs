use crate::job_registry::{JobLifecycle, JobRegistry, JobRegistryEntry};
use crate::local_ai::{LocalAiRequest, smart_provider_for_request};
use crate::settings::DpBudgetLedger;
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerError, AnonymizerService, ProcessControl,
    ProcessProgress, ReleaseMode, SmartReplacementProvider,
};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

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

#[derive(Debug)]
pub struct AnonymizeJobStore {
    registry: JobRegistry<AnonymizeJob>,
}

#[derive(Debug)]
pub struct AnonymizeJob {
    lifecycle: JobLifecycle<AnonymizeJobStatus>,
}

impl Default for AnonymizeJobStore {
    fn default() -> Self {
        Self {
            registry: JobRegistry::new(
                "job",
                "Anonymization job store is unavailable.",
                "anonymization job",
                MAX_RETAINED_TERMINAL_JOBS,
                TERMINAL_JOB_TTL,
            ),
        }
    }
}

impl AnonymizeJobStore {
    pub fn create_job(&self, total_rows: Option<usize>) -> Result<Arc<AnonymizeJob>, String> {
        self.registry.create_job(|id, sequence| AnonymizeJob {
            lifecycle: JobLifecycle::new(
                sequence,
                AnonymizeJobStatus {
                    job_id: id,
                    state: AnonymizeJobState::Running,
                    rows_processed: 0,
                    total_rows,
                    cancel_requested: false,
                    result: None,
                    error: None,
                },
                "Anonymization job status is unavailable.",
            ),
        })
    }

    pub fn snapshot_job(&self, job_id: &str) -> Result<AnonymizeJobStatus, String> {
        self.registry.snapshot_job(job_id)
    }

    pub fn get_job(&self, job_id: &str) -> Result<Arc<AnonymizeJob>, String> {
        self.registry.get_job(job_id)
    }

    #[cfg(test)]
    fn job_count(&self) -> usize {
        self.registry.job_count()
    }
}

impl JobRegistryEntry for AnonymizeJob {
    type Status = AnonymizeJobStatus;

    fn lifecycle(&self) -> &JobLifecycle<Self::Status> {
        &self.lifecycle
    }

    fn status_is_terminal(status: &Self::Status) -> bool {
        status.state.is_terminal()
    }
}

impl AnonymizeJob {
    pub fn snapshot(&self) -> Result<AnonymizeJobStatus, String> {
        self.lifecycle.snapshot()
    }

    pub fn report_progress(&self, rows_processed: usize) {
        let _ = self.lifecycle.update_status(|status| {
            if status.state == AnonymizeJobState::Running {
                status.rows_processed = rows_processed;
            }
        });
    }

    pub fn request_cancel(&self) -> Result<AnonymizeJobStatus, String> {
        self.lifecycle.request_cancel(|status| {
            if status.state == AnonymizeJobState::Running {
                status.cancel_requested = true;
            }
        })
    }

    pub fn should_cancel(&self) -> bool {
        self.lifecycle.should_cancel()
    }

    pub fn finish(&self, result: Result<AnonymizeData, AnonymizerError>) {
        let _ = self.lifecycle.update_status(|status| match result {
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
        });
        self.lifecycle.mark_terminal();
    }

    pub(super) fn finish_panic(&self) {
        let _ = self.lifecycle.update_status(|status| {
            status.state = AnonymizeJobState::Failed;
            status.error = Some("Anonymization job failed unexpectedly.".to_string());
        });
        self.lifecycle.mark_terminal();
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

    let release_mode = input
        .privacy_config
        .as_ref()
        .map(|config| config.release_mode)
        .unwrap_or_default();
    let provider_controls = if release_mode == ReleaseMode::Standard {
        input.controls.as_slice()
    } else {
        &[]
    };
    let result = match smart_provider_for_request(local_ai, provider_controls) {
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
        job.lifecycle.set_terminal_at(
            std::time::SystemTime::now() - TERMINAL_JOB_TTL - Duration::from_secs(1),
        );
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
    fn panic_failure_marks_job_failed_and_terminal() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job(Some(10)).unwrap();
        let job_id = job.snapshot().unwrap().job_id;

        job.finish_panic();

        let status = store.snapshot_job(&job_id).unwrap();
        assert_eq!(status.state, AnonymizeJobState::Failed);
        assert!(status.error.unwrap().contains("unexpectedly"));
        assert!(store.get_job(&job_id).is_err());
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
                redacted_columns: 0,
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
                smart_replacement_rejections: 0,
                smart_replacement_rejection_reasons: Vec::new(),
                smart_replacement_fallbacks: 0,
                formal_models: Vec::new(),
                readiness: Default::default(),
                evidence: Vec::new(),
                column_reports: Vec::new(),
                utility_metrics: Vec::new(),
                notes: Vec::new(),
            },
        }
    }
}
