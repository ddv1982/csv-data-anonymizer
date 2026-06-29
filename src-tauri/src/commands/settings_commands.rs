use crate::settings::{AppSettings, DpBudgetLedger};
use std::sync::Arc;
use tauri::State;

const DP_BUDGET_RESET_CONFIRMATION_PHRASE: &str = "RESET DP BUDGET";

#[tauri::command]
pub fn load_settings(settings: State<'_, Arc<DpBudgetLedger>>) -> Result<AppSettings, String> {
    settings
        .load_settings()
        .map_err(|error| format!("Could not load settings: {error}"))
}

#[tauri::command]
pub fn save_settings(
    ledger: State<'_, Arc<DpBudgetLedger>>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    ledger
        .save_user_settings(&settings)
        .map_err(|error| format!("Could not save settings: {error}"))
}

#[tauri::command]
pub fn reset_dp_budget_ledger(
    ledger: State<'_, Arc<DpBudgetLedger>>,
    confirmation_phrase: String,
) -> Result<AppSettings, String> {
    validate_dp_budget_reset_confirmation(&confirmation_phrase)?;
    ledger
        .reset_dp_budget()
        .map_err(|error| format!("Could not reset local DP budget: {error}"))
}

fn validate_dp_budget_reset_confirmation(confirmation_phrase: &str) -> Result<(), String> {
    if confirmation_phrase.trim() == DP_BUDGET_RESET_CONFIRMATION_PHRASE {
        Ok(())
    } else {
        Err(format!(
            "DP budget reset requires confirmation phrase: {DP_BUDGET_RESET_CONFIRMATION_PHRASE}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_required_dp_budget_reset_confirmation_phrase() {
        assert!(validate_dp_budget_reset_confirmation("RESET DP BUDGET").is_ok());
    }

    #[test]
    fn rejects_missing_dp_budget_reset_confirmation_phrase() {
        let error = validate_dp_budget_reset_confirmation("").unwrap_err();

        assert!(error.contains("RESET DP BUDGET"));
    }

    #[test]
    fn rejects_incorrect_dp_budget_reset_confirmation_phrase() {
        let error = validate_dp_budget_reset_confirmation("reset").unwrap_err();

        assert!(error.contains("DP budget reset requires confirmation phrase"));
    }
}
