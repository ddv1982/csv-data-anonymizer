use super::model::{
    AppSettings, DEFAULT_OUTPUT_SUFFIX, LEGACY_OUTPUT_SUFFIX, SETTINGS_SCHEMA_VERSION,
    sanitize_settings,
};
use directories::ProjectDirs;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY_SETTINGS_FILE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct SettingsStore {
    pub(super) path: PathBuf,
    save_lock: Mutex<()>,
}

impl Default for SettingsStore {
    fn default() -> Self {
        Self::new(default_settings_path())
    }
}

impl SettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            save_lock: Mutex::new(()),
        }
    }

    pub fn load_settings(&self) -> io::Result<AppSettings> {
        let _guard = self
            .save_lock
            .lock()
            .map_err(|_| io::Error::other("Settings persistence lock is unavailable"))?;
        load_settings_from_path(&self.path)
    }

    pub fn save_settings(&self, settings: &AppSettings) -> io::Result<AppSettings> {
        let _guard = self
            .save_lock
            .lock()
            .map_err(|_| io::Error::other("Settings save lock is unavailable"))?;
        let mut session_settings = settings.clone();
        sanitize_settings(&mut session_settings);
        save_settings_to_path(&self.path, &session_settings)?;
        Ok(session_settings)
    }
}

pub(super) fn load_settings_from_path(path: &Path) -> io::Result<AppSettings> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let content = fs::read_to_string(path)?;
    let mut settings = serde_json::from_str::<AppSettings>(&content)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let stored_settings = settings.clone();

    if settings.schema_version != SETTINGS_SCHEMA_VERSION {
        if settings.schema_version < 3 && settings.default_output_suffix == LEGACY_OUTPUT_SUFFIX {
            settings.default_output_suffix = DEFAULT_OUTPUT_SUFFIX.to_string();
        }
        settings.schema_version = SETTINGS_SCHEMA_VERSION;
    }

    sanitize_settings(&mut settings);
    if settings != stored_settings {
        save_settings_to_path(path, &settings)?;
    }
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
    let temporary_sequence = NEXT_TEMPORARY_SETTINGS_FILE.fetch_add(1, Ordering::Relaxed);
    let temporary_path = path.with_extension(format!(
        "json.{}.{temporary_sequence}.tmp",
        std::process::id()
    ));
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
    fn load_settings_defaults_theme_mode_and_drops_legacy_seed_fields() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        fs::write(
            &settings_path,
            r#"{
              "schemaVersion": 5,
              "deterministicDefault": true,
              "rememberSeed": true,
              "seed": "existing-seed",
              "overwriteOutput": false,
              "sampleRowCount": 250,
              "previewSampleCount": 10,
              "defaultOutputSuffix": "_private_output",
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
        assert_eq!(settings.sample_row_count, 250);
    }

    #[test]
    fn save_settings_to_path_omits_legacy_seed_fields() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let settings = AppSettings::default();

        save_settings_to_path(&settings_path, &settings).unwrap();

        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(!saved_content.contains("deterministicDefault"));
        assert!(!saved_content.contains("seed"));
    }

    #[test]
    fn settings_store_save_round_trips_current_settings() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let store = SettingsStore::new(settings_path.clone());
        let settings = AppSettings {
            preview_sample_count: 7,
            ..AppSettings::default()
        };

        let returned = store.save_settings(&settings).unwrap();

        assert_eq!(returned.preview_sample_count, 7);
        let loaded = store.load_settings().unwrap();
        assert_eq!(loaded.preview_sample_count, 7);
    }

    #[test]
    fn disabling_path_memory_clears_persisted_directories() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(temp_dir.path().join("settings.json"));
        let settings = AppSettings {
            remember_last_paths: false,
            last_input_directory: Some(PathBuf::from("/private/input")),
            last_output_directory: Some(PathBuf::from("/private/output")),
            ..AppSettings::default()
        };

        let saved = store.save_settings(&settings).unwrap();
        assert_eq!(saved.last_input_directory, None);
        assert_eq!(saved.last_output_directory, None);
        assert_eq!(store.load_settings().unwrap(), saved);
    }

    #[test]
    fn loading_disabled_path_memory_removes_stale_directories_from_disk() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let mut settings = AppSettings {
            remember_last_paths: false,
            last_input_directory: Some(PathBuf::from("/private/input")),
            last_output_directory: Some(PathBuf::from("/private/output")),
            ..AppSettings::default()
        };
        settings.schema_version = SETTINGS_SCHEMA_VERSION;
        fs::write(&settings_path, serde_json::to_string(&settings).unwrap()).unwrap();
        let store = SettingsStore::new(settings_path.clone());

        let loaded = store.load_settings().unwrap();
        let persisted: AppSettings =
            serde_json::from_str(&fs::read_to_string(settings_path).unwrap()).unwrap();
        assert_eq!(loaded.last_input_directory, None);
        assert_eq!(persisted.last_input_directory, None);
        assert_eq!(persisted.last_output_directory, None);
    }

    #[test]
    fn concurrent_settings_saves_leave_valid_json() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(SettingsStore::new(temp_dir.path().join("settings.json")));
        let threads = (1..=8)
            .map(|preview_sample_count| {
                let store = store.clone();
                std::thread::spawn(move || {
                    store.save_settings(&AppSettings {
                        preview_sample_count,
                        ..AppSettings::default()
                    })
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().unwrap().unwrap();
        }
        assert!((1..=8).contains(&store.load_settings().unwrap().preview_sample_count));
    }
}
