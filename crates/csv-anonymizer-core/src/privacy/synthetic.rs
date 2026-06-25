use super::PrivacyProcessResult;
use super::dataset::{
    CsvDataset, check_canceled, read_dataset, report_progress, write_atomically, write_record,
};
use super::generalization::generalize_value;
use super::roles::{RolePlan, build_role_plan, validate_common_config};
use crate::error::{AnonymizerError, Result};
use crate::hash::{deterministic_number, deterministic_uuid};
use crate::types::{
    ColumnMetadata, ColumnRole, DataType, PrivacyConfig, PrivacyModel, PrivacyModelReport,
    PrivacyReport, ProcessControl, ReleaseMode, SyntheticDataConfig,
};
use std::path::{Path, PathBuf};
use std::time::Instant;

struct SyntheticWritePlan<'a> {
    columns: &'a [ColumnMetadata],
    role_plan: &'a RolePlan,
    row_count: usize,
    seed: &'a str,
}

pub(super) fn process_synthetic_data(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    _deterministic: bool,
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
        seed,
    };
    let output_path = write_synthetic_release(output_path, &dataset, write_plan, control)?;
    let formal_models = vec![PrivacyModelReport {
        model: PrivacyModel::SyntheticData,
        satisfied: true,
        actual: format!("{requested_rows} generated row(s)"),
        threshold: format!("{requested_rows} requested row(s)"),
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
    write_atomically(output_path, control, |writer, control| {
        write_record(writer, dataset.headers.iter().map(String::as_str))?;
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
                    synthetic_value(column, role, row_index, plan.seed)
                })
                .collect::<Vec<_>>();
            write_record(writer, row.iter().map(String::as_str))?;
            report_progress(control, row_index + 1);
        }
        Ok(())
    })
}

fn synthetic_value(
    column: &ColumnMetadata,
    role: ColumnRole,
    row_index: usize,
    seed: &str,
) -> String {
    if matches!(role, ColumnRole::DirectIdentifier | ColumnRole::Exclude) {
        return synthetic_identifier(column.detected_type, row_index, seed);
    }
    if role == ColumnRole::Sensitive {
        return synthetic_sensitive_value(column.detected_type, row_index, seed);
    }
    if role == ColumnRole::QuasiIdentifier {
        let value = synthetic_attribute_value(column.detected_type, row_index, seed);
        generalize_value(&value, column.detected_type, 1)
    } else {
        synthetic_attribute_value(column.detected_type, row_index, seed)
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

fn synthetic_attribute_value(data_type: DataType, row_index: usize, seed: &str) -> String {
    let ordinal = row_index + 1;
    match data_type {
        DataType::Email => format!("attribute{ordinal}@example.invalid"),
        DataType::Phone => format!("555-020-{:04}", ordinal % 10_000),
        DataType::FirstName => format!("AttrFirst{ordinal}"),
        DataType::LastName => format!("AttrLast{ordinal}"),
        DataType::FullName => format!("Attribute Person {ordinal}"),
        DataType::TaxId => format!("ATTR-{:06}", ordinal % 1_000_000),
        DataType::Address => format!("{ordinal} Attribute Avenue"),
        DataType::Uuid => deterministic_uuid(&format!("synthetic-attribute:{ordinal}"), seed),
        DataType::NumericId | DataType::NumericValue | DataType::Currency => {
            deterministic_number(&format!("synthetic-attribute:{ordinal}"), seed, 1, 10_000)
                .to_string()
        }
        DataType::Percentage => {
            deterministic_number(&format!("synthetic-attribute:{ordinal}"), seed, 0, 100)
                .to_string()
        }
        DataType::Boolean => {
            if deterministic_number(&format!("synthetic-attribute:{ordinal}"), seed, 0, 1) == 0 {
                "false".to_string()
            } else {
                "true".to_string()
            }
        }
        DataType::Timestamp => "2000-01-01T00:00:00Z".to_string(),
        DataType::CountryCode => format!("ZZ{:02}", ordinal % 100),
        DataType::Enum => format!("synthetic-enum-{ordinal}"),
        DataType::PostalCode => format!("000{:02}", ordinal % 100),
        DataType::IpAddress => format!("192.0.2.{}", (ordinal % 254) + 1),
        DataType::Url => format!("https://example.invalid/item/{ordinal}"),
        DataType::MacAddress => format!("02:00:00:00:{:02x}:{:02x}", ordinal / 256, ordinal % 256),
        DataType::String | DataType::Unknown => format!("synthetic-attribute-{ordinal}"),
    }
}

fn synthetic_notes() -> Vec<String> {
    vec![
        "Synthetic data mode generates new rows from simple per-column distributions and does not make the source data anonymous by itself."
            .to_string(),
        "Direct identifier columns are replaced with generated placeholders instead of sampled source values."
            .to_string(),
        "Sensitive and Attribute columns are replaced with generated placeholders instead of sampled source values."
            .to_string(),
    ]
}
