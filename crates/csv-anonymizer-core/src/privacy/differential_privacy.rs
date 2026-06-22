use super::PrivacyProcessResult;
use super::dataset::{CsvDataset, check_canceled, read_dataset, report_progress, write_atomically};
use super::roles::{build_role_plan, validate_column_index, validate_common_config};
use crate::error::{AnonymizerError, Result, csv_error};
use crate::hash::deterministic_hash;
use crate::types::{
    ColumnMetadata, ColumnRole, DifferentialPrivacyConfig, DpAggregate, PrivacyConfig,
    PrivacyModel, PrivacyModelReport, PrivacyReport, ProcessControl, ReleaseMode,
};
use rand::Rng;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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
    validate_common_config(columns, config)?;
    validate_dp_config(columns, &config.differential_privacy)?;
    let start_time = Instant::now();
    let dataset = read_dataset(input_path, control.as_deref_mut())?;
    let output_path = write_dp_aggregate_release(
        output_path,
        &dataset,
        columns,
        &config.differential_privacy,
        deterministic,
        seed,
        control,
    )?;
    let role_plan = build_role_plan(columns, config)?;
    let epsilon = format_epsilon(config.differential_privacy.epsilon);

    Ok(PrivacyProcessResult {
        row_count: dataset.data_row_count(),
        output_path,
        duration_ms: start_time.elapsed().as_millis(),
        columns_anonymized: config
            .differential_privacy
            .value_column
            .or(config.differential_privacy.group_by_column)
            .map(|_| 1)
            .unwrap_or(0),
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
            unique_pseudonym_values: 0,
            reused_pseudonym_values: 0,
            collisions_avoided: 0,
            exhausted_pseudonym_pools: 0,
            opaque_token_values: 0,
            smart_replacement_values: 0,
            smart_replacement_fallbacks: 0,
            formal_models: vec![PrivacyModelReport {
                model: PrivacyModel::DifferentialPrivacy,
                satisfied: true,
                actual: format!("epsilon={epsilon}"),
                threshold: format!("epsilon={epsilon}"),
                message: "Released aggregate values include Laplace noise with one-query budget accounting."
                    .to_string(),
            }],
            notes: dp_notes(&config.differential_privacy),
        },
    })
}

