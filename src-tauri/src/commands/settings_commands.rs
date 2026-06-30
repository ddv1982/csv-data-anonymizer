use crate::settings::{AppSettings, SettingsStore};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn load_settings(settings: State<'_, Arc<SettingsStore>>) -> Result<AppSettings, String> {
    settings
        .load_settings()
        .map_err(|error| format!("Could not load settings: {error}"))
}

#[tauri::command]
pub fn save_settings(
    store: State<'_, Arc<SettingsStore>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    store
        .save_settings(&settings)
        .map_err(|error| format!("Could not save settings: {error}"))
}
