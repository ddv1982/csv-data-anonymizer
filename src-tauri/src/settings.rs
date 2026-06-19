use crate::local_ai::DEFAULT_OLLAMA_MODEL;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SETTINGS_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    pub deterministic_default: bool,
    pub seed: String,
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
            deterministic_default: false,
            seed: String::new(),
            overwrite_output: false,
            sample_row_count: 100,
            preview_sample_count: 5,
            default_output_suffix: "_anonymized".to_string(),
            remember_last_paths: true,
            last_input_directory: None,
            last_output_directory: None,
            local_ai_enabled: false,
            local_ai_model: default_local_ai_model(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new(default_settings_path())
    }
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn load(&self) -> io::Result<AppSettings> {
        load_settings_from_path(&self.path)
    }

    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
        save_settings_to_path(&self.path, settings)
    }
}

fn load_settings_from_path(path: &Path) -> io::Result<AppSettings> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let content = fs::read_to_string(path)?;
    let mut settings = serde_json::from_str::<AppSettings>(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        settings.schema_version = SETTINGS_SCHEMA_VERSION;
    }

    sanitize_settings(&mut settings);
    Ok(settings)
}

fn save_settings_to_path(path: &Path, settings: &AppSettings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut settings = settings.clone();
    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    sanitize_settings(&mut settings);

    let content = serde_json::to_string_pretty(&settings)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let temporary_path = path.with_extension("json.tmp");
    fs::write(&temporary_path, content)?;
    fs::rename(temporary_path, path)
}

fn default_settings_path() -> PathBuf {
    ProjectDirs::from("io.github.ddv1982", "CSV Anonymizer", "CSV Anonymizer")
        .map(|dirs| dirs.config_dir().join("settings.json"))
        .unwrap_or_else(|| PathBuf::from(".csv-anonymizer-settings.json"))
}

pub fn sanitize_settings(settings: &mut AppSettings) {
    settings.sample_row_count = settings.sample_row_count.clamp(1, 10_000);
    settings.preview_sample_count = settings.preview_sample_count.clamp(1, 100);
    if settings.default_output_suffix.trim().is_empty() {
        settings.default_output_suffix = "_anonymized".to_string();
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
