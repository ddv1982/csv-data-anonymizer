use crate::job_registry::{JobLifecycle, JobRegistry, JobRegistryEntry};
use crate::local_ai::{LocalAiRequest, smart_provider_for_request};
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerError, AnonymizerService, DetectionRunSummary,
    ProcessControl, ProcessProgress, SmartReplacementProvider,
};
use serde::Serialize;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::ipc::Channel;

const MAX_RETAINED_TERMINAL_JOBS: usize = 20;
const TERMINAL_JOB_TTL: Duration = Duration::from_secs(30 * 60);
const PROGRESS_PUBLISH_INTERVAL: Duration = Duration::from_millis(100);

/// Refusal shown when the single anonymization slot is taken.
///
/// Names the abandoned case explicitly because the client can lose a job without the
/// backend losing it: after two minutes of unreachable status polls the UI stops
/// tracking the run and returns to idle, while the job keeps running and keeps this
/// lease. "Another job is already running" alone then reads as a bug — nothing is
/// visibly running — and leaves no way forward. Restarting the app is the only exit
/// this layer can honestly offer: cancelling needs a job id the client no longer has.
const ACTIVE_JOB_REFUSAL: &str = "Another anonymization job is already running. \
Only one run is allowed at a time; if this app stopped tracking an earlier run, \
that run still holds the slot until it finishes or the app is restarted.";

/// Refusal shown when admission state itself cannot be read.
const ADMISSION_UNAVAILABLE: &str = "Anonymization job admission is unavailable.";

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
    active_job: Arc<Mutex<bool>>,
}

pub struct AnonymizeJob {
    lifecycle: JobLifecycle<AnonymizeJobStatus>,
    active_job_lease: Mutex<Option<ActiveJobLease>>,
    progress_channel: Mutex<Option<Channel<AnonymizeJobStatus>>>,
    last_progress_publish: Mutex<Instant>,
}

impl std::fmt::Debug for AnonymizeJob {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AnonymizeJob")
            .field("lifecycle", &self.lifecycle)
            .field("active_job_lease", &self.active_job_lease)
            .field("progress_channel", &"[IPC channel]")
            .finish()
    }
}

#[derive(Debug)]
struct ActiveJobLease {
    active_job: Arc<Mutex<bool>>,
}

impl Drop for ActiveJobLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_job.lock() {
            *active = false;
        }
    }
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
            active_job: Arc::new(Mutex::new(false)),
        }
    }
}

impl AnonymizeJobStore {
    #[cfg(test)]
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
            active_job_lease: Mutex::new(None),
            progress_channel: Mutex::new(None),
            last_progress_publish: Mutex::new(Instant::now()),
        })
    }

    /// Admits one anonymization job at a time, whatever it writes.
    ///
    /// Deliberately not keyed by output path. Two runs writing different files would
    /// be safe to interleave, but a per-path lease cannot be made correct: the same
    /// destination can be spelled more than one way, and path identity varies by
    /// platform, so "different output" is not a question this can answer reliably.
    /// One global lease is the answer it can answer.
    /// Reports whether a job could be admitted right now, without reserving the slot.
    ///
    /// Exists so a caller can refuse an already-busy request before it makes the user
    /// do interactive work (picking or confirming an output file) that a later refusal
    /// throws away. Deliberately does not take the lease: acquiring it here would leave
    /// it held — and the app permanently unable to start any run — on every path that
    /// returns before the job exists, including the user simply cancelling that dialog.
    /// So this is advisory only; `create_job_for_output` stays the sole authority, and a
    /// second request that slips through this check is still refused there.
    pub fn admission_available(&self) -> Result<(), String> {
        let active = self
            .active_job
            .lock()
            .map_err(|_| ADMISSION_UNAVAILABLE.to_string())?;
        if *active {
            return Err(ACTIVE_JOB_REFUSAL.to_string());
        }
        Ok(())
    }

    pub fn create_job_for_output(
        &self,
        total_rows: Option<usize>,
    ) -> Result<Arc<AnonymizeJob>, String> {
        let mut active = self
            .active_job
            .lock()
            .map_err(|_| ADMISSION_UNAVAILABLE.to_string())?;
        if *active {
            return Err(ACTIVE_JOB_REFUSAL.to_string());
        }
        *active = true;
        drop(active);

        let active_job = self.active_job.clone();
        let result = self.registry.create_job(|id, sequence| AnonymizeJob {
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
            active_job_lease: Mutex::new(Some(ActiveJobLease { active_job })),
            progress_channel: Mutex::new(None),
            last_progress_publish: Mutex::new(Instant::now()),
        });
        if result.is_err()
            && let Ok(mut active) = self.active_job.lock()
        {
            *active = false;
        }
        result
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
    pub fn attach_progress_channel(&self, channel: Channel<AnonymizeJobStatus>) {
        if let Ok(mut slot) = self.progress_channel.lock() {
            *slot = Some(channel);
        }
        self.publish_status();
    }

    fn publish_status(&self) {
        let Ok(status) = self.snapshot() else { return };
        let Ok(channel) = self.progress_channel.lock() else {
            return;
        };
        if let Some(channel) = channel.as_ref() {
            let _ = channel.send(status);
        }
    }

    pub fn snapshot(&self) -> Result<AnonymizeJobStatus, String> {
        self.lifecycle.snapshot()
    }

    pub fn report_progress(&self, rows_processed: usize) {
        let _ = self.lifecycle.update_status(|status| {
            if status.state == AnonymizeJobState::Running {
                status.rows_processed = rows_processed;
            }
        });
        if self.progress_publish_due() {
            self.publish_status();
        }
    }

    fn progress_publish_due(&self) -> bool {
        let Ok(mut last_publish) = self.last_progress_publish.lock() else {
            return false;
        };
        if last_publish.elapsed() < PROGRESS_PUBLISH_INTERVAL {
            return false;
        }
        *last_publish = Instant::now();
        true
    }

    pub fn request_cancel(&self) -> Result<AnonymizeJobStatus, String> {
        let status = self.lifecycle.request_cancel(|status| {
            if status.state == AnonymizeJobState::Running {
                status.cancel_requested = true;
            }
        })?;
        self.publish_status();
        Ok(status)
    }

    pub fn should_cancel(&self) -> bool {
        self.lifecycle.should_cancel()
    }

    pub fn finish(&self, result: Result<AnonymizeData, AnonymizerError>) {
        // Processing is complete. Reopen admission before publishing a terminal
        // state so a client that observes completion can always start the next job.
        self.release_active_job_lease();
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
        self.publish_status();
    }

    pub(super) fn finish_panic(&self) {
        self.release_active_job_lease();
        let _ = self.lifecycle.update_status(|status| {
            status.state = AnonymizeJobState::Failed;
            status.error = Some("Anonymization job failed unexpectedly.".to_string());
        });
        self.lifecycle.mark_terminal();
        self.publish_status();
    }

    fn release_active_job_lease(&self) {
        if let Ok(mut lease) = self.active_job_lease.lock() {
            lease.take();
        }
    }
}

