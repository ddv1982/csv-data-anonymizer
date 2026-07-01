mod download;
mod ollama;
mod prompt;
mod provider;
mod types;

use reqwest::{Client as AsyncClient, blocking::Client as BlockingClient};
use std::time::Duration;

pub use download::{LocalAiDownloadStore, start_download_job};
pub use ollama::{ensure_ollama_runtime_available, local_ai_status};
pub use provider::{smart_provider_for_request, smart_provider_for_strategy};
pub use types::{LocalAiDownloadState, LocalAiDownloadStatus, LocalAiRequest, LocalAiStatus};

pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "gemma3:4b";
const OLLAMA_DOWNLOAD_URL: &str = "https://ollama.com/download";
const OLLAMA_UNAVAILABLE_MESSAGE: &str =
    "Ollama is not running. Install or start Ollama to use Local AI.";
const OLLAMA_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const OLLAMA_DOWNLOAD_READ_TIMEOUT: Duration = Duration::from_secs(15);

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
