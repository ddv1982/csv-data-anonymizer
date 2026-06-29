use crate::local_ai::DEFAULT_OLLAMA_MODEL;
use csv_anonymizer_core::{DpAggregate, DpBudgetAction, DpBudgetStatus};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub(super) const SETTINGS_SCHEMA_VERSION: u32 = 8;
pub(super) const DEFAULT_OUTPUT_SUFFIX: &str = "_private_output";
pub(super) const LEGACY_OUTPUT_SUFFIX: &str = "_anonymized";
const DEFAULT_DP_BUDGET_LIMIT_EPSILON: f64 = 10.0;
const MAX_DP_RELEASE_HISTORY: usize = 200;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub schema_version: u32,
    #[serde(default)]
    pub theme_mode: ThemeMode,
    pub deterministic_default: bool,
    #[serde(default)]
    pub remember_seed: bool,
    pub seed: String,
    pub overwrite_output: bool,
    pub sample_row_count: usize,
    pub preview_sample_count: usize,
    pub default_output_suffix: String,
    #[serde(default = "default_dp_budget_enabled")]
    pub dp_budget_enabled: bool,
    #[serde(default = "default_dp_budget_limit_epsilon")]
    pub dp_budget_limit_epsilon: Option<f64>,
    #[serde(default)]
    pub dp_budget_spent_epsilon: f64,
    #[serde(default)]
    pub dp_budget_action: DpBudgetAction,
    #[serde(default)]
    pub dp_release_history: Vec<DpReleaseRecord>,
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
            theme_mode: ThemeMode::System,
            deterministic_default: false,
            remember_seed: false,
            seed: String::new(),
            overwrite_output: false,
            sample_row_count: 100,
            preview_sample_count: 5,
            default_output_suffix: DEFAULT_OUTPUT_SUFFIX.to_string(),
            dp_budget_enabled: default_dp_budget_enabled(),
            dp_budget_limit_epsilon: default_dp_budget_limit_epsilon(),
            dp_budget_spent_epsilon: 0.0,
            dp_budget_action: DpBudgetAction::Block,
            dp_release_history: Vec::new(),
            remember_last_paths: true,
            last_input_directory: None,
            last_output_directory: None,
            local_ai_enabled: false,
            local_ai_model: default_local_ai_model(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DpReleaseRecord {
    pub id: String,
    pub timestamp_unix_seconds: u64,
    pub output_path: Option<PathBuf>,
    pub aggregate: DpAggregate,
    pub grouped: bool,
    pub public_group_count: usize,
    pub value_column: Option<usize>,
    pub privacy_unit_column: Option<usize>,
    pub max_contributions_per_unit: Option<usize>,
    pub epsilon: String,
    pub spent_epsilon_before: String,
    pub spent_epsilon_after: String,
    pub remaining_epsilon: String,
    pub status: DpBudgetStatus,
    pub action: DpBudgetAction,
}

pub fn sanitize_settings(settings: &mut AppSettings) {
    sanitize_settings_with_seed_policy(settings, true);
}

pub fn sanitize_persistent_settings(settings: &mut AppSettings) {
    sanitize_settings_with_seed_policy(settings, true);
    settings.seed.clear();
}

pub fn sanitize_session_settings(settings: &mut AppSettings) {
    sanitize_settings_with_seed_policy(settings, false);
}

fn sanitize_settings_with_seed_policy(settings: &mut AppSettings, clear_unremembered_seed: bool) {
    settings.schema_version = SETTINGS_SCHEMA_VERSION;
    settings.sample_row_count = settings.sample_row_count.clamp(1, 10_000);
    settings.preview_sample_count = settings.preview_sample_count.clamp(1, 100);
    if settings.default_output_suffix.trim().is_empty() {
        settings.default_output_suffix = DEFAULT_OUTPUT_SUFFIX.to_string();
    }
    if clear_unremembered_seed && !settings.remember_seed {
        settings.seed.clear();
    }
    if settings.dp_budget_enabled {
        let valid_limit = settings
            .dp_budget_limit_epsilon
            .is_some_and(|limit| limit.is_finite() && limit > 0.0);
        if !valid_limit {
            settings.dp_budget_limit_epsilon = default_dp_budget_limit_epsilon();
        }
    } else if settings
        .dp_budget_limit_epsilon
        .is_some_and(|limit| !limit.is_finite() || limit <= 0.0)
    {
        settings.dp_budget_limit_epsilon = default_dp_budget_limit_epsilon();
    }
    if !settings.dp_budget_spent_epsilon.is_finite() || settings.dp_budget_spent_epsilon < 0.0 {
        settings.dp_budget_spent_epsilon = 0.0;
    }
    trim_release_history(&mut settings.dp_release_history);
    if settings.local_ai_model.trim().is_empty() {
        settings.local_ai_model = default_local_ai_model();
    } else {
        settings.local_ai_model = settings.local_ai_model.trim().to_string();
    }
}

fn default_local_ai_model() -> String {
    DEFAULT_OLLAMA_MODEL.to_string()
}

fn default_dp_budget_enabled() -> bool {
    true
}

fn default_dp_budget_limit_epsilon() -> Option<f64> {
    Some(DEFAULT_DP_BUDGET_LIMIT_EPSILON)
}

pub(super) fn trim_release_history(history: &mut Vec<DpReleaseRecord>) {
    let excess = history.len().saturating_sub(MAX_DP_RELEASE_HISTORY);
    if excess > 0 {
        history.drain(0..excess);
    }
}
