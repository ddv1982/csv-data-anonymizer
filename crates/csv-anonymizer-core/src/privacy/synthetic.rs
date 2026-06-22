use super::PrivacyProcessResult;
use super::dataset::{CsvDataset, check_canceled, read_dataset, report_progress, write_atomically};
use super::generalization::generalize_value;
use super::roles::{RolePlan, build_role_plan, validate_common_config};
use crate::detection::is_empty_value;
use crate::error::{AnonymizerError, Result, csv_error};
use crate::hash::{deterministic_number, deterministic_uuid};
use crate::types::{
    ColumnMetadata, ColumnRole, DataType, PrivacyConfig, PrivacyModel, PrivacyModelReport,
    PrivacyReport, ProcessControl, ReleaseMode, SyntheticDataConfig,
};
use rand::seq::SliceRandom;
use std::path::{Path, PathBuf};
use std::time::Instant;

struct SyntheticWritePlan<'a> {
    columns: &'a [ColumnMetadata],
    role_plan: &'a RolePlan,
    row_count: usize,
    deterministic: bool,
    seed: &'a str,
}

pub(super) fn process_synthetic_data(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    deterministic: bool,
    seed: &str,
    mut control: Option<&mut ProcessControl<'_>>,
) -> Result<PrivacyProcessResult> {
    validate_common_config(columns, config)?;
    validate_synthetic_config(&config.synthetic)?;
    let start_time = Instant::now();
    let dataset = read_dataset(input_path, control.as_deref_mut())?;
    let role_plan = build_role_plan(columns, config)?;
    let requested_rows = config
        .synthetic
        .row_count
        .unwrap_or_else(|| dataset.data_row_count());
    let write_plan = SyntheticWritePlan {
        columns,
        role_plan: &role_plan,
        row_count: requested_rows,
        deterministic,
        seed,
    };
    let output_path = write_synthetic_release(output_path, &dataset, write_plan, control)?;
    let formal_models = vec![PrivacyModelReport {
        model: PrivacyModel::SyntheticData,
        satisfied: true,
        actual: format!("{} generated row(s)", requested_rows),
        threshold: format!("{} requested row(s)", requested_rows),
        message: "Generated rows are sampled independently from column distributions and direct identifiers are replaced."
            .to_string(),
    }];

    Ok(PrivacyProcessResult {
        row_count: requested_rows,
        output_path,
        duration_ms: start_time.elapsed().as_millis(),
        columns_anonymized: columns.iter().filter(|column| column.is_selected).count(),
        privacy_report: PrivacyReport {
            release_mode: ReleaseMode::SyntheticData,
            direct_identifiers: role_plan.role_count(ColumnRole::DirectIdentifier),
            quasi_identifiers: role_plan.role_count(ColumnRole::QuasiIdentifier),
            sensitive_columns: role_plan.role_count(ColumnRole::Sensitive),
            pseudonymized_columns: role_plan.role_count(ColumnRole::DirectIdentifier)
                + role_plan.role_count(ColumnRole::Sensitive),
            smart_replacement_columns: 0,
            opaque_token_columns: 0,
            masked_columns: 0,
            generalized_columns: role_plan.role_count(ColumnRole::QuasiIdentifier),
            pass_through_columns: role_plan.role_count(ColumnRole::Attribute),
            suppressed_rows: 0,
            synthetic_rows: requested_rows,
            dp_epsilon: None,
            dp_budget: None,
            unique_pseudonym_values: 0,
            reused_pseudonym_values: 0,
            collisions_avoided: 0,
            exhausted_pseudonym_pools: 0,
            opaque_token_values: 0,
            smart_replacement_values: 0,
            smart_replacement_fallbacks: 0,
            formal_models,
            notes: synthetic_notes(),
        },
    })
}

fn validate_synthetic_config(config: &SyntheticDataConfig) -> Result<()> {
    if let Some(row_count) = config.row_count
        && row_count > 1_000_000
    {
        return Err(AnonymizerError::Privacy(
            "synthetic data row count is limited to 1,000,000 rows".to_string(),
        ));
    }
    if config.epsilon.is_some() {
        return Err(AnonymizerError::Privacy(
            "synthetic DP epsilon is not supported by this generator; clear epsilon until a DP synthetic-data generator is implemented"
                .to_string(),
        ));
    }
    Ok(())
}

