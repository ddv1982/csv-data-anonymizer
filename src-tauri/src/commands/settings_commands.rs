use crate::settings::{AppSettings, SettingsStore, sanitize_settings};

#[tauri::command]
pub fn load_settings() -> Result<AppSettings, String> {
    SettingsStore::default()
        .load()
        .map_err(|error| format!("Could not load settings: {error}"))
}

#[tauri::command]
pub fn save_settings(mut settings: AppSettings) -> Result<(), String> {
    sanitize_settings(&mut settings);
    SettingsStore::default()
        .save(&settings)
        .map_err(|error| format!("Could not save settings: {error}"))
}
