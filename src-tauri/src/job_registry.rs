use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub(crate) trait JobRegistryEntry: std::fmt::Debug + Send + Sync + 'static {
    type Status: Clone + Send + 'static;

    fn lifecycle(&self) -> &JobLifecycle<Self::Status>;
    fn status_is_terminal(status: &Self::Status) -> bool;

    fn created_sequence(&self) -> u64 {
        self.lifecycle().created_sequence()
    }

    fn snapshot(&self) -> Result<Self::Status, String> {
        self.lifecycle().snapshot()
    }

    fn terminal_at(&self) -> Option<SystemTime> {
        self.lifecycle().terminal_at()
    }
}

#[derive(Debug)]
pub(crate) struct JobRegistry<J: JobRegistryEntry> {
    next_id: AtomicU64,
    jobs: Mutex<HashMap<String, Arc<J>>>,
    id_prefix: &'static str,
    store_unavailable_message: &'static str,
    unknown_job_label: &'static str,
    max_retained_terminal_jobs: usize,
    terminal_ttl: Duration,
}

#[derive(Debug)]
pub(crate) struct JobLifecycle<S> {
    created_sequence: u64,
    cancel_requested: AtomicBool,
    status: Mutex<S>,
    terminal_at: Mutex<Option<SystemTime>>,
    status_unavailable_message: &'static str,
}

impl<J: JobRegistryEntry> JobRegistry<J> {
    pub(crate) fn new(
        id_prefix: &'static str,
        store_unavailable_message: &'static str,
        unknown_job_label: &'static str,
        max_retained_terminal_jobs: usize,
        terminal_ttl: Duration,
    ) -> Self {
        Self {
            next_id: AtomicU64::new(0),
            jobs: Mutex::new(HashMap::new()),
            id_prefix,
            store_unavailable_message,
            unknown_job_label,
            max_retained_terminal_jobs,
            terminal_ttl,
        }
    }

    pub(crate) fn create_job(
        &self,
        build: impl FnOnce(String, u64) -> J,
    ) -> Result<Arc<J>, String> {
        let sequence = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let id = format!("{}-{}-{sequence}", self.id_prefix, std::process::id());
        let job = Arc::new(build(id.clone(), sequence));

        let mut jobs = self.lock_jobs()?;
        jobs.insert(id, job.clone());
        self.prune_terminal_jobs(&mut jobs, None);
        Ok(job)
    }

