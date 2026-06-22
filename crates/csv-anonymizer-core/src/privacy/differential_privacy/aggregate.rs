use super::super::dataset::CsvDataset;
use super::validation::normalized_public_group_values;
use crate::error::{AnonymizerError, Result};
use crate::types::{DifferentialPrivacyConfig, DpAggregate};
use rand::Rng;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Debug, Clone, Copy)]
pub(super) struct AggregateState {
    count: usize,
    sum: f64,
}

pub(super) fn build_aggregates(
    dataset: &CsvDataset,
    config: &DifferentialPrivacyConfig,
) -> Result<BTreeMap<String, AggregateState>> {
    let mut aggregates: BTreeMap<String, AggregateState> = BTreeMap::new();
    let public_group_values = normalized_public_group_values(config)?;
    let public_group_set = (!public_group_values.is_empty()).then(|| {
        public_group_values
            .iter()
            .cloned()
            .collect::<BTreeSet<String>>()
    });
    for group in &public_group_values {
        aggregates.insert(group.clone(), AggregateState { count: 0, sum: 0.0 });
    }
    let mut privacy_unit_counts: HashMap<String, usize> = HashMap::new();
    let max_contributions = max_contributions_per_unit(config);

    for (row_index, row) in dataset.rows.iter().enumerate() {
        if row.is_blank {
            continue;
        }

        if let Some(unit_index) = config.privacy_unit_column {
            let unit = row
                .values
                .get(unit_index)
                .map(|value| value.trim())
                .unwrap_or("");
            if unit.is_empty() {
                return Err(AnonymizerError::Privacy(format!(
                    "DP privacy unit column cannot be empty; row {} has an empty privacy unit",
                    row_index + 2
                )));
            }
            let used = privacy_unit_counts.entry(unit.to_string()).or_default();
            if *used >= max_contributions {
                continue;
            }
            *used += 1;
        }

        let group = if let Some(group_index) = config.group_by_column {
            row.values
                .get(group_index)
                .map(|value| value.trim().to_string())
                .unwrap_or_default()
        } else {
            "all".to_string()
        };
        if let Some(public_group_set) = &public_group_set
            && !public_group_set.contains(&group)
        {
            return Err(AnonymizerError::Privacy(format!(
                "DP grouped release found group '{}' at row {} that is not in the configured allowed group values",
                group,
                row_index + 2
            )));
        }
        let entry = aggregates
            .entry(group)
            .or_insert(AggregateState { count: 0, sum: 0.0 });
        entry.count += 1;
        if let Some(value_index) = config
            .value_column
            .filter(|_| matches!(config.aggregate, DpAggregate::Sum | DpAggregate::Mean))
        {
            let raw = row
                .values
                .get(value_index)
                .map(String::as_str)
                .unwrap_or("");
            let value = raw.trim().parse::<f64>().map_err(|_| {
                AnonymizerError::Privacy(format!(
                    "DP {} release requires numeric values in the value column; row {} contains '{}'",
                    aggregate_label(config.aggregate),
                    row_index + 2,
                    raw
                ))
            })?;
            if !value.is_finite() {
                return Err(AnonymizerError::Privacy(format!(
                    "DP {} release requires finite numeric values in the value column; row {} contains '{}'",
                    aggregate_label(config.aggregate),
                    row_index + 2,
                    raw
                )));
            }
            entry.sum += clamp_value(value, config.lower_bound, config.upper_bound);
        }
    }
    if aggregates.is_empty() && config.group_by_column.is_none() {
        aggregates.insert("all".to_string(), AggregateState { count: 0, sum: 0.0 });
    }
    Ok(aggregates)
}

pub(super) fn noisy_aggregate(
    group: &str,
    aggregate: &AggregateState,
    config: &DifferentialPrivacyConfig,
    seed: &str,
) -> f64 {
    let contribution_limit = max_contributions_per_unit(config) as f64;
    match config.aggregate {
        DpAggregate::Count => {
            let noise = laplace_noise(contribution_limit / config.epsilon, seed, group);
            f64::max(0.0, aggregate.count as f64 + noise)
        }
        DpAggregate::Sum => {
            let sensitivity = contribution_limit * sum_sensitivity(config);
            aggregate.sum + laplace_noise(sensitivity / config.epsilon, seed, group)
        }
        DpAggregate::Mean => {
            let sensitivity = contribution_limit * sum_sensitivity(config);
            let noisy_sum = aggregate.sum
                + laplace_noise(
                    sensitivity / (config.epsilon / 2.0),
                    seed,
                    &format!("{group}:sum"),
                );
            let noisy_count = f64::max(
                1.0,
                aggregate.count as f64
                    + laplace_noise(
                        contribution_limit / (config.epsilon / 2.0),
                        seed,
                        &format!("{group}:count"),
                    ),
            );
            noisy_sum / noisy_count
        }
    }
}

pub(super) fn aggregate_label(aggregate: DpAggregate) -> &'static str {
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

pub(super) fn dp_input_column_count(config: &DifferentialPrivacyConfig) -> usize {
    let mut columns = BTreeSet::new();
    if let Some(index) = config.group_by_column {
        columns.insert(index);
    }
    if let Some(index) = config.value_column
        && matches!(config.aggregate, DpAggregate::Sum | DpAggregate::Mean)
    {
        columns.insert(index);
    }
    if let Some(index) = config.privacy_unit_column {
        columns.insert(index);
    }
    columns.len()
}

fn laplace_noise(scale: f64, _seed: &str, _context: &str) -> f64 {
    let u = rand::thread_rng().gen_range(-0.5_f64..0.5_f64);
    if u == 0.0 {
        return 0.0;
    }
    -scale * u.signum() * (1.0 - 2.0 * u.abs()).ln()
}

fn max_contributions_per_unit(config: &DifferentialPrivacyConfig) -> usize {
    if config.privacy_unit_column.is_some() {
        config.max_contributions_per_unit.unwrap_or(1)
    } else {
        1
    }
}