fn write_synthetic_release(
    output_path: &Path,
    dataset: &CsvDataset,
    plan: SyntheticWritePlan<'_>,
    control: Option<&mut ProcessControl<'_>>,
) -> Result<PathBuf> {
    let column_values = collect_column_values(dataset, plan.columns.len());
    write_atomically(output_path, control, |writer, control| {
        writer.write_record(&dataset.headers).map_err(csv_error)?;
        for row_index in 0..plan.row_count {
            check_canceled(control)?;
            let row = plan
                .columns
                .iter()
                .map(|column| {
                    let role = plan
                        .role_plan
                        .roles
                        .get(column.index)
                        .copied()
                        .unwrap_or(ColumnRole::Attribute);
                    synthetic_value(
                        column,
                        role,
                        row_index,
                        &column_values[column.index],
                        plan.deterministic,
                        plan.seed,
                    )
                })
                .collect::<Vec<_>>();
            writer.write_record(row).map_err(csv_error)?;
            report_progress(control, row_index + 1);
        }
        Ok(())
    })
}

fn collect_column_values(dataset: &CsvDataset, column_count: usize) -> Vec<Vec<String>> {
    (0..column_count)
        .map(|column_index| {
            dataset
                .rows
                .iter()
                .filter(|row| !row.is_blank)
                .filter_map(|row| row.values.get(column_index))
                .filter(|value| !is_empty_value(value))
                .cloned()
                .collect::<Vec<_>>()
        })
        .collect()
}

fn synthetic_value(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    observed_values: &[String],
    deterministic: bool,
    seed: &str,
) -> String {
    if matches!(role, ColumnRole::DirectIdentifier | ColumnRole::Exclude) {
        return synthetic_identifier(column.detected_type, row_index, seed);
    }
    if role == ColumnRole::Sensitive {
        return synthetic_sensitive_value(column.detected_type, row_index, seed);
    }
    if observed_values.is_empty() {
        return String::new();
    }
    let sampled = if deterministic {
        let choice = deterministic_number(
            &format!("{}:{row_index}", column.name),
            seed,
            0,
            observed_values.len().saturating_sub(1) as i64,
        ) as usize;
        observed_values.get(choice)
    } else {
        observed_values.choose(&mut rand::thread_rng())
    };
    let value = sampled.cloned().unwrap_or_default();
    if role == ColumnRole::QuasiIdentifier {
        generalize_value(&value, column.detected_type, 1)
    } else {
        value
    }
}

fn synthetic_identifier(data_type: DataType, row_index: usize, seed: &str) -> String {
    let ordinal = row_index + 1;
    match data_type {
        DataType::Email => format!("person{ordinal}@example.invalid"),
        DataType::Phone => format!("555-010-{:04}", ordinal % 10_000),
        DataType::FirstName => format!("First{ordinal}"),
        DataType::LastName => format!("Last{ordinal}"),
        DataType::FullName => format!("Person {ordinal}"),
        DataType::TaxId => format!("TAX-{:06}", ordinal % 1_000_000),
        DataType::Address => format!("{ordinal} Example Street"),
        DataType::Uuid => deterministic_uuid(&format!("synthetic:{ordinal}"), seed),
        _ => format!("synthetic-{ordinal}"),
    }
}

fn synthetic_sensitive_value(data_type: DataType, row_index: usize, seed: &str) -> String {
    let ordinal = row_index + 1;
    match data_type {
        DataType::NumericId
        | DataType::NumericValue
        | DataType::Currency
        | DataType::Percentage => {
            deterministic_number(&format!("synthetic-sensitive:{ordinal}"), seed, 1, 100)
                .to_string()
        }
        DataType::Boolean => {
            if deterministic_number(&format!("synthetic-sensitive:{ordinal}"), seed, 0, 1) == 0 {
                "false".to_string()
            } else {
                "true".to_string()
            }
        }
        DataType::Timestamp => "2000-01-01T00:00:00Z".to_string(),
        _ => format!("synthetic-sensitive-{ordinal}"),
    }
}

fn synthetic_notes() -> Vec<String> {
    vec![
        "Synthetic data mode generates new rows from simple per-column distributions and does not make the source data anonymous by itself."
            .to_string(),
        "Direct identifier columns are replaced with generated placeholders instead of sampled source values."
            .to_string(),
        "Sensitive columns are replaced with generated placeholders; Attribute columns may still sample observed source values."
            .to_string(),
    ]
}