    pub(crate) fn snapshot_job(&self, job_id: &str) -> Result<J::Status, String> {
        let mut jobs = self.lock_jobs()?;
        let job = jobs
            .get(job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown {}: {job_id}", self.unknown_job_label))?;
        let status = job.snapshot()?;
        // Terminal jobs stay readable until TTL/capacity pruning removes them,
        // so a dropped poll response cannot turn a finished job into an
        // "unknown job" error on the next poll.
        self.prune_terminal_jobs(&mut jobs, Some(job_id));
        Ok(status)
    }

    pub(crate) fn get_job(&self, job_id: &str) -> Result<Arc<J>, String> {
        let mut jobs = self.lock_jobs()?;
        let job = jobs
            .get(job_id)
            .cloned()
            .ok_or_else(|| format!("Unknown {}: {job_id}", self.unknown_job_label))?;
        self.prune_terminal_jobs(&mut jobs, Some(job_id));
        Ok(job)
    }

    fn lock_jobs(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Arc<J>>>, String> {
        self.jobs
            .lock()
            .map_err(|_| self.store_unavailable_message.to_string())
    }

    fn prune_terminal_jobs(
        &self,
        jobs: &mut HashMap<String, Arc<J>>,
        protected_job_id: Option<&str>,
    ) {
        let now = SystemTime::now();
        jobs.retain(|job_id, job| {
            protected_job_id == Some(job_id.as_str())
                || !terminal_job_expired(job.as_ref(), now, self.terminal_ttl)
        });

        let mut terminal_jobs = jobs
            .iter()
            .filter(|(job_id, _)| protected_job_id != Some(job_id.as_str()))
            .filter_map(|(job_id, job)| {
                job.snapshot()
                    .ok()
                    .filter(J::status_is_terminal)
                    .map(|_| (job_id.clone(), job.created_sequence()))
            })
            .collect::<Vec<_>>();
        if terminal_jobs.len() <= self.max_retained_terminal_jobs {
            return;
        }

        terminal_jobs.sort_by_key(|(_, sequence)| *sequence);
        let remove_count = terminal_jobs.len() - self.max_retained_terminal_jobs;
        for (job_id, _) in terminal_jobs.into_iter().take(remove_count) {
            jobs.remove(&job_id);
        }
    }

    #[cfg(test)]
    pub(crate) fn job_count(&self) -> usize {
        self.jobs.lock().map(|jobs| jobs.len()).unwrap_or_default()
    }
}

impl<S: Clone> JobLifecycle<S> {
    pub(crate) fn new(
        created_sequence: u64,
        status: S,
        status_unavailable_message: &'static str,
    ) -> Self {
        Self {
            created_sequence,
            cancel_requested: AtomicBool::new(false),
            status: Mutex::new(status),
            terminal_at: Mutex::new(None),
            status_unavailable_message,
        }
    }

    pub(crate) fn created_sequence(&self) -> u64 {
        self.created_sequence
    }

    pub(crate) fn snapshot(&self) -> Result<S, String> {
        self.status
            .lock()
            .map(|status| status.clone())
            .map_err(|_| self.status_unavailable_message.to_string())
    }

    pub(crate) fn request_cancel(&self, update_status: impl FnOnce(&mut S)) -> Result<S, String> {
        self.cancel_requested.store(true, Ordering::SeqCst);
        self.update_status(update_status)?;
        self.snapshot()
    }

    pub(crate) fn update_status(&self, update: impl FnOnce(&mut S)) -> Result<(), String> {
        let mut status = self
            .status
            .lock()
            .map_err(|_| self.status_unavailable_message.to_string())?;
        update(&mut status);
        Ok(())
    }

    pub(crate) fn should_cancel(&self) -> bool {
        self.cancel_requested.load(Ordering::SeqCst)
    }

    pub(crate) fn mark_terminal(&self) {
        if let Ok(mut terminal_at) = self.terminal_at.lock() {
            *terminal_at = Some(SystemTime::now());
        }
    }

    pub(crate) fn terminal_at(&self) -> Option<SystemTime> {
        self.terminal_at
            .lock()
            .ok()
            .and_then(|terminal_at| *terminal_at)
    }

    #[cfg(test)]
    pub(crate) fn set_terminal_at(&self, time: SystemTime) {
        *self.terminal_at.lock().expect("terminal timestamp") = Some(time);
    }
}

fn terminal_job_expired<J: JobRegistryEntry>(job: &J, now: SystemTime, ttl: Duration) -> bool {
    let Some(terminal_at) = job.terminal_at() else {
        return false;
    };
    match now.duration_since(terminal_at) {
        Ok(age) => age >= ttl,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID_PREFIX: &str = "registry-test";
    const STORE_UNAVAILABLE: &str = "Test job store is unavailable.";
    const STATUS_UNAVAILABLE: &str = "Test job status is unavailable.";
    const UNKNOWN_JOB_LABEL: &str = "test job";
    const LONG_TTL: Duration = Duration::from_secs(30 * 60);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestState {
        Running,
        Finished,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestStatus {
        job_id: String,
        state: TestState,
        note: String,
    }

    #[derive(Debug)]
    struct TestJob {
        lifecycle: JobLifecycle<TestStatus>,
    }

    impl JobRegistryEntry for TestJob {
        type Status = TestStatus;

        fn lifecycle(&self) -> &JobLifecycle<Self::Status> {
            &self.lifecycle
        }

        fn status_is_terminal(status: &Self::Status) -> bool {
            match status.state {
                TestState::Running => false,
                TestState::Finished => true,
            }
        }
    }

    impl TestJob {
        fn job_id(&self) -> String {
            self.lifecycle.snapshot().expect("status").job_id
        }

        fn finish(&self) {
            let _ = self
                .lifecycle
                .update_status(|status| status.state = TestState::Finished);
            self.lifecycle.mark_terminal();
        }

        fn note(&self, note: &str) {
            let _ = self
                .lifecycle
                .update_status(|status| status.note = note.to_string());
        }
    }

    fn registry(max_retained_terminal_jobs: usize, terminal_ttl: Duration) -> JobRegistry<TestJob> {
        JobRegistry::new(
            ID_PREFIX,
            STORE_UNAVAILABLE,
            UNKNOWN_JOB_LABEL,
            max_retained_terminal_jobs,
            terminal_ttl,
        )
    }

    fn create(registry: &JobRegistry<TestJob>) -> Arc<TestJob> {
        registry
            .create_job(|job_id, sequence| TestJob {
                lifecycle: JobLifecycle::new(
                    sequence,
                    TestStatus {
                        job_id,
                        state: TestState::Running,
                        note: String::new(),
                    },
                    STATUS_UNAVAILABLE,
                ),
            })
            .expect("job creation")
    }

    fn age_terminal_job(job: &TestJob, ttl: Duration) {
        job.lifecycle
            .set_terminal_at(SystemTime::now() - ttl - Duration::from_secs(1));
    }

    /// A registered job is found again under the id it was told to report.
    ///
    /// The client only ever holds the id from the first response, so lookup has to
    /// resolve that exact string back to the same job instance rather than to a copy
    /// or to a differently-keyed entry.
    #[test]
    fn registered_job_is_found_again_under_the_id_it_reports() {
        let registry = registry(4, LONG_TTL);
        let job = create(&registry);
        let job_id = job.job_id();

        let looked_up = registry.get_job(&job_id).expect("registered job");

        assert!(Arc::ptr_eq(&looked_up, &job));
        assert_eq!(
            registry.snapshot_job(&job_id).expect("status").state,
            TestState::Running
        );
        assert_eq!(registry.job_count(), 1);
    }

    /// Minted ids carry the registry prefix, which is what keeps sibling registries apart.
    ///
    /// Anonymization jobs and Local AI downloads live in separate registries but share one
    /// id namespace in the client, so the prefix is the only thing distinguishing them.
    #[test]
    fn minted_job_ids_carry_the_registry_prefix() {
        let registry = registry(4, LONG_TTL);

        let job_id = create(&registry).job_id();

        assert!(job_id.starts_with(ID_PREFIX), "unexpected job id {job_id}");
    }

    /// An unknown id is refused by name instead of resolving to some other job.
    ///
    /// A poll or cancel aimed at an id this registry never minted must fail loudly; a
    /// silent success would let the UI report progress for a run that is not there.
    #[test]
    fn unknown_job_id_is_refused_and_named_in_the_error() {
        let registry = registry(4, LONG_TTL);
        let _live_job = create(&registry);

        let get_error = registry.get_job("registry-test-0-999").unwrap_err();
        let snapshot_error = registry.snapshot_job("registry-test-0-999").unwrap_err();

        assert!(get_error.contains(UNKNOWN_JOB_LABEL));
        assert!(get_error.contains("registry-test-0-999"));
        assert_eq!(get_error, snapshot_error);
    }

    /// Two jobs in one registry keep separate ids and separate status.
    ///
    /// Sharing either would let one run's progress, cancel flag or terminal state be
    /// reported for the other.
    #[test]
    fn two_jobs_keep_separate_ids_and_separate_status() {
        let registry = registry(4, LONG_TTL);
        let first = create(&registry);
        let second = create(&registry);

        first.note("first");
        second.finish();

        assert_ne!(first.job_id(), second.job_id());
        assert_eq!(registry.job_count(), 2);
        let first_status = registry.snapshot_job(&first.job_id()).expect("status");
        let second_status = registry.snapshot_job(&second.job_id()).expect("status");
        assert_eq!(first_status.note, "first");
        assert_eq!(first_status.state, TestState::Running);
        assert_eq!(second_status.note, "");
        assert_eq!(second_status.state, TestState::Finished);
    }

    /// Terminal jobs past the retention limit are dropped oldest first.
    ///
    /// Newest-first pruning would evict the run the user is most likely still looking at.
    #[test]
    fn retention_limit_drops_the_oldest_terminal_jobs_first() {
        let registry = registry(2, LONG_TTL);
        let terminal_ids = (0..4)
            .map(|_| {
                let job = create(&registry);
                job.finish();
                job.job_id()
            })
            .collect::<Vec<_>>();

        let trigger = create(&registry);

        assert!(registry.get_job(&terminal_ids[0]).is_err());
        assert!(registry.get_job(&terminal_ids[1]).is_err());
        assert!(registry.get_job(&terminal_ids[2]).is_ok());
        assert!(registry.get_job(&terminal_ids[3]).is_ok());
        assert!(registry.get_job(&trigger.job_id()).is_ok());
    }

    /// Running jobs are never pruned, however full the registry gets.
    ///
    /// Dropping a running job would strand a real anonymization run: the worker keeps
    /// writing while every poll and every cancel attempt reports an unknown job.
    #[test]
    fn running_jobs_survive_pruning_that_clears_terminal_ones() {
        let registry = registry(0, LONG_TTL);
        let running = create(&registry);
        let terminal = create(&registry);
        terminal.finish();

        let _trigger = create(&registry);

        assert!(registry.get_job(&running.job_id()).is_ok());
        assert!(registry.get_job(&terminal.job_id()).is_err());
    }

    /// The job being asked about survives the prune its own lookup triggers.
    ///
    /// Polling is retried, so a finished job has to answer more than once; pruning it
    /// while answering would turn a completed run into an unknown-job error.
    #[test]
    fn requested_terminal_job_survives_the_prune_its_own_lookup_triggers() {
        let registry = registry(0, LONG_TTL);
        let job = create(&registry);
        let job_id = job.job_id();
        job.finish();

        let first = registry.snapshot_job(&job_id).expect("first poll");
        let second = registry.snapshot_job(&job_id).expect("repeat poll");

        assert_eq!(first.state, TestState::Finished);
        assert_eq!(second.state, TestState::Finished);

        // Protection covers only the id being asked for: an unrelated lookup may reclaim it.
        let _other = create(&registry);
        assert!(registry.snapshot_job(&job_id).is_err());
    }

    /// Terminal jobs are reclaimed once their TTL has passed.
    ///
    /// Without expiry a long-lived session accumulates finished jobs — each holding its
    /// full result, including a privacy report — for as long as the app stays open.
    #[test]
    fn terminal_jobs_are_reclaimed_after_their_ttl() {
        let ttl = Duration::from_secs(60);
        let registry = registry(100, ttl);
        let expired = create(&registry);
        expired.finish();
        age_terminal_job(&expired, ttl);
        let expired_id = expired.job_id();
        let running = create(&registry);

        let _trigger = create(&registry);

        assert!(registry.get_job(&expired_id).is_err());
        assert!(registry.get_job(&running.job_id()).is_ok());
    }

    /// A job that never went terminal has no expiry clock at all.
    ///
    /// `terminal_at` staying unset is what keeps a slow run out of TTL pruning, however
    /// long it takes.
    #[test]
    fn job_has_no_terminal_timestamp_until_it_is_marked_terminal() {
        let registry = registry(4, LONG_TTL);
        let job = create(&registry);

        assert!(job.terminal_at().is_none());

        job.finish();

        assert!(job.terminal_at().is_some());
    }

    /// A cancel request is visible to the worker and recorded in the status it returns.
    ///
    /// The worker polls `should_cancel` between batches while the UI reads the returned
    /// status; if either half were missed the run would keep writing output the user
    /// asked it to stop producing.
    #[test]
    fn cancel_request_reaches_the_worker_and_the_returned_status() {
        let registry = registry(4, LONG_TTL);
        let job = create(&registry);

        assert!(!job.lifecycle.should_cancel());

        let status = job
            .lifecycle
            .request_cancel(|status| status.note = "canceling".to_string())
            .expect("cancel");

        assert!(job.lifecycle.should_cancel());
        assert_eq!(status.note, "canceling");
        assert_eq!(
            registry
                .snapshot_job(&job.job_id())
                .expect("status after cancel")
                .note,
            "canceling"
        );
    }

    /// Creation order is recorded per job so pruning can order jobs it never saw created.
    ///
    /// Oldest-first eviction reads this sequence, not map order, which is unordered.
    #[test]
    fn each_job_records_an_increasing_creation_sequence() {
        let registry = registry(4, LONG_TTL);

        let first = create(&registry);
        let second = create(&registry);

        assert!(first.created_sequence() < second.created_sequence());
    }
}
