mod download;
mod ollama;
mod prompt;
mod provider;
mod types;

use reqwest::blocking::Client;
use std::time::Duration;

pub use download::{LocalAiDownloadStore, start_download_job};
pub use ollama::{ensure_ollama_runtime_available, local_ai_status};
pub use provider::smart_provider_for_request;
pub use types::{LocalAiDownloadState, LocalAiDownloadStatus, LocalAiRequest, LocalAiStatus};

pub const DEFAULT_OLLAMA_ENDPOINT: &str = "http://127.0.0.1:11434";
pub const DEFAULT_OLLAMA_MODEL: &str = "gemma3:4b";
const OLLAMA_DOWNLOAD_URL: &str = "https://ollama.com/download";
const OLLAMA_UNAVAILABLE_MESSAGE: &str =
    "Ollama is not running. Install or start Ollama to use Local AI.";

pub fn open_setup_url() -> Result<(), String> {
    open::that(OLLAMA_DOWNLOAD_URL)
        .map_err(|error| format!("Could not open Ollama download page: {error}"))
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
