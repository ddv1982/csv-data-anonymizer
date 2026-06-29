use super::model::{AppSettings, DpReleaseRecord, sanitize_session_settings, trim_release_history};
#[cfg(not(test))]
use super::seed_vault::KeyringSeedVault;
use super::seed_vault::SeedVault;
use super::store::{
    SettingsStore, default_settings_path, load_settings_from_path_with_seed_vault,
    save_settings_to_path, save_settings_to_path_with_seed_vault,
};
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerError, DpAggregate, DpBudgetConfig, DpBudgetReport,
    PrivacyConfig, ReleaseMode,
};
use std::collections::BTreeSet;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DpBudgetLedger {
    store: SettingsStore,
    seed_vault: Arc<dyn SeedVault>,
    lock: Mutex<()>,
}

#[derive(Debug, Clone, Copy)]
struct DpReleaseMetadata {
    aggregate: DpAggregate,
    grouped: bool,
    public_group_count: usize,
    value_column: Option<usize>,
    privacy_unit_column: Option<usize>,
    max_contributions_per_unit: Option<usize>,
}

impl Default for DpBudgetLedger {
    fn default() -> Self {
        Self::new(default_settings_path())
    }
}

impl DpBudgetLedger {
    pub fn new(path: PathBuf) -> Self {
        Self::with_seed_vault(path, default_seed_vault())
    }

    pub fn with_seed_vault(path: PathBuf, seed_vault: Arc<dyn SeedVault>) -> Self {
        Self {
            store: SettingsStore::new(path),
            seed_vault,
            lock: Mutex::new(()),
        }
    }

    pub fn load_settings(&self) -> io::Result<AppSettings> {
        let _guard = self.lock_settings()?;
        load_settings_from_path_with_seed_vault(&self.store.path, self.seed_vault.as_ref())
    }

    pub fn save_user_settings(&self, settings: &AppSettings) -> io::Result<AppSettings> {
        let _guard = self.lock_settings()?;
        let current =
            load_settings_from_path_with_seed_vault(&self.store.path, self.seed_vault.as_ref())?;
        let mut next = settings.clone();
        next.dp_budget_spent_epsilon = current.dp_budget_spent_epsilon;
        next.dp_release_history = current.dp_release_history;
        sanitize_session_settings(&mut next);
        save_settings_to_path_with_seed_vault(&self.store.path, &next, self.seed_vault.as_ref())
    }

    pub fn reset_dp_budget(&self) -> io::Result<AppSettings> {
        let _guard = self.lock_settings()?;
        let mut settings =
            load_settings_from_path_with_seed_vault(&self.store.path, self.seed_vault.as_ref())?;
        settings.dp_budget_spent_epsilon = 0.0;
        settings.dp_release_history.clear();
        save_settings_to_path(&self.store.path, &settings)?;
        load_settings_from_path_with_seed_vault(&self.store.path, self.seed_vault.as_ref())
    }

    pub fn privacy_config_for_preflight(
        &self,
        mut privacy_config: Option<PrivacyConfig>,
    ) -> io::Result<Option<PrivacyConfig>> {
        if dp_release_metadata(privacy_config.as_ref()).is_none() {
            return Ok(privacy_config);
        }

        let _guard = self.lock_settings()?;
        let settings =
            load_settings_from_path_with_seed_vault(&self.store.path, self.seed_vault.as_ref())?;
        inject_budget_from_settings(&mut privacy_config, &settings);
        Ok(privacy_config)
    }

