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
