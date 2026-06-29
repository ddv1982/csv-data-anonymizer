use super::model::{
    AppSettings, DEFAULT_OUTPUT_SUFFIX, LEGACY_OUTPUT_SUFFIX, SETTINGS_SCHEMA_VERSION,
    sanitize_persistent_settings, sanitize_session_settings, sanitize_settings,
};
use super::seed_vault::SeedVault;
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
        if settings.schema_version < 7 && !settings.seed.trim().is_empty() {
            settings.remember_seed = true;
        }
        settings.schema_version = SETTINGS_SCHEMA_VERSION;
    }

    sanitize_settings(&mut settings);
    Ok(settings)
}

pub(super) fn load_settings_from_path_with_seed_vault(
    path: &Path,
    seed_vault: &dyn SeedVault,
) -> io::Result<AppSettings> {
    let mut settings = load_settings_from_path(path)?;
    if settings.remember_seed {
        if !settings.seed.trim().is_empty() {
            let seed = settings.seed.clone();
            seed_vault.save_seed(&seed)?;
            let mut disk_settings = settings.clone();
            disk_settings.seed.clear();
            save_settings_to_path(path, &disk_settings)?;
            settings.seed = seed;
        } else if let Some(seed) = seed_vault.load_seed()? {
            settings.seed = seed;
        }
    }
    sanitize_session_settings(&mut settings);
    Ok(settings)
}

pub(super) fn save_settings_to_path_with_seed_vault(
    path: &Path,
    settings: &AppSettings,
    seed_vault: &dyn SeedVault,
) -> io::Result<AppSettings> {
    let mut session_settings = settings.clone();
    sanitize_session_settings(&mut session_settings);
    if session_settings.remember_seed {
        if !session_settings.seed.trim().is_empty() {
            seed_vault.save_seed(&session_settings.seed)?;
        } else {
            session_settings.seed.clear();
            session_settings.remember_seed = false;
            seed_vault.delete_seed()?;
        }
    } else {
        seed_vault.delete_seed()?;
    }

    let mut disk_settings = session_settings.clone();
    disk_settings.seed.clear();
    save_settings_to_path(path, &disk_settings)?;
    Ok(session_settings)
}

pub(super) fn save_settings_to_path(path: &Path, settings: &AppSettings) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut settings = settings.clone();
    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    sanitize_persistent_settings(&mut settings);

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
    use crate::settings::seed_vault::tests::MemorySeedVault;

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
        assert!(settings.remember_seed);
        assert_eq!(settings.seed, "existing-seed");
    }

    #[test]
    fn save_settings_clears_seed_when_seed_is_not_remembered() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let settings = AppSettings {
            deterministic_default: true,
            remember_seed: false,
            seed: "session-only-seed".to_string(),
            ..AppSettings::default()
        };

        save_settings_to_path(&settings_path, &settings).unwrap();

        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(!saved_content.contains("session-only-seed"));
        let loaded = load_settings_from_path(&settings_path).unwrap();
        assert!(loaded.seed.is_empty());
        assert!(!loaded.remember_seed);
    }

    #[test]
    fn load_settings_migrates_remembered_seed_to_seed_vault() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let seed_vault = MemorySeedVault::default();
        fs::write(
            &settings_path,
            r#"{
              "schemaVersion": 7,
              "themeMode": "system",
              "deterministicDefault": true,
              "rememberSeed": true,
              "seed": "remembered-seed",
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

        let settings =
            load_settings_from_path_with_seed_vault(&settings_path, &seed_vault).unwrap();

        assert_eq!(settings.seed, "remembered-seed");
        assert_eq!(seed_vault.seed().as_deref(), Some("remembered-seed"));
        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(!saved_content.contains("remembered-seed"));
    }

    #[test]
    fn load_settings_keeps_json_seed_when_seed_vault_migration_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let seed_vault = MemorySeedVault::with_save_failure();
        fs::write(
            &settings_path,
            r#"{
              "schemaVersion": 7,
              "themeMode": "system",
              "deterministicDefault": true,
              "rememberSeed": true,
              "seed": "remembered-seed",
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

        let error =
            load_settings_from_path_with_seed_vault(&settings_path, &seed_vault).unwrap_err();

        assert!(error.to_string().contains("seed vault save failed"));
        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(saved_content.contains("remembered-seed"));
    }

    #[test]
    fn save_settings_writes_remembered_seed_to_seed_vault_not_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let seed_vault = MemorySeedVault::default();
        let settings = AppSettings {
            deterministic_default: true,
            remember_seed: true,
            seed: "remembered-seed".to_string(),
            ..AppSettings::default()
        };

        let returned =
            save_settings_to_path_with_seed_vault(&settings_path, &settings, &seed_vault).unwrap();

        assert_eq!(returned.seed, "remembered-seed");
        assert_eq!(seed_vault.seed().as_deref(), Some("remembered-seed"));
        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(!saved_content.contains("remembered-seed"));
    }

    #[test]
    fn save_settings_deletes_seed_vault_when_seed_is_not_remembered() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let seed_vault = MemorySeedVault::default();
        seed_vault.save_seed("old-remembered-seed").unwrap();
        let settings = AppSettings {
            deterministic_default: true,
            remember_seed: false,
            seed: "session-only-seed".to_string(),
            ..AppSettings::default()
        };

        let returned =
            save_settings_to_path_with_seed_vault(&settings_path, &settings, &seed_vault).unwrap();

        assert_eq!(returned.seed, "session-only-seed");
        assert!(seed_vault.seed().is_none());
        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(!saved_content.contains("session-only-seed"));
        assert!(!saved_content.contains("old-remembered-seed"));
    }

    #[test]
    fn save_settings_reports_unremembered_seed_vault_delete_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let seed_vault = MemorySeedVault::with_delete_failure("old-remembered-seed");
        let settings = AppSettings {
            deterministic_default: true,
            remember_seed: false,
            seed: "session-only-seed".to_string(),
            ..AppSettings::default()
        };

        let error = save_settings_to_path_with_seed_vault(&settings_path, &settings, &seed_vault)
            .unwrap_err();

        assert!(error.to_string().contains("seed vault delete failed"));
        assert_eq!(seed_vault.seed().as_deref(), Some("old-remembered-seed"));
        assert!(!settings_path.exists());
    }

    #[test]
    fn save_settings_clears_remembered_seed_when_seed_is_blank() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let seed_vault = MemorySeedVault::default();
        seed_vault.save_seed("old-remembered-seed").unwrap();
        let settings = AppSettings {
            deterministic_default: true,
            remember_seed: true,
            seed: "   ".to_string(),
            ..AppSettings::default()
        };

        let returned =
            save_settings_to_path_with_seed_vault(&settings_path, &settings, &seed_vault).unwrap();

        assert!(returned.seed.is_empty());
        assert!(!returned.remember_seed);
        assert!(seed_vault.seed().is_none());
        let loaded = load_settings_from_path_with_seed_vault(&settings_path, &seed_vault).unwrap();
        assert!(loaded.seed.is_empty());
        assert!(!loaded.remember_seed);
    }
}
