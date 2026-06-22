use super::super::roles::validate_column_index;
use crate::error::{AnonymizerError, Result};
use crate::types::{ColumnMetadata, ColumnRole, DifferentialPrivacyConfig, DpAggregate};
use std::collections::BTreeSet;

pub(super) fn validate_dp_config(
    columns: &[ColumnMetadata],
    config: &DifferentialPrivacyConfig,
) -> Result<()> {
    validate_epsilon(config.epsilon)?;
    if let Some(index) = config.group_by_column {
        validate_column_index(columns, index)?;
        if !config.group_labels_public {
            return Err(AnonymizerError::Privacy(
                "DP group labels are written to the aggregate output; mark group labels as public before using a group column"
                    .to_string(),
            ));
        }
        let public_groups = normalized_public_group_values(config)?;
        if public_groups.is_empty() {
            return Err(AnonymizerError::Privacy(
                "grouped DP aggregate releases require allowed group values; enter every group value that may be released"
                    .to_string(),
            ));
        }
    } else if !config.public_group_values.is_empty() {
        return Err(AnonymizerError::Privacy(
            "allowed group values require a DP group column".to_string(),
        ));
    }
    if let Some(index) = config.value_column {
        validate_column_index(columns, index)?;
    }
    if config.aggregate == DpAggregate::Count && config.value_column.is_some() {
        return Err(AnonymizerError::Privacy(
            "count releases do not use a value column; clear the value column before creating count output"
                .to_string(),
        ));
    }
    if matches!(config.aggregate, DpAggregate::Sum | DpAggregate::Mean)
        && config.value_column.is_none()
    {
        return Err(AnonymizerError::Privacy(
            "sum and mean releases require a value column".to_string(),
        ));
    }
    if matches!(config.aggregate, DpAggregate::Sum | DpAggregate::Mean)
        && (config.lower_bound.is_none() || config.upper_bound.is_none())
    {
        return Err(AnonymizerError::Privacy(
            "sum and mean releases require public lower and upper bounds".to_string(),
        ));
    }
    if let (Some(lower), Some(upper)) = (config.lower_bound, config.upper_bound)
        && (!lower.is_finite() || !upper.is_finite() || lower > upper)
    {
        return Err(AnonymizerError::Privacy(
            "differential privacy bounds must be finite and lower <= upper".to_string(),
        ));
    }
    if let Some(index) = config.privacy_unit_column {
        validate_column_index(columns, index)?;
    }
    if let Some(limit) = config.max_contributions_per_unit
        && limit == 0
    {
        return Err(AnonymizerError::Privacy(
            "max contributions per privacy unit must be greater than 0".to_string(),
        ));
    }
    if config.max_contributions_per_unit.is_some() && config.privacy_unit_column.is_none() {
        return Err(AnonymizerError::Privacy(
            "max contributions per privacy unit requires a privacy unit column".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn validate_group_label_policy(
    roles: &[ColumnRole],
    config: &DifferentialPrivacyConfig,
) -> Result<()> {
    let Some(group_index) = config.group_by_column else {
        return Ok(());
    };
    let role = roles
        .get(group_index)
        .copied()
        .unwrap_or(ColumnRole::Attribute);
    if role != ColumnRole::Attribute {
        return Err(AnonymizerError::Privacy(format!(
            "DP group column {group_index} is written as a released group label; set its role to Attribute only when the allowed group values are safe to publish"
        )));
    }
    Ok(())
}

pub(super) fn validate_epsilon(epsilon: f64) -> Result<()> {
    if !epsilon.is_finite() || epsilon <= 0.0 {
        return Err(AnonymizerError::Privacy(
            "epsilon must be a finite value greater than 0".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn normalized_public_group_values(
    config: &DifferentialPrivacyConfig,
) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for value in &config.public_group_values {
        let value = value.trim();
        if value.is_empty() {
            return Err(AnonymizerError::Privacy(
                "allowed group values cannot be empty".to_string(),
            ));
        }
        if seen.insert(value.to_string()) {
            values.push(value.to_string());
        }
    }
    Ok(values)
}
