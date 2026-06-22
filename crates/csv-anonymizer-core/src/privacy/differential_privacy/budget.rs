use super::validation::validate_epsilon;
use crate::error::{AnonymizerError, Result};
use crate::types::{DifferentialPrivacyConfig, DpBudgetAction, DpBudgetReport, DpBudgetStatus};

pub(super) fn evaluate_budget(
    config: &DifferentialPrivacyConfig,
) -> Result<Option<DpBudgetReport>> {
    if !config.budget.enabled {
        return Ok(None);
    }
    let Some(limit) = config.budget.limit_epsilon else {
        return Err(AnonymizerError::Privacy(
            "DP budget tracking requires a budget limit epsilon".to_string(),
        ));
    };
    validate_epsilon(limit)?;
    if !config.budget.spent_epsilon.is_finite() || config.budget.spent_epsilon < 0.0 {
        return Err(AnonymizerError::Privacy(
            "DP budget spent epsilon must be a finite value greater than or equal to 0".to_string(),
        ));
    }
    let spent_after = config.budget.spent_epsilon + config.epsilon;
    if !spent_after.is_finite() {
        return Err(AnonymizerError::Privacy(
            "DP budget spent epsilon overflowed".to_string(),
        ));
    }
    let remaining = limit - spent_after;
    let status = if spent_after > limit {
        DpBudgetStatus::OverBudget
    } else if (spent_after - limit).abs() <= f64::EPSILON {
        DpBudgetStatus::AtBudget
    } else {
        DpBudgetStatus::WithinBudget
    };
    if status == DpBudgetStatus::OverBudget && config.budget.action == DpBudgetAction::Block {
        return Err(AnonymizerError::Privacy(format!(
            "DP budget would be exceeded: spent {}, release epsilon {}, limit {}",
            format_epsilon(config.budget.spent_epsilon),
            format_epsilon(config.epsilon),
            format_epsilon(limit)
        )));
    }

    Ok(Some(DpBudgetReport {
        limit_epsilon: format_epsilon(limit),
        spent_epsilon_before: format_epsilon(config.budget.spent_epsilon),
        release_epsilon: format_epsilon(config.epsilon),
        spent_epsilon_after: format_epsilon(spent_after),
        remaining_epsilon: format_epsilon(remaining),
        status,
        action: config.budget.action,
    }))
}

pub(super) fn format_epsilon(epsilon: f64) -> String {
    if epsilon.fract() == 0.0 {
        format!("{epsilon:.0}")
    } else {
        format!("{epsilon:.3}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}
