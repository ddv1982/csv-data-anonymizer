use super::generalization::MAX_GENERALIZATION_LEVEL;
use crate::error::{AnonymizerError, Result};
use crate::types::{ColumnMetadata, ColumnRole, DataType, PrivacyConfig};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(super) struct RolePlan {
    pub(super) roles: Vec<ColumnRole>,
    pub(super) explicit_levels: HashMap<usize, u8>,
}

impl RolePlan {
    pub(super) fn direct_count(&self) -> usize {
        self.roles
            .iter()
            .filter(|role| matches!(role, ColumnRole::DirectIdentifier))
            .count()
    }

    pub(super) fn quasi_indices(&self) -> Vec<usize> {
        self.roles
            .iter()
            .enumerate()
            .filter_map(|(index, role)| {
                matches!(role, ColumnRole::QuasiIdentifier).then_some(index)
            })
            .collect()
    }

    pub(super) fn sensitive_indices(&self) -> Vec<usize> {
        self.roles
            .iter()
            .enumerate()
            .filter_map(|(index, role)| matches!(role, ColumnRole::Sensitive).then_some(index))
            .collect()
    }

    pub(super) fn role_count(&self, role: ColumnRole) -> usize {
        self.roles
            .iter()
            .filter(|candidate| **candidate == role)
            .count()
    }
}

pub(super) fn validate_common_config(
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
) -> Result<()> {
    for role in &config.column_roles {
        validate_column_index(columns, role.column_index)?;
    }
    Ok(())
}

pub(super) fn validate_column_index(columns: &[ColumnMetadata], index: usize) -> Result<()> {
    if index >= columns.len() {
        return Err(AnonymizerError::ColumnOutOfRange {
            index,
            max_index: columns.len().saturating_sub(1),
        });
    }
    Ok(())
}

pub(super) fn build_role_plan(
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
) -> Result<RolePlan> {
    let mut explicit_roles = HashMap::new();
    let mut explicit_levels = HashMap::new();
    for role in &config.column_roles {
        validate_column_index(columns, role.column_index)?;
        explicit_roles.insert(role.column_index, role.role);
        explicit_levels.insert(
            role.column_index,
            role.generalization_level.min(MAX_GENERALIZATION_LEVEL),
        );
    }

    let roles = columns
        .iter()
        .map(|column| {
            let role = explicit_roles
                .get(&column.index)
                .copied()
                .unwrap_or(ColumnRole::Auto);
            if role == ColumnRole::Auto {
                infer_column_role(column)
            } else {
                role
            }
        })
        .collect();

    Ok(RolePlan {
        roles,
        explicit_levels,
    })
}

fn infer_column_role(column: &ColumnMetadata) -> ColumnRole {
    match column.detected_type {
        DataType::Email
        | DataType::Phone
        | DataType::FullName
        | DataType::FirstName
        | DataType::LastName
        | DataType::TaxId
        | DataType::Address => ColumnRole::DirectIdentifier,
        DataType::Uuid
        | DataType::NumericId
        | DataType::PostalCode
        | DataType::IpAddress
        | DataType::Url
        | DataType::MacAddress
        | DataType::Timestamp
        | DataType::CountryCode => ColumnRole::QuasiIdentifier,
        _ => ColumnRole::Attribute,
    }
}