fn validate_dp_config(
    columns: &[ColumnMetadata],
    config: &DifferentialPrivacyConfig,
) -> Result<()> {
    validate_epsilon(config.epsilon)?;
    if let Some(index) = config.group_by_column {
        validate_column_index(columns, index)?;
    }
    if let Some(index) = config.value_column {
        validate_column_index(columns, index)?;
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

fn write_dp_aggregate_release(
    output_path: &Path,
    dataset: &CsvDataset,
    columns: &[ColumnMetadata],
    config: &DifferentialPrivacyConfig,
    deterministic: bool,
    seed: &str,
    control: Option<&mut ProcessControl<'_>>,
) -> Result<PathBuf> {
    let aggregates = build_aggregates(dataset, config);
    write_atomically(output_path, control, |writer, control| {
        if let Some(group_index) = config.group_by_column {
            let group_name = columns
                .get(group_index)
                .map(|column| column.name.as_str())
                .unwrap_or("group");
            writer
                .write_record([group_name, "aggregate", "noisyValue", "epsilon"])
                .map_err(csv_error)?;
        } else {
            writer
                .write_record(["aggregate", "noisyValue", "epsilon"])
                .map_err(csv_error)?;
        }

        let mut rows_written = 0;
        for (group, aggregate) in aggregates {
            check_canceled(control)?;
            let noisy = noisy_aggregate(&group, &aggregate, config, deterministic, seed);
            if config.group_by_column.is_some() {
                writer
                    .write_record([
                        group.as_str(),
                        aggregate_label(config.aggregate),
                        &format!("{noisy:.6}"),
                        &format_epsilon(config.epsilon),
                    ])
                    .map_err(csv_error)?;
            } else {
                writer
                    .write_record([
                        aggregate_label(config.aggregate),
                        &format!("{noisy:.6}"),
                        &format_epsilon(config.epsilon),
                    ])
                    .map_err(csv_error)?;
            }
            rows_written += 1;
            report_progress(control, rows_written);
        }
        Ok(())
    })
}

#[derive(Debug, Clone, Copy)]
struct AggregateState {
    count: usize,
    sum: f64,
}

fn build_aggregates(
    dataset: &CsvDataset,
    config: &DifferentialPrivacyConfig,
) -> BTreeMap<String, AggregateState> {
    let mut aggregates: BTreeMap<String, AggregateState> = BTreeMap::new();
    for row in dataset.rows.iter().filter(|row| !row.is_blank) {
        let group = config
            .group_by_column
            .and_then(|index| row.values.get(index).cloned())
            .unwrap_or_else(|| "all".to_string());
        let entry = aggregates
            .entry(group)
            .or_insert(AggregateState { count: 0, sum: 0.0 });
        entry.count += 1;
        if let Some(value_index) = config.value_column {
            let value = row
                .values
                .get(value_index)
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or_default();
            entry.sum += clamp_value(value, config.lower_bound, config.upper_bound);
        }
    }
    if aggregates.is_empty() {
        aggregates.insert("all".to_string(), AggregateState { count: 0, sum: 0.0 });
    }
    aggregates
}

fn noisy_aggregate(
    group: &str,
    aggregate: &AggregateState,
    config: &DifferentialPrivacyConfig,
    deterministic: bool,
    seed: &str,
) -> f64 {
    match config.aggregate {
        DpAggregate::Count => {
            let noise = laplace_noise(1.0 / config.epsilon, deterministic, seed, group);
            f64::max(0.0, aggregate.count as f64 + noise)
        }
        DpAggregate::Sum => {
            let sensitivity = sum_sensitivity(config);
            aggregate.sum + laplace_noise(sensitivity / config.epsilon, deterministic, seed, group)
        }
        DpAggregate::Mean => {
            let sensitivity = sum_sensitivity(config);
            let noisy_sum = aggregate.sum
                + laplace_noise(
                    sensitivity / (config.epsilon / 2.0),
                    deterministic,
                    seed,
                    &format!("{group}:sum"),
                );
            let noisy_count = f64::max(
                1.0,
                aggregate.count as f64
                    + laplace_noise(
                        1.0 / (config.epsilon / 2.0),
                        deterministic,
                        seed,
                        &format!("{group}:count"),
                    ),
            );
            noisy_sum / noisy_count
        }
    }
}

fn aggregate_label(aggregate: DpAggregate) -> &'static str {
    match aggregate {
        DpAggregate::Count => "count",
        DpAggregate::Sum => "sum",
        DpAggregate::Mean => "mean",
    }
}

fn sum_sensitivity(config: &DifferentialPrivacyConfig) -> f64 {
    match (config.lower_bound, config.upper_bound) {
        (Some(lower), Some(upper)) => f64::max(lower.abs(), upper.abs()).max(upper - lower),
        _ => 1.0,
    }
}

fn clamp_value(value: f64, lower: Option<f64>, upper: Option<f64>) -> f64 {
    match (lower, upper) {
        (Some(lower), Some(upper)) => value.clamp(lower, upper),
        (Some(lower), None) => value.max(lower),
        (None, Some(upper)) => value.min(upper),
        (None, None) => value,
    }
}

fn laplace_noise(scale: f64, deterministic: bool, seed: &str, context: &str) -> f64 {
    let u = if deterministic {
        let hash = deterministic_hash(context, seed);
        let raw =
            u64::from_str_radix(&hash[..12], 16).unwrap_or(0) as f64 / 0xffffffffffff_u64 as f64;
        raw - 0.5
    } else {
        rand::thread_rng().gen_range(-0.5_f64..0.5_f64)
    };
    if u == 0.0 {
        return 0.0;
    }
    -scale * u.signum() * (1.0 - 2.0 * u.abs()).ln()
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

fn dp_notes(config: &DifferentialPrivacyConfig) -> Vec<String> {
    let mut notes = vec![
        "Differential privacy aggregate mode releases noisy statistics, not row-level source data."
            .to_string(),
        "Repeated releases spend additional privacy budget; track epsilon outside this file when publishing multiple outputs."
            .to_string(),
    ];
    if matches!(config.aggregate, DpAggregate::Mean) {
        notes.push("Mean releases split epsilon between noisy sum and noisy count.".to_string());
    }
    notes
}
