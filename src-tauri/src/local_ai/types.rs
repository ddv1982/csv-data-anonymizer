use serde::{Deserialize, Serialize};

use super::normalized_model;

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

impl LocalAiDownloadState {
    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Canceled)
    }
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