impl AnonymizeJobState {
    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

pub fn run_anonymize_job(
    job: Arc<AnonymizeJob>,
    input: AnonymizeParams,
    sample_row_count: usize,
    local_ai: Option<LocalAiRequest>,
    local_ai_enabled: bool,
    detection_run_summary: Option<DetectionRunSummary>,
    tokenization_key: Option<csv_anonymizer_core::TokenizationKey>,
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

    let mut result = match smart_provider_for_request(
        local_ai,
        &input.controls,
        &input.columns,
        local_ai_enabled,
    ) {
        Ok(mut provider) => {
            let provider = provider
                .as_mut()
                .map(|provider| provider as &mut dyn SmartReplacementProvider);
            service().anonymize_csv_with_run_secrets(
                input,
                sample_row_count,
                Some(&mut control),
                provider,
                tokenization_key.as_ref(),
            )
        }
        Err(error) => Err(AnonymizerError::SmartReplacement(error)),
    };
    if let (Ok(data), Some(summary)) = (&mut result, detection_run_summary) {
        data.privacy_report.detection_run_summary = Some(summary);
    }
    job.finish(result);
}

pub(crate) fn service() -> AnonymizerService {
    AnonymizerService::new(env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv_anonymizer_core::{
        AnonymizationStrategy, ColumnControl, DataType, PrivacyReport, SmartReplacementEntry,
    };
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
    fn progress_channel_throttles_rows_but_always_receives_terminal_status() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job(Some(10)).unwrap();
        let messages = Arc::new(AtomicUsize::new(0));
        let observed = messages.clone();
        job.attach_progress_channel(Channel::new(move |_| {
            observed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }));

        for row in 1..=3 {
            job.report_progress(row);
        }
        assert_eq!(job.snapshot().unwrap().rows_processed, 3);
        assert_eq!(messages.load(Ordering::SeqCst), 1);
        job.finish(Err(AnonymizerError::Canceled));

        assert_eq!(messages.load(Ordering::SeqCst), 2);
    }

    /// Admission is global: one job at a time, whatever it writes.
    ///
    /// There is no second test for "a different output path is refused too".
    /// `create_job_for_output` takes no path, so nothing at this layer can tell the
    /// two cases apart, and a test that cannot express the distinction only claims
    /// to pin it. The choice itself is documented on `create_job_for_output`; the
    /// path lives one layer up, in `start_anonymize_job`, where it is authorized
    /// after admission has already been decided.
    #[test]
    fn rejects_second_job_until_active_job_finishes() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job_for_output(None).unwrap();

        assert!(
            store
                .create_job_for_output(None)
                .unwrap_err()
                .contains("already running")
        );

        job.finish(Err(AnonymizerError::Canceled));
        assert!(store.create_job_for_output(None).is_ok());
    }

