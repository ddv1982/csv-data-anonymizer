mod dataset;
mod differential_privacy;
mod formal;
mod generalization;
mod roles;
mod synthetic;

use crate::error::{AnonymizerError, Result};
use crate::types::{
    ColumnMetadata, ColumnRole, PrivacyConfig, PrivacyReport, ProcessControl, ReleaseMode,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PrivacyProcessResult {
    pub row_count: usize,
    pub output_path: PathBuf,
    pub duration_ms: u128,
    pub columns_anonymized: usize,
    pub privacy_report: PrivacyReport,
}

pub fn process_privacy_release(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    deterministic: bool,
    seed: &str,
    control: Option<&mut ProcessControl<'_>>,
) -> Result<PrivacyProcessResult> {
    validate_selected_release_config(columns, config)?;

    match config.release_mode {
        ReleaseMode::Standard => Err(AnonymizerError::Privacy(
            "standard releases use the normal anonymization pipeline".to_string(),
        )),
        ReleaseMode::FormalTabular => {
            formal::process_formal_tabular(input_path, output_path, columns, config, control)
        }
        ReleaseMode::DifferentialPrivacyAggregate => differential_privacy::process_dp_aggregate(
            input_path,
            output_path,
            columns,
            config,
            deterministic,
            seed,
            control,
        ),
        ReleaseMode::SyntheticData => synthetic::process_synthetic_data(
            input_path,
            output_path,
            columns,
            config,
            deterministic,
            seed,
            control,
        ),
    }
}

pub(super) fn constrain_unselected_roles_to_attributes(
    columns: &[ColumnMetadata],
    roles: &mut [ColumnRole],
) {
    for (index, column) in columns.iter().enumerate() {
        if !column.is_selected
            && let Some(role) = roles.get_mut(index)
        {
            *role = ColumnRole::Attribute;
        }
    }
}

fn validate_selected_release_config(
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
) -> Result<()> {
    for role in &config.column_roles {
        validate_selected_column_reference(columns, role.column_index, "column role")?;
    }

    if matches!(
        config.release_mode,
        ReleaseMode::DifferentialPrivacyAggregate
    ) {
        if let Some(index) = config.differential_privacy.group_by_column {
            validate_selected_column_reference(columns, index, "group_by_column")?;
        }
        if let Some(index) = config.differential_privacy.value_column {
            validate_selected_column_reference(columns, index, "value_column")?;
        }
        if let Some(index) = config.differential_privacy.privacy_unit_column {
            validate_selected_column_reference(columns, index, "privacy_unit_column")?;
        }
    }

    if config.release_mode == ReleaseMode::SyntheticData {
        if config.synthetic.epsilon.is_some() {
            return Err(AnonymizerError::Privacy(
                "synthetic DP epsilon is not supported by this generator; clear epsilon until a DP synthetic-data generator is implemented"
                    .to_string(),
            ));
        }
        if columns.iter().any(|column| !column.is_selected) {
            return Err(AnonymizerError::Privacy(
                "synthetic data releases require every column to be selected; unselected source columns would otherwise remain in the output"
                    .to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_selected_column_reference(
    columns: &[ColumnMetadata],
    index: usize,
    context: &str,
) -> Result<()> {
    let Some(column) = columns.get(index) else {
        return Err(AnonymizerError::ColumnOutOfRange {
            index,
            max_index: columns.len().saturating_sub(1),
        });
    };
    if !column.is_selected {
        return Err(AnonymizerError::Privacy(format!(
            "privacy release {context} references unselected column {} ('{}'); select the column or remove it from the release config",
            column.index, column.name
        )));
    }
    Ok(())
}