    pub fn run_with_budget<F>(
        &self,
        mut input: AnonymizeParams,
        execute: F,
    ) -> Result<AnonymizeData, AnonymizerError>
    where
        F: FnOnce(AnonymizeParams) -> Result<AnonymizeData, AnonymizerError>,
    {
        let Some(metadata) = dp_release_metadata(input.privacy_config.as_ref()) else {
            return execute(input);
        };

        let _guard = self.lock_for_privacy()?;
        let settings =
            load_settings_from_path_with_seed_vault(&self.store.path, self.seed_vault.as_ref())
                .map_err(AnonymizerError::Io)?;
        inject_budget_from_settings(&mut input.privacy_config, &settings);
        let result = execute(input)?;

        if settings.dp_budget_enabled
            && result.privacy_report.release_mode == ReleaseMode::DifferentialPrivacyAggregate
            && let Some(report) = result.privacy_report.dp_budget.as_ref()
        {
            self.commit_dp_release(settings, &result, report, metadata)?;
        }

        Ok(result)
    }

    fn commit_dp_release(
        &self,
        mut settings: AppSettings,
        result: &AnonymizeData,
        report: &DpBudgetReport,
        metadata: DpReleaseMetadata,
    ) -> Result<(), AnonymizerError> {
        let spent_after = parse_report_epsilon(&report.spent_epsilon_after)?;
        settings.dp_budget_spent_epsilon = spent_after;
        settings.dp_release_history.push(DpReleaseRecord {
            id: release_id(),
            timestamp_unix_seconds: current_unix_seconds(),
            output_path: settings
                .remember_last_paths
                .then(|| result.output_path.clone()),
            aggregate: metadata.aggregate,
            grouped: metadata.grouped,
            public_group_count: metadata.public_group_count,
            value_column: metadata.value_column,
            privacy_unit_column: metadata.privacy_unit_column,
            max_contributions_per_unit: metadata.max_contributions_per_unit,
            epsilon: report.release_epsilon.clone(),
            spent_epsilon_before: report.spent_epsilon_before.clone(),
            spent_epsilon_after: report.spent_epsilon_after.clone(),
            remaining_epsilon: report.remaining_epsilon.clone(),
            status: report.status,
            action: report.action,
        });
        trim_release_history(&mut settings.dp_release_history);
        save_settings_to_path(&self.store.path, &settings).map_err(AnonymizerError::Io)
    }

    fn lock_settings(&self) -> io::Result<std::sync::MutexGuard<'_, ()>> {
        self.lock
            .lock()
            .map_err(|_| io::Error::other("local DP budget is unavailable"))
    }

    fn lock_for_privacy(&self) -> Result<std::sync::MutexGuard<'_, ()>, AnonymizerError> {
        self.lock
            .lock()
            .map_err(|_| AnonymizerError::Privacy("local DP budget is unavailable".to_string()))
    }
}

#[cfg(not(test))]
fn default_seed_vault() -> Arc<dyn SeedVault> {
    Arc::new(KeyringSeedVault)
}

#[cfg(test)]
fn default_seed_vault() -> Arc<dyn SeedVault> {
    Arc::new(super::seed_vault::tests::MemorySeedVault::default())
}

fn dp_release_metadata(config: Option<&PrivacyConfig>) -> Option<DpReleaseMetadata> {
    let config = config?;
    if config.release_mode != ReleaseMode::DifferentialPrivacyAggregate {
        return None;
    }
    let dp = &config.differential_privacy;
    Some(DpReleaseMetadata {
        aggregate: dp.aggregate,
        grouped: dp.group_by_column.is_some(),
        public_group_count: public_group_count(&dp.public_group_values),
        value_column: dp.value_column,
        privacy_unit_column: dp.privacy_unit_column,
        max_contributions_per_unit: dp.max_contributions_per_unit,
    })
}

fn public_group_count(values: &[String]) -> usize {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .len()
}

fn inject_budget_from_settings(config: &mut Option<PrivacyConfig>, settings: &AppSettings) {
    let Some(config) = config.as_mut() else {
        return;
    };
    if config.release_mode != ReleaseMode::DifferentialPrivacyAggregate {
        return;
    }
    config.differential_privacy.budget = DpBudgetConfig {
        enabled: settings.dp_budget_enabled,
        limit_epsilon: settings.dp_budget_limit_epsilon,
        spent_epsilon: settings.dp_budget_spent_epsilon,
        action: settings.dp_budget_action,
    };
}

