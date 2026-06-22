use super::model::{
    AppSettings, DEFAULT_OUTPUT_SUFFIX, LEGACY_OUTPUT_SUFFIX, SETTINGS_SCHEMA_VERSION,
    sanitize_settings,
};
use directories::ProjectDirs;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SettingsStore {
    pub(super) path: PathBuf,
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
}

pub(super) fn load_settings_from_path(path: &Path) -> io::Result<AppSettings> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let content = fs::read_to_string(path)?;
    let mut settings = serde_json::from_str::<AppSettings>(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        if settings.schema_version < 3 && settings.default_output_suffix == LEGACY_OUTPUT_SUFFIX {
            settings.default_output_suffix = DEFAULT_OUTPUT_SUFFIX.to_string();
        }
        settings.schema_version = SETTINGS_SCHEMA_VERSION;
    }

    sanitize_settings(&mut settings);
    Ok(settings)
}

pub(super) fn save_settings_to_path(path: &Path, settings: &AppSettings) -> io::Result<()> {
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

pub(super) fn default_settings_path() -> PathBuf {
    ProjectDirs::from("io.github.ddv1982", "CSV Anonymizer", "CSV Anonymizer")
        .map(|dirs| dirs.config_dir().join("settings.json"))
        .unwrap_or_else(|| PathBuf::from(".csv-anonymizer-settings.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::model::ThemeMode;

    #[test]
    fn load_settings_defaults_theme_mode_for_existing_settings() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{
              "schemaVersion": 5,
              "deterministicDefault": true,
              "seed": "existing-seed",
              "overwriteOutput": false,
              "sampleRowCount": 250,
              "previewSampleCount": 10,
              "defaultOutputSuffix": "_private_output",
              "dpBudgetEnabled": true,
              "dpBudgetLimitEpsilon": 10.0,
              "dpBudgetSpentEpsilon": 0.0,
              "dpBudgetAction": "block",
              "dpReleaseHistory": [],
              "rememberLastPaths": true,
              "lastInputDirectory": null,
              "lastOutputDirectory": null,
              "localAiEnabled": false,
              "localAiModel": "gemma3:4b"
            }"#,
        )
        .unwrap();

        let settings = load_settings_from_path(&settings_path).unwrap();

        assert_eq!(settings.schema_version, SETTINGS_SCHEMA_VERSION);
        assert_eq!(settings.theme_mode, ThemeMode::System);
        assert!(settings.deterministic_default);
        assert_eq!(settings.seed, "existing-seed");
    }
}
