use crate::command_error::CommandError;
use crate::settings::{AppSettings, SettingsStore};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn load_settings(settings: State<'_, Arc<SettingsStore>>) -> Result<AppSettings, CommandError> {
    settings
        .load_settings()
        .map_err(|error| CommandError::from(format!("Could not load settings: {error}")))
}

#[tauri::command]
pub fn save_settings(
    store: State<'_, Arc<SettingsStore>>,
    settings: AppSettings,
) -> Result<AppSettings, CommandError> {
    store
        .save_settings(&settings)
        .map_err(|error| CommandError::from(format!("Could not save settings: {error}")))
}