fn parse_report_epsilon(value: &str) -> Result<f64, AnonymizerError> {
    value.parse::<f64>().map_err(|_| {
        AnonymizerError::Privacy(format!(
            "DP budget report contained an invalid epsilon value: {value}"
        ))
    })
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn release_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("dp-{}-{nanos}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    use csv_anonymizer_core::{
        AnonymizationStrategy, ColumnControl, DpBudgetAction, DpBudgetStatus, PrivacyReport,
    };
    use std::fs;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn save_user_settings_preserves_backend_owned_budget_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let ledger = DpBudgetLedger::new(settings_path.clone());
        let mut initial = AppSettings {
            dp_budget_spent_epsilon: 2.0,
            ..AppSettings::default()
        };
        initial.dp_release_history.push(sample_release_record());
        save_settings_to_path(&settings_path, &initial).unwrap();

        let stale_frontend_settings = AppSettings {
            dp_budget_limit_epsilon: Some(5.0),
            dp_budget_spent_epsilon: 0.0,
            dp_release_history: Vec::new(),
            ..AppSettings::default()
        };

        let saved = ledger.save_user_settings(&stale_frontend_settings).unwrap();

        assert_eq!(saved.dp_budget_limit_epsilon, Some(5.0));
        assert_eq!(saved.dp_budget_spent_epsilon, 2.0);
        assert_eq!(saved.dp_release_history.len(), 1);
    }

    #[test]
    fn save_user_settings_returns_session_seed_without_persisting_it() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let ledger = DpBudgetLedger::new(settings_path.clone());
        let session_settings = AppSettings {
            deterministic_default: true,
            remember_seed: false,
            seed: "session-only-seed".to_string(),
            ..AppSettings::default()
        };

        let saved = ledger.save_user_settings(&session_settings).unwrap();

        assert_eq!(saved.seed, "session-only-seed");
        let saved_content = fs::read_to_string(&settings_path).unwrap();
        assert!(!saved_content.contains("session-only-seed"));
        let loaded = ledger.load_settings().unwrap();
        assert!(loaded.seed.is_empty());
    }

    #[test]
    fn run_with_budget_injects_persisted_spent_and_commits_success() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let ledger = DpBudgetLedger::new(settings_path.clone());
        save_settings_to_path(
            &settings_path,
            &AppSettings {
                dp_budget_enabled: true,
                dp_budget_limit_epsilon: Some(5.0),
                dp_budget_spent_epsilon: 1.25,
                dp_budget_action: DpBudgetAction::Block,
                ..AppSettings::default()
            },
        )
        .unwrap();

        let result = ledger
            .run_with_budget(dp_input(temp_dir.path().join("out.csv")), |input| {
                let budget = &input
                    .privacy_config
                    .as_ref()
                    .unwrap()
                    .differential_privacy
                    .budget;
                assert_eq!(budget.spent_epsilon, 1.25);
                assert_eq!(budget.limit_epsilon, Some(5.0));
                Ok(dp_result(&input, 1.25, 2.25, budget.action))
            })
            .unwrap();
        let saved = ledger.load_settings().unwrap();

        assert_eq!(
            result.privacy_report.dp_budget.unwrap().spent_epsilon_after,
            "2.25"
        );
        assert_eq!(saved.dp_budget_spent_epsilon, 2.25);
        assert_eq!(saved.dp_release_history.len(), 1);
        assert_eq!(saved.dp_release_history[0].spent_epsilon_before, "1.25");
        assert_eq!(saved.dp_release_history[0].spent_epsilon_after, "2.25");
    }

    #[test]
    fn privacy_config_for_preflight_injects_persisted_budget_state() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let ledger = DpBudgetLedger::new(settings_path.clone());
        save_settings_to_path(
            &settings_path,
            &AppSettings {
                dp_budget_enabled: true,
                dp_budget_limit_epsilon: Some(1.5),
                dp_budget_spent_epsilon: 1.25,
                dp_budget_action: DpBudgetAction::Block,
                ..AppSettings::default()
            },
        )
        .unwrap();
        let mut stale_config = dp_input(temp_dir.path().join("out.csv"))
            .privacy_config
            .unwrap();
        stale_config.differential_privacy.budget = DpBudgetConfig {
            enabled: true,
            limit_epsilon: Some(10.0),
            spent_epsilon: 0.0,
            action: DpBudgetAction::Warn,
        };

        let prepared = ledger
            .privacy_config_for_preflight(Some(stale_config))
            .unwrap()
            .unwrap();

        assert!(prepared.differential_privacy.budget.enabled);
        assert_eq!(
            prepared.differential_privacy.budget.limit_epsilon,
            Some(1.5)
        );
        assert_eq!(prepared.differential_privacy.budget.spent_epsilon, 1.25);
        assert_eq!(
            prepared.differential_privacy.budget.action,
            DpBudgetAction::Block
        );
    }

    #[test]
    fn run_with_budget_does_not_commit_failed_release() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let ledger = DpBudgetLedger::new(settings_path.clone());
        save_settings_to_path(
            &settings_path,
            &AppSettings {
                dp_budget_enabled: true,
                dp_budget_spent_epsilon: 1.0,
                ..AppSettings::default()
            },
        )
        .unwrap();

        let error = ledger
            .run_with_budget(dp_input(temp_dir.path().join("out.csv")), |_input| {
                Err(AnonymizerError::Privacy("blocked".to_string()))
            })
            .unwrap_err();
        let saved = ledger.load_settings().unwrap();

        assert!(error.to_string().contains("blocked"));
        assert_eq!(saved.dp_budget_spent_epsilon, 1.0);
        assert!(saved.dp_release_history.is_empty());
    }

    #[test]
    fn run_with_budget_serializes_concurrent_spend() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let ledger = Arc::new(DpBudgetLedger::new(settings_path.clone()));
        save_settings_to_path(
            &settings_path,
            &AppSettings {
                dp_budget_enabled: true,
                dp_budget_limit_epsilon: Some(1.5),
                dp_budget_spent_epsilon: 0.0,
                dp_budget_action: DpBudgetAction::Block,
                ..AppSettings::default()
            },
        )
        .unwrap();
        let barrier = Arc::new(Barrier::new(2));

        let handles = (0..2)
            .map(|index| {
                let ledger = ledger.clone();
                let barrier = barrier.clone();
                let output_path = temp_dir.path().join(format!("out-{index}.csv"));
                thread::spawn(move || {
                    barrier.wait();
                    ledger
                        .run_with_budget(dp_input(output_path), |input| {
                            let spent = input
                                .privacy_config
                                .as_ref()
                                .unwrap()
                                .differential_privacy
                                .budget
                                .spent_epsilon;
                            if spent + 1.0 > 1.5 {
                                return Err(AnonymizerError::Privacy(
                                    "DP budget would be exceeded".to_string(),
                                ));
                            }
                            Ok(dp_result(&input, spent, spent + 1.0, DpBudgetAction::Block))
                        })
                        .is_ok()
                })
            })
            .collect::<Vec<_>>();

        let successes = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|success| *success)
            .count();
        let saved = ledger.load_settings().unwrap();

        assert_eq!(successes, 1);
        assert_eq!(saved.dp_budget_spent_epsilon, 1.0);
        assert_eq!(saved.dp_release_history.len(), 1);
    }

    #[test]
    fn reset_dp_budget_clears_spent_and_history() {
        let temp_dir = tempfile::tempdir().unwrap();
        let settings_path = temp_dir.path().join("settings.json");
        let ledger = DpBudgetLedger::new(settings_path.clone());
        let mut settings = AppSettings {
            dp_budget_spent_epsilon: 4.0,
            ..AppSettings::default()
        };
        settings.dp_release_history.push(sample_release_record());
        save_settings_to_path(&settings_path, &settings).unwrap();

        let reset = ledger.reset_dp_budget().unwrap();

        assert_eq!(reset.dp_budget_spent_epsilon, 0.0);
        assert!(reset.dp_release_history.is_empty());
    }

    fn dp_input(output_path: PathBuf) -> AnonymizeParams {
        AnonymizeParams {
            file_path: PathBuf::from("input.csv"),
            output_path,
            columns: vec![0],
            controls: vec![ColumnControl {
                column_index: 0,
                type_override: None,
                strategy: AnonymizationStrategy::PassThrough,
            }],
            deterministic: false,
            seed: String::new(),
            force: false,
            preview_smart_replacements: Vec::new(),
            privacy_config: Some(PrivacyConfig {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                differential_privacy: csv_anonymizer_core::DifferentialPrivacyConfig {
                    epsilon: 1.0,
                    aggregate: DpAggregate::Count,
                    budget: DpBudgetConfig {
                        enabled: true,
                        limit_epsilon: Some(10.0),
                        spent_epsilon: 0.0,
                        action: DpBudgetAction::Block,
                    },
                    ..csv_anonymizer_core::DifferentialPrivacyConfig::default()
                },
                ..PrivacyConfig::default()
            }),
        }
    }

    fn dp_result(
        input: &AnonymizeParams,
        spent_before: f64,
        spent_after: f64,
        action: DpBudgetAction,
    ) -> AnonymizeData {
        AnonymizeData {
            output_path: input.output_path.clone(),
            row_count: 1,
            columns_anonymized: 0,
            duration_ms: 1,
            privacy_report: PrivacyReport {
                release_mode: ReleaseMode::DifferentialPrivacyAggregate,
                direct_identifiers: 0,
                quasi_identifiers: 0,
                sensitive_columns: 0,
                pseudonymized_columns: 0,
                smart_replacement_columns: 0,
                opaque_token_columns: 0,
                masked_columns: 0,
                redacted_columns: 0,
                generalized_columns: 0,
                pass_through_columns: 0,
                suppressed_rows: 0,
                synthetic_rows: 0,
                dp_epsilon: Some("1".to_string()),
                dp_budget: Some(DpBudgetReport {
                    limit_epsilon: "10".to_string(),
                    spent_epsilon_before: format_epsilon(spent_before),
                    release_epsilon: "1".to_string(),
                    spent_epsilon_after: format_epsilon(spent_after),
                    remaining_epsilon: format_epsilon(10.0 - spent_after),
                    status: DpBudgetStatus::WithinBudget,
                    action,
                }),
                unique_pseudonym_values: 0,
                reused_pseudonym_values: 0,
                collisions_avoided: 0,
                exhausted_pseudonym_pools: 0,
                opaque_token_values: 0,
                smart_replacement_values: 0,
                smart_replacement_rejections: 0,
                smart_replacement_rejection_reasons: Vec::new(),
                smart_replacement_fallbacks: 0,
                formal_models: Vec::new(),
                readiness: Default::default(),
                evidence: Vec::new(),
                column_reports: Vec::new(),
                utility_metrics: Vec::new(),
                notes: Vec::new(),
            },
        }
    }

    fn sample_release_record() -> DpReleaseRecord {
        DpReleaseRecord {
            id: "dp-test".to_string(),
            timestamp_unix_seconds: 1,
            output_path: None,
            aggregate: DpAggregate::Count,
            grouped: false,
            public_group_count: 0,
            value_column: None,
            privacy_unit_column: None,
            max_contributions_per_unit: None,
            epsilon: "1".to_string(),
            spent_epsilon_before: "1".to_string(),
            spent_epsilon_after: "2".to_string(),
            remaining_epsilon: "8".to_string(),
            status: DpBudgetStatus::WithinBudget,
            action: DpBudgetAction::Block,
        }
    }

    fn format_epsilon(value: f64) -> String {
        if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            format!("{value:.3}")
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        }
    }
}
