pub(crate) mod candidate_detector;
mod download;
mod ollama;
mod prompt;
mod provider;
mod types;

use reqwest::{Client as AsyncClient, blocking::Client as BlockingClient};
use std::time::Duration;

pub use download::{LocalAiDownloadStore, start_download_job};
pub use ollama::{ensure_ollama_runtime_available, local_ai_status};
pub use provider::{
    LOCAL_AI_DISABLED_MESSAGE, selection_requires_local_ai, smart_provider_for_request,
    smart_provider_for_strategy,
};
pub use types::{LocalAiDownloadStatus, LocalAiRequest, LocalAiStatus};

pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "gemma3:4b";
const OLLAMA_DOWNLOAD_URL: &str = "https://ollama.com/download";
const OLLAMA_UNAVAILABLE_MESSAGE: &str =
    "Ollama is not running. Install or start Ollama to use Local AI.";
const OLLAMA_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const OLLAMA_DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(15);
const OLLAMA_CLOUD_MODEL_MESSAGE: &str = "Cloud-backed Ollama models are not allowed because \
CSV values may leave this device. Choose a model that is installed and runs locally.";

pub fn open_setup_url() -> Result<(), String> {
    open::that_detached(OLLAMA_DOWNLOAD_URL)
        .map_err(|error| format!("Could not open Ollama download page: {error}"))
}

fn client() -> Result<BlockingClient, String> {
    BlockingClient::builder()
        .timeout(Duration::from_secs(120))
        .connect_timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| format!("Could not create Local AI client: {error}"))
}

fn download_client() -> Result<AsyncClient, String> {
    AsyncClient::builder()
        .timeout(OLLAMA_DOWNLOAD_TIMEOUT)
        .read_timeout(OLLAMA_DOWNLOAD_READ_TIMEOUT)
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

/// Refuses model names that Ollama documents as cloud-backed.
///
/// This is intentionally only an obvious-name guard, not proof that an independently
/// configured Ollama process is local-only. Keeping the check shared makes status and
/// generation fail closed for the cloud model forms the application can identify.
pub(crate) fn ensure_obviously_local_model(model: &str) -> Result<(), String> {
    let normalized = model.trim().to_ascii_lowercase();
    let tag = normalized.rsplit_once(':').map(|(_, tag)| tag);
    if normalized.ends_with(":cloud") || tag.is_some_and(|tag| tag.ends_with("-cloud")) {
        Err(OLLAMA_CLOUD_MODEL_MESSAGE.to_string())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_documented_cloud_model_name_forms_case_insensitively() {
        for model in [
            "glm-4.7:cloud",
            "gpt-oss:120b-cloud",
            "  GPT-OSS:120B-CLOUD  ",
        ] {
            assert_eq!(
                ensure_obviously_local_model(model).unwrap_err(),
                OLLAMA_CLOUD_MODEL_MESSAGE
            );
        }
    }

    #[test]
    fn accepts_ordinary_local_model_names() {
        for model in ["gemma3:4b", "llama3.2:latest", "local-cloud-detector:v1"] {
            ensure_obviously_local_model(model).unwrap();
        }
    }
}
