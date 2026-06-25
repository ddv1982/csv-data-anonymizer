use csv_anonymizer_core::{
    AnonymizationStrategy, AnonymizerError, ColumnControl, Result as CoreResult, SmartReplacement,
    SmartReplacementProvider, SmartReplacementRequest,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "gemma3:4b";
const OLLAMA_DOWNLOAD_URL: &str = "https://ollama.com/download";
const OLLAMA_UNAVAILABLE_MESSAGE: &str =
    "Ollama is not running. Install or start Ollama to use Local AI.";
const MAX_RETAINED_DOWNLOAD_JOBS: usize = 10;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAiRequest {
    pub enabled: bool,
    pub model: String,
}

impl LocalAiRequest {
    pub fn model_name(&self) -> String {
        normalized_model(&self.model)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAiStatus {
    pub enabled: bool,
    pub provider: String,
    pub model: String,
    pub available_models: Vec<String>,
    pub endpoint: String,
    pub runtime_available: bool,
    pub model_installed: bool,
    pub ready: bool,
    pub runtime_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalAiDownloadState {
    Running,
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalAiDownloadStatus {
    pub job_id: String,
    pub state: LocalAiDownloadState,
    pub model: String,
    pub status_message: String,
    pub completed_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub cancel_requested: bool,
    pub error: Option<String>,
}

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
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaVersion {
    version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaTags {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaModel {
    name: Option<String>,
    model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaPullProgress {
    status: Option<String>,
    completed: Option<u64>,
    total: Option<u64>,
    error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ReplacementPayload {
    replacements: Vec<ReplacementItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReplacementItem {
    original: String,
    replacement: String,
}

impl LocalAiDownloadStore {
    pub fn create_job(&self, model: String) -> Result<Arc<LocalAiDownloadJob>, String> {
        let sequence = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let id = format!("ai-model-{}-{sequence}", std::process::id());
        let job = Arc::new(LocalAiDownloadJob {
            created_sequence: sequence,
            cancel_requested: AtomicBool::new(false),
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

    fn should_cancel(&self) -> bool {
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

    fn finish_success(&self) {
        if let Ok(mut status) = self.status.lock() {
            status.state = LocalAiDownloadState::Succeeded;
            status.status_message = format!("{} is ready for Local AI.", status.model);
            status.cancel_requested = false;
            status.error = None;
        }
    }

    fn finish_canceled(&self) {
        if let Ok(mut status) = self.status.lock() {
            status.state = LocalAiDownloadState::Canceled;
            status.status_message = "Model download canceled.".to_string();
            status.cancel_requested = true;
            status.error = None;
        }
    }

    fn finish_error(&self, error: String) {
        if let Ok(mut status) = self.status.lock() {
            status.state = LocalAiDownloadState::Failed;
            status.error = Some(error.clone());
            status.status_message = error;
        }
    }
}

fn prune_terminal_download_jobs(
    jobs: &mut HashMap<String, Arc<LocalAiDownloadJob>>,
    protected_job_id: Option<&str>,
    max_retained: usize,
) {
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

impl LocalAiDownloadState {
    fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
}

#[derive(Debug, Clone)]
pub struct OllamaSmartReplacementProvider {
    client: Client,
    endpoint: String,
    model: String,
}

impl OllamaSmartReplacementProvider {
    fn new(model: String) -> CoreResult<Self> {
        Ok(Self {
            client: client().map_err(AnonymizerError::SmartReplacement)?,
            endpoint: DEFAULT_OLLAMA_ENDPOINT.to_string(),
            model,
        })
    }
}

impl SmartReplacementProvider for OllamaSmartReplacementProvider {
    fn generate_replacements(
        &mut self,
        request: SmartReplacementRequest<'_>,
    ) -> CoreResult<Vec<SmartReplacement>> {
        let prompt = smart_replacement_prompt(request);
        let options = if request.deterministic {
            json!({
                "temperature": 0.0,
                "seed": stable_seed(request.seed, request.column.index)
            })
        } else {
            json!({
                "temperature": 0.35
            })
        };
        let body = json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
            "format": replacement_schema(),
            "options": options
        });
        let response = self
            .client
            .post(format!("{}/api/generate", self.endpoint))
            .json(&body)
            .send()
            .map_err(|error| {
                AnonymizerError::SmartReplacement(format!("Local AI request failed: {error}"))
            })?
            .error_for_status()
            .map_err(|error| {
                AnonymizerError::SmartReplacement(format!("Local AI request failed: {error}"))
            })?
            .json::<OllamaGenerateResponse>()
            .map_err(|error| {
                AnonymizerError::SmartReplacement(format!(
                    "Local AI response was not valid: {error}"
                ))
            })?;
        let parsed =
            serde_json::from_str::<ReplacementPayload>(&response.response).map_err(|error| {
                AnonymizerError::SmartReplacement(format!(
                    "Local AI returned replacement data that could not be parsed: {error}"
                ))
            })?;
        Ok(parsed
            .replacements
            .into_iter()
            .map(|item| SmartReplacement {
                original: item.original,
                replacement: item.replacement,
            })
            .collect())
    }
}

pub fn local_ai_status(request: LocalAiRequest) -> Result<LocalAiStatus, String> {
    local_ai_status_with_endpoint(request, DEFAULT_OLLAMA_ENDPOINT)
}

pub fn ensure_ollama_runtime_available() -> Result<(), String> {
    ensure_runtime_available(DEFAULT_OLLAMA_ENDPOINT)
}

fn local_ai_status_with_endpoint(
    request: LocalAiRequest,
    endpoint: &str,
) -> Result<LocalAiStatus, String> {
    let model = request.model_name();
    let client = client()?;
    let version = ollama_version(&client, endpoint);
    let Ok(version) = version else {
        return Ok(LocalAiStatus {
            enabled: request.enabled,
            provider: "ollama".to_string(),
            model,
            available_models: Vec::new(),
            endpoint: endpoint.to_string(),
            runtime_available: false,
            model_installed: false,
            ready: false,
            runtime_version: None,
            message: OLLAMA_UNAVAILABLE_MESSAGE.to_string(),
        });
    };

    let tags = client
        .get(format!("{endpoint}/api/tags"))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<OllamaTags>())
        .map_err(|error| format!("Could not inspect Ollama models: {error}"))?;
    let available_models = installed_model_names(&tags.models);
    let model_installed = is_model_installed(&tags.models, &model);
    let ready = request.enabled && model_installed;
    let message = if !request.enabled {
        "Local AI is off. Enable it before choosing Smart replacement.".to_string()
    } else if model_installed {
        "Local AI is ready. CSV values stay on this device and are sent only to Ollama on localhost."
            .to_string()
    } else {
        format!("{model} is not downloaded in Ollama yet.")
    };

    Ok(LocalAiStatus {
        enabled: request.enabled,
        provider: "ollama".to_string(),
        model,
        available_models,
        endpoint: endpoint.to_string(),
        runtime_available: true,
        model_installed,
        ready,
        runtime_version: version.version,
        message,
    })
}

fn ensure_runtime_available(endpoint: &str) -> Result<(), String> {
    let client = client()?;
    ollama_version(&client, endpoint)
        .map(|_| ())
        .map_err(|_| OLLAMA_UNAVAILABLE_MESSAGE.to_string())
}

fn ollama_version(client: &Client, endpoint: &str) -> Result<OllamaVersion, reqwest::Error> {
    client
        .get(format!("{endpoint}/api/version"))
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.json::<OllamaVersion>())
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

pub fn open_setup_url() -> Result<(), String> {
    open::that(OLLAMA_DOWNLOAD_URL)
        .map_err(|error| format!("Could not open Ollama download page: {error}"))
}

pub fn smart_provider_for_request(
    request: Option<LocalAiRequest>,
    controls: &[ColumnControl],
) -> Result<Option<OllamaSmartReplacementProvider>, String> {
    if !controls
        .iter()
        .any(|control| control.strategy == AnonymizationStrategy::LocalAi)
    {
        return Ok(None);
    }

    let Some(request) = request.filter(|request| request.enabled) else {
        return Ok(None);
    };
    OllamaSmartReplacementProvider::new(request.model_name())
        .map(Some)
        .map_err(|error| error.to_string())
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

fn client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| format!("Could not create Local AI client: {error}"))
}

fn download_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| format!("Could not create Local AI download client: {error}"))
}

fn normalized_model(model: &str) -> String {
    let model = model.trim();
    if model.is_empty() {
        DEFAULT_OLLAMA_MODEL.to_string()
    } else {
        model.to_string()
    }
}

fn installed_model_names(models: &[OllamaModel]) -> Vec<String> {
    let mut names = models
        .iter()
        .filter_map(|installed| {
            [installed.name.as_deref(), installed.model.as_deref()]
                .into_iter()
                .flatten()
                .find(|name| !name.trim().is_empty())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn is_model_installed(models: &[OllamaModel], model: &str) -> bool {
    models.iter().any(|installed| {
        installed.name.as_deref() == Some(model) || installed.model.as_deref() == Some(model)
    })
}

fn smart_replacement_prompt(request: SmartReplacementRequest<'_>) -> String {
    let values = serde_json::to_string(request.values).unwrap_or_else(|_| "[]".to_string());
    format!(
        "Create realistic fake CSV replacement values. Data stays local. Return only JSON matching the schema. Do not copy any original value, do not include personal data, and keep the same broad data type. Column name: {name}. Detected type: {data_type:?}. Values JSON array: {values}",
        name = request.column.name,
        data_type = request.column.detected_type,
    )
}

fn replacement_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "replacements": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "original": { "type": "string" },
                        "replacement": { "type": "string" }
                    },
                    "required": ["original", "replacement"]
                }
            }
        },
        "required": ["replacements"]
    })
}

fn stable_seed(seed: &str, column_index: usize) -> u64 {
    let mut hash = 14_695_981_039_346_656_037_u64;
    for byte in format!("{seed}:{column_index}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash & 0x7fff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn installed_model_names_are_sorted_deduped_and_fallback_to_model() {
        let names = installed_model_names(&[
            OllamaModel {
                name: Some("llama3.2:latest".to_string()),
                model: Some("llama3.2:latest".to_string()),
            },
            OllamaModel {
                name: Some("".to_string()),
                model: Some("gemma3:4b".to_string()),
            },
            OllamaModel {
                name: Some("llama3.2:latest".to_string()),
                model: None,
            },
        ]);

        assert_eq!(names, vec!["gemma3:4b", "llama3.2:latest"]);
    }

    #[test]
    fn is_model_installed_checks_name_and_model_fields() {
        let models = [OllamaModel {
            name: Some("llama3.2".to_string()),
            model: Some("llama3.2:latest".to_string()),
        }];

        assert!(is_model_installed(&models, "llama3.2"));
        assert!(is_model_installed(&models, "llama3.2:latest"));
        assert!(!is_model_installed(&models, "gemma3:4b"));
    }

    #[test]
    fn local_ai_status_reports_friendly_message_when_ollama_is_unavailable() {
        let status = local_ai_status_with_endpoint(
            LocalAiRequest {
                enabled: true,
                model: "gemma3:4b".to_string(),
            },
            &unused_loopback_endpoint(),
        )
        .expect("local ai status should be returned even when ollama is unavailable");

        assert!(!status.runtime_available);
        assert!(!status.model_installed);
        assert!(!status.ready);
        assert_eq!(status.message, OLLAMA_UNAVAILABLE_MESSAGE);
    }

    #[test]
    fn runtime_preflight_returns_friendly_message_when_ollama_is_unavailable() {
        let error = ensure_runtime_available(&unused_loopback_endpoint())
            .expect_err("runtime preflight should fail without ollama");

        assert_eq!(error, OLLAMA_UNAVAILABLE_MESSAGE);
    }

    #[test]
    fn download_store_prunes_old_terminal_jobs_but_keeps_running_jobs() {
        let store = LocalAiDownloadStore::default();
        let running_job = store.create_job("gemma3:4b".to_string()).unwrap();

        for index in 0..(MAX_RETAINED_DOWNLOAD_JOBS + 3) {
            let job = store.create_job(format!("model-{index}")).unwrap();
            job.finish_error("failed".to_string());
        }

        assert!(store.job_count() <= MAX_RETAINED_DOWNLOAD_JOBS + 2);
        assert!(
            store
                .get_job(&running_job.snapshot().unwrap().job_id)
                .is_ok()
        );
    }

    fn unused_loopback_endpoint() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("loopback port should bind");
        let address = listener
            .local_addr()
            .expect("loopback address should be available");
        drop(listener);
        format!("http://{address}")
    }
}