    /// The pre-dialog check must answer the same question as admission without taking the slot.
    ///
    /// Both halves matter: if it reserved the slot, `start_anonymize_job` would leak the
    /// lease whenever output authorization failed or the user dismissed the dialog, and no
    /// run could ever start again; if it did not refuse, the reorder would buy nothing.
    #[test]
    fn admission_check_refuses_while_busy_without_consuming_the_slot() {
        let store = AnonymizeJobStore::default();

        assert!(store.admission_available().is_ok());
        // Checking twice must not have used the slot up.
        assert!(store.admission_available().is_ok());

        let job = store.create_job_for_output(None).unwrap();
        assert!(store.admission_available().is_err());

        job.finish(Err(AnonymizerError::Canceled));
        assert!(store.admission_available().is_ok());
        assert!(store.create_job_for_output(None).is_ok());
    }

    /// The refusal has to explain the abandoned-run case, not just state that one is running.
    ///
    /// A client that gave up polling shows nothing running, so the bare message read as a
    /// bug and offered no way out.
    #[test]
    fn active_job_refusal_names_the_abandoned_run_and_the_way_out() {
        let store = AnonymizeJobStore::default();
        let _job = store.create_job_for_output(None).unwrap();

        let message = store.create_job_for_output(None).unwrap_err();

        assert_eq!(message, store.admission_available().unwrap_err());
        assert!(message.contains("already running"));
        assert!(message.contains("stopped tracking"));
        assert!(message.contains("restarted"));
    }

    /// Repeat cancel requests are safe, which is what lets the UI keep the button live.
    ///
    /// `ProcessingStatus` no longer disables Cancel once `cancelRequested` is set, so the
    /// command can be called again while the worker is still winding down.
    #[test]
    fn repeated_cancel_requests_are_idempotent() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job(None).unwrap();

        let first = job.request_cancel().unwrap();
        let second = job.request_cancel().unwrap();

        assert!(job.should_cancel());
        assert_eq!(first.state, AnonymizeJobState::Running);
        assert_eq!(second.state, AnonymizeJobState::Running);
        assert!(second.cancel_requested);

        job.finish(Err(AnonymizerError::Canceled));
        let after_terminal = job.request_cancel().unwrap();
        assert_eq!(after_terminal.state, AnonymizeJobState::Canceled);
        assert!(after_terminal.cancel_requested);
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
    fn snapshot_job_keeps_terminal_job_readable_for_repeat_polls() {
        let store = AnonymizeJobStore::default();
        let job = store.create_job(None).unwrap();
        let job_id = job.snapshot().unwrap().job_id;
        job.finish(Ok(result_fixture()));

        let status = store.snapshot_job(&job_id).unwrap();
        let repeat_status = store.snapshot_job(&job_id).unwrap();

        assert_eq!(status.state, AnonymizeJobState::Succeeded);
        assert!(status.result.is_some());
        assert_eq!(repeat_status.state, AnonymizeJobState::Succeeded);
        assert!(store.get_job(&job_id).is_ok());
        assert_eq!(store.job_count(), 1);
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
        let repeat_status = store.snapshot_job(&job_id).unwrap();
        assert_eq!(repeat_status.state, AnonymizeJobState::Failed);
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
            false,
            None,
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
                force: false,
                preview_smart_replacements: vec![],
            },
            10,
            None,
            false,
            None,
            None,
        );

        let status = job.snapshot().unwrap();
        let output = fs::read_to_string(&output_path).unwrap();

        assert_eq!(status.state, AnonymizeJobState::Succeeded);
        assert!(status.result.is_some());
        assert!(output.contains("[EMAIL]"));
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
                force: false,
                preview_smart_replacements: vec![SmartReplacementEntry {
                    column_index: 0,
                    original: "Alice Smith".to_string(),
                    replacement: "Preview Alice".to_string(),
                }],
            },
            10,
            None,
            false,
            None,
            None,
        );

        let status = job.snapshot().unwrap();
        let output = fs::read_to_string(&output_path).unwrap();

        assert_eq!(status.state, AnonymizeJobState::Succeeded);
        assert!(status.result.is_some());
        assert!(output.contains("[EMAIL]"));
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
                force: false,
                preview_smart_replacements: vec![],
            },
            10,
            None,
            false,
            None,
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
                detection_run_summary: None,
                direct_identifiers: 0,
                quasi_identifiers: 0,
                pseudonymized_columns: 1,
                smart_replacement_columns: 0,
                opaque_token_columns: 0,
                masked_columns: 0,
                labelled_columns: 0,
                redacted_columns: 0,
                pass_through_columns: 0,
                unique_pseudonym_values: 1,
                reused_pseudonym_values: 0,
                collisions_avoided: 0,
                exhausted_pseudonym_pools: 0,
                opaque_token_values: 0,
                keyed_token_values: 0,
                keyed_token_columns: Vec::new(),
                smart_replacement_values: 0,
                smart_replacement_rejections: 0,
                smart_replacement_rejection_reasons: Vec::new(),
                smart_replacement_fallbacks: 0,
                shape_fallback_values: 0,
                column_value_distributions: Vec::new(),
                row_uniqueness: None,
                readiness: Default::default(),
                evidence: Vec::new(),
                column_reports: Vec::new(),
                utility_metrics: Vec::new(),
                notes: Vec::new(),
            },
        }
    }
}
