mod aggregate;
mod budget;
mod report;
mod validation;
mod writer;

use super::dataset::read_dataset;
use super::roles::{build_role_plan, validate_common_config};
use super::{PrivacyProcessResult, constrain_unselected_roles_to_attributes};
use crate::error::{AnonymizerError, Result};
use crate::release_report::{
    ReportContext, build_column_reports, build_evidence, build_readiness, build_utility_metrics,
};
use crate::types::{
    ColumnMetadata, ColumnRole, PrivacyConfig, PrivacyModel, PrivacyModelReport, PrivacyReport,
    ProcessControl, ReleaseMode,
};
use std::path::Path;
use std::time::Instant;

pub(super) fn process_dp_aggregate(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    deterministic: bool,
    seed: &str,
    mut control: Option<&mut ProcessControl<'_>>,
) -> Result<PrivacyProcessResult> {
    if deterministic {
        return Err(AnonymizerError::Privacy(
            "deterministic output is not supported for DP aggregate releases; turn off repeatable replacements before creating DP output"
                .to_string(),
        ));
    }
    validate_common_config(columns, config)?;
    let mut role_plan = build_role_plan(columns, config)?;
    constrain_unselected_roles_to_attributes(columns, &mut role_plan.roles);
    let budget_report =
        validate_dp_release_config(columns, config, deterministic, &role_plan.roles)?;
    let start_time = Instant::now();
    let dataset = read_dataset(input_path, control.as_deref_mut())?;
    let (output_path, released_row_count) = writer::write_dp_aggregate_release(
        output_path,
        &dataset,
        columns,
        &config.differential_privacy,
        seed,
        control,
    )?;
    let epsilon = budget::format_epsilon(config.differential_privacy.epsilon);
    let columns_anonymized = aggregate::dp_input_column_count(&config.differential_privacy);
    let report_context = ReportContext {
        roles: Some(&role_plan.roles),
        dp_budget: budget_report.as_ref(),
        dp_group_count: Some(released_row_count),
        ..ReportContext::default()
    };

    Ok(PrivacyProcessResult {
        row_count: released_row_count,
        output_path,
        duration_ms: start_time.elapsed().as_millis(),
        columns_anonymized,
        privacy_report: PrivacyReport {
            release_mode: ReleaseMode::DifferentialPrivacyAggregate,
            direct_identifiers: role_plan.role_count(ColumnRole::DirectIdentifier),
            quasi_identifiers: role_plan.role_count(ColumnRole::QuasiIdentifier),
            sensitive_columns: role_plan.role_count(ColumnRole::Sensitive),
            pseudonymized_columns: 0,
            smart_replacement_columns: 0,
            opaque_token_columns: 0,
            masked_columns: 0,
            generalized_columns: 0,
            pass_through_columns: 0,
            suppressed_rows: 0,
            synthetic_rows: 0,
            dp_epsilon: Some(epsilon.clone()),
            dp_budget: budget_report.clone(),
            unique_pseudonym_values: 0,
            reused_pseudonym_values: 0,
            collisions_avoided: 0,
            exhausted_pseudonym_pools: 0,
            opaque_token_values: 0,
            smart_replacement_values: 0,
            smart_replacement_rejections: 0,
            smart_replacement_rejection_reasons: Vec::new(),
            smart_replacement_fallbacks: 0,
            formal_models: vec![PrivacyModelReport {
                model: PrivacyModel::DifferentialPrivacy,
                satisfied: true,
                actual: format!("epsilon={epsilon}"),
                threshold: format!("epsilon={epsilon}"),
                message: report::dp_model_message(budget_report.as_ref()),
            }],
            readiness: build_readiness(
                ReleaseMode::DifferentialPrivacyAggregate,
                columns,
                Some(config),
                &report_context,
            ),
            evidence: build_evidence(
                ReleaseMode::DifferentialPrivacyAggregate,
                columns,
                &report_context,
            ),
            column_reports: build_column_reports(
                ReleaseMode::DifferentialPrivacyAggregate,
                columns,
                report_context.roles,
            ),
            utility_metrics: build_utility_metrics(
                ReleaseMode::DifferentialPrivacyAggregate,
                columns,
                &report_context,
            ),
            notes: report::dp_notes(&config.differential_privacy, budget_report.as_ref()),
        },
    })
}

pub(super) fn validate_dp_release_config(
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    deterministic: bool,
    roles: &[ColumnRole],
) -> Result<Option<crate::types::DpBudgetReport>> {
    if deterministic {
        return Err(AnonymizerError::Privacy(
            "deterministic output is not supported for DP aggregate releases; turn off repeatable replacements before creating DP output"
                .to_string(),
        ));
    }
    validation::validate_dp_config(columns, &config.differential_privacy)?;
    validation::validate_group_label_policy(roles, &config.differential_privacy)?;
    budget::evaluate_budget(&config.differential_privacy)
}
