use crate::settings::{AppSettings, DpBudgetLedger, sanitize_settings};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn load_settings(settings: State<'_, Arc<DpBudgetLedger>>) -> Result<AppSettings, String> {
    settings
        .load_settings()
        .map_err(|error| format!("Could not load settings: {error}"))
}

#[tauri::command]
pub fn save_settings(
    ledger: State<'_, Arc<DpBudgetLedger>>,
    mut settings: AppSettings,
) -> Result<AppSettings, String> {
    sanitize_settings(&mut settings);
    ledger
        .save_user_settings(&settings)
        .map_err(|error| format!("Could not save settings: {error}"))
}

#[tauri::command]
pub fn reset_dp_budget_ledger(
    ledger: State<'_, Arc<DpBudgetLedger>>,
) -> Result<AppSettings, String> {
    ledger
        .reset_dp_budget()
        .map_err(|error| format!("Could not reset local DP budget: {error}"))
}
