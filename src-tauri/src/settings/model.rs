use crate::local_ai::DEFAULT_OLLAMA_MODEL;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(super) const SETTINGS_SCHEMA_VERSION: u32 = 10;
pub(super) const DEFAULT_OUTPUT_SUFFIX: &str = "_private_output";
pub(super) const LEGACY_OUTPUT_SUFFIX: &str = "_anonymized";

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    pub overwrite_output: bool,
    pub sample_row_count: usize,
    pub preview_sample_count: usize,
    pub default_output_suffix: String,
    pub remember_last_paths: bool,
    pub last_input_directory: Option<PathBuf>,
    pub last_output_directory: Option<PathBuf>,
    #[serde(default)]
    pub local_ai_enabled: bool,
    #[serde(default = "default_local_ai_model")]
    pub local_ai_model: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_SCHEMA_VERSION,
            theme_mode: ThemeMode::System,
            overwrite_output: false,
            sample_row_count: 100,
            preview_sample_count: 5,
            default_output_suffix: DEFAULT_OUTPUT_SUFFIX.to_string(),
            remember_last_paths: true,
            last_input_directory: None,
            last_output_directory: None,
            local_ai_enabled: false,
            local_ai_model: default_local_ai_model(),
        }
    }
}

pub fn sanitize_settings(settings: &mut AppSettings) {
    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    settings.sample_row_count = settings.sample_row_count.clamp(1, 10_000);
    settings.preview_sample_count = settings.preview_sample_count.clamp(1, 100);
    if settings.default_output_suffix.trim().is_empty() {
        settings.default_output_suffix = DEFAULT_OUTPUT_SUFFIX.to_string();
    }
    if settings.local_ai_model.trim().is_empty() {
        settings.local_ai_model = default_local_ai_model();
    } else {
        settings.local_ai_model = settings.local_ai_model.trim().to_string();
    }
}

fn default_local_ai_model() -> String {
    DEFAULT_OLLAMA_MODEL.to_string()
}
