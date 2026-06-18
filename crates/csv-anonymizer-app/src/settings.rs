use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SETTINGS_SCHEMA_VERSION: u32 = 1;

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

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> io::Result<AppSettings> {
        load_settings_from_path(&self.path)
    }

    pub fn save(&self, settings: &AppSettings) -> io::Result<()> {
        save_settings_to_path(&self.path, settings)
    }
}

pub fn load_settings_from_path(path: &Path) -> io::Result<AppSettings> {
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

pub fn save_settings_to_path(path: &Path, settings: &AppSettings) -> io::Result<()> {
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

fn sanitize_settings(settings: &mut AppSettings) {
    settings.sample_row_count = settings.sample_row_count.clamp(1, 10_000);
    settings.preview_sample_count = settings.preview_sample_count.clamp(1, 100);
    if settings.default_output_suffix.trim().is_empty() {
        settings.default_output_suffix = "_anonymized".to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_settings() {
        let temporary_dir = tempfile::tempdir().unwrap();
        let path = temporary_dir.path().join("settings.json");
        let settings = AppSettings {
            deterministic_default: true,
            seed: "stable-seed".to_string(),
            overwrite_output: true,
            sample_row_count: 25,
            preview_sample_count: 7,
            default_output_suffix: "_private".to_string(),
            remember_last_paths: true,
            last_input_directory: Some(PathBuf::from("/tmp/input")),
            last_output_directory: Some(PathBuf::from("/tmp/output")),
            ..AppSettings::default()
        };

        save_settings_to_path(&path, &settings).unwrap();
        let loaded = load_settings_from_path(&path).unwrap();

        assert_eq!(loaded, settings);
    }

    #[test]
    fn missing_settings_loads_default() {
        let temporary_dir = tempfile::tempdir().unwrap();
        let path = temporary_dir.path().join("missing.json");

        assert_eq!(
            load_settings_from_path(&path).unwrap(),
            AppSettings::default()
        );
    }

    #[test]
    fn invalid_counts_are_clamped() {
        let temporary_dir = tempfile::tempdir().unwrap();
        let path = temporary_dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{
  "schemaVersion": 1,
  "deterministicDefault": false,
  "seed": "",
  "overwriteOutput": false,
  "sampleRowCount": 0,
  "previewSampleCount": 999,
  "defaultOutputSuffix": "",
  "rememberLastPaths": true,
  "lastInputDirectory": null,
  "lastOutputDirectory": null
}"#,
        )
        .unwrap();

        let loaded = load_settings_from_path(&path).unwrap();

        assert_eq!(loaded.sample_row_count, 1);
        assert_eq!(loaded.preview_sample_count, 100);
        assert_eq!(loaded.default_output_suffix, "_anonymized");
    }
}
