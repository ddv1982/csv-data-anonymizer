use crate::types::{DifferentialPrivacyConfig, DpAggregate, DpBudgetReport, DpBudgetStatus};

pub(super) fn dp_model_message(budget: Option<&DpBudgetReport>) -> String {
    match budget {
        Some(report) => format!(
            "Released aggregate values include Laplace noise calibrated with epsilon; DP budget status is {} after this release.",
            budget_status_label(report.status)
        ),
        None => "Released aggregate values include Laplace noise calibrated with epsilon; no local release history was provided for this run."
            .to_string(),
    }
}

pub(super) fn dp_notes(
    config: &DifferentialPrivacyConfig,
    budget: Option<&DpBudgetReport>,
) -> Vec<String> {
    let mut notes = vec![
        "Differential privacy aggregate mode releases noisy statistics, not row-level source data."
            .to_string(),
    ];
    if config.group_by_column.is_some() {
        notes.push(
            "Grouped DP aggregate output writes every allowed group value, including groups with no matching source rows."
                .to_string(),
        );
    }
    if let Some(limit) = config.max_contributions_per_unit
        && config.privacy_unit_column.is_some()
    {
        notes.push(format!(
            "Privacy-unit contribution bounding used at most {limit} row contribution(s) per privacy unit."
        ));
    }
    notes.push(
        "DP aggregate row count reports released aggregate rows, not the exact source row count."
            .to_string(),
    );
    if let Some(report) = budget {
        notes.push(format!(
            "Local DP budget: spent {} before this release, spent {} after, limit {}, remaining {}.",
            report.spent_epsilon_before,
            report.spent_epsilon_after,
            report.limit_epsilon,
            report.remaining_epsilon
        ));
        if report.status == DpBudgetStatus::OverBudget {
            notes.push(
                "This release exceeded the configured DP budget limit and was allowed because the budget action is warn."
                    .to_string(),
            );
        }
    } else {
        notes.push(
            "No local release history was provided; repeated releases spend additional privacy budget and need to be tracked if you create DP releases outside this app."
                .to_string(),
        );
    }
    if matches!(config.aggregate, DpAggregate::Mean) {
        notes.push("Mean releases split epsilon between noisy sum and noisy count.".to_string());
    }
    notes
}

fn budget_status_label(status: DpBudgetStatus) -> &'static str {
    match status {
        DpBudgetStatus::WithinBudget => "within budget",
        DpBudgetStatus::AtBudget => "at budget",
        DpBudgetStatus::OverBudget => "over budget",
    }
}
