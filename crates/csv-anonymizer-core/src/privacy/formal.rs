use super::dataset::{
    CsvDataset, check_canceled, read_dataset, report_progress, write_atomically, write_record,
};
use super::generalization::{MAX_GENERALIZATION_LEVEL, generalize_value};
use super::roles::{RolePlan, build_role_plan, validate_common_config};
use super::{PrivacyProcessResult, constrain_unselected_roles_to_attributes};
use crate::detection::is_empty_value;
use crate::error::{AnonymizerError, Result};
use crate::report_notes::push_unselected_column_note;
use crate::types::{
    ColumnMetadata, ColumnRole, DataType, FormalPrivacyConfig, PrivacyConfig, PrivacyModel,
    PrivacyModelReport, PrivacyReport, ProcessControl, ReleaseMode,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

const REDACTED_VALUE: &str = "[redacted]";

struct FormalWritePlan<'a> {
    columns: &'a [ColumnMetadata],
    role_plan: &'a RolePlan,
    quasi_indices: &'a [usize],
    levels: &'a HashMap<usize, u8>,
    suppressed_rows: &'a HashSet<usize>,
}

pub(super) fn process_formal_tabular(
    input_path: &Path,
    output_path: &Path,
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    mut control: Option<&mut ProcessControl<'_>>,
) -> Result<PrivacyProcessResult> {
    validate_common_config(columns, config)?;
    validate_formal_config(&config.formal)?;
    let start_time = Instant::now();
    let dataset = read_dataset(input_path, control.as_deref_mut())?;
    let mut role_plan = build_role_plan(columns, config)?;
    constrain_unselected_roles_to_attributes(columns, &mut role_plan.roles);
    let quasi_indices = role_plan.quasi_indices();
    let sensitive_indices = role_plan.sensitive_indices();
    let levels = choose_generalization_levels(&dataset, columns, &role_plan, &config.formal);
    let class_map = equivalence_classes(&dataset, columns, &quasi_indices, &levels);
    let suppressed_rows = if config.formal.suppress_small_classes {
        rows_to_suppress(&class_map, config.formal.k)
    } else {
        HashSet::new()
    };
    let write_plan = FormalWritePlan {
        columns,
        role_plan: &role_plan,
        quasi_indices: &quasi_indices,
        levels: &levels,
        suppressed_rows: &suppressed_rows,
    };
    let output_path = write_formal_release(output_path, &dataset, write_plan, control)?;
    let released_count = dataset
        .rows
        .iter()
        .enumerate()
        .filter(|(index, row)| !row.is_blank && !suppressed_rows.contains(index))
        .count();
    let formal_models = formal_model_reports(
        &dataset,
        columns,
        &quasi_indices,
        &sensitive_indices,
        &levels,
        &suppressed_rows,
        &config.formal,
    );
    let generalized_columns = quasi_indices
        .iter()
        .filter(|index| levels.get(index).copied().unwrap_or_default() > 0)
        .count();

    Ok(PrivacyProcessResult {
        row_count: dataset.data_row_count(),
        output_path,
        duration_ms: start_time.elapsed().as_millis(),
        columns_anonymized: role_plan.direct_count() + generalized_columns,
        privacy_report: PrivacyReport {
            release_mode: ReleaseMode::FormalTabular,
            direct_identifiers: role_plan.role_count(ColumnRole::DirectIdentifier),
            quasi_identifiers: quasi_indices.len(),
            sensitive_columns: sensitive_indices.len(),
            pseudonymized_columns: 0,
            smart_replacement_columns: 0,
            opaque_token_columns: 0,
            masked_columns: role_plan.role_count(ColumnRole::DirectIdentifier),
            generalized_columns,
            pass_through_columns: role_plan.role_count(ColumnRole::Attribute)
                + role_plan.role_count(ColumnRole::Sensitive),
            suppressed_rows: suppressed_rows.len(),
            synthetic_rows: 0,
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
            notes: formal_notes(columns, config, released_count),
        },
    })
}

fn validate_formal_config(config: &FormalPrivacyConfig) -> Result<()> {
    if config.k == 0 {
        return Err(AnonymizerError::Privacy(
            "k-anonymity requires k to be at least 1".to_string(),
        ));
    }
    if matches!(config.l_diversity, Some(0)) {
        return Err(AnonymizerError::Privacy(
            "l-diversity requires l to be at least 1".to_string(),
        ));
    }
    if let Some(t) = config.t_closeness
        && (!t.is_finite() || !(0.0..=1.0).contains(&t))
    {
        return Err(AnonymizerError::Privacy(
            "t-closeness requires t between 0 and 1 for categorical distance".to_string(),
        ));
    }
    Ok(())
}

fn choose_generalization_levels(
    dataset: &CsvDataset,
    columns: &[ColumnMetadata],
    role_plan: &RolePlan,
    config: &FormalPrivacyConfig,
) -> HashMap<usize, u8> {
    let quasi_indices = role_plan.quasi_indices();
    let mut levels = quasi_indices
        .iter()
        .map(|index| {
            (
                *index,
                role_plan
                    .explicit_levels
                    .get(index)
                    .copied()
                    .unwrap_or_default()
                    .min(MAX_GENERALIZATION_LEVEL),
            )
        })
        .collect::<HashMap<_, _>>();

    if quasi_indices.is_empty() || config.k <= 1 {
        return levels;
    }

    while min_class_size(&equivalence_classes(
        dataset,
        columns,
        &quasi_indices,
        &levels,
    ))
    .is_some_and(|size| size < config.k)
    {
        let mut advanced = false;
        for index in &quasi_indices {
            let level = levels.entry(*index).or_default();
            if *level < MAX_GENERALIZATION_LEVEL {
                *level += 1;
                advanced = true;
            }
        }
        if !advanced {
            break;
        }
    }

    levels
}

fn equivalence_classes(
    dataset: &CsvDataset,
    columns: &[ColumnMetadata],
    quasi_indices: &[usize],
    levels: &HashMap<usize, u8>,
) -> HashMap<Vec<String>, Vec<usize>> {
    let mut classes: HashMap<Vec<String>, Vec<usize>> = HashMap::new();
    for (row_index, row) in dataset.rows.iter().enumerate() {
        if row.is_blank {
            continue;
        }
        let key = quasi_indices
            .iter()
            .map(|column_index| {
                let value = row
                    .values
                    .get(*column_index)
                    .map(String::as_str)
                    .unwrap_or("");
                generalize_value(
                    value,
                    columns
                        .get(*column_index)
                        .map(|column| column.detected_type)
                        .unwrap_or(DataType::Unknown),
                    levels.get(column_index).copied().unwrap_or_default(),
                )
            })
            .collect::<Vec<_>>();
        classes.entry(key).or_default().push(row_index);
    }
    classes
}

fn rows_to_suppress(class_map: &HashMap<Vec<String>, Vec<usize>>, k: usize) -> HashSet<usize> {
    class_map
        .values()
        .filter(|rows| rows.len() < k)
        .flat_map(|rows| rows.iter().copied())
        .collect()
}

fn min_class_size(class_map: &HashMap<Vec<String>, Vec<usize>>) -> Option<usize> {
    class_map.values().map(Vec::len).min()
}

fn write_formal_release(
    output_path: &Path,
    dataset: &CsvDataset,
    plan: FormalWritePlan<'_>,
    control: Option<&mut ProcessControl<'_>>,
) -> Result<PathBuf> {
    write_atomically(output_path, control, |writer, control| {
        write_record(writer, dataset.headers.iter().map(String::as_str))?;
        let mut rows_written = 0;
        for (row_index, row) in dataset.rows.iter().enumerate() {
            check_canceled(control)?;
            if row.is_blank {
                write_record(writer, row.values.iter().map(String::as_str))?;
                continue;
            }
            if plan.suppressed_rows.contains(&row_index) {
                continue;
            }
            let mut output = row.values.clone();
            if output.len() < dataset.headers.len() {
                output.resize(dataset.headers.len(), String::new());
            }
            for (column_index, role) in plan.role_plan.roles.iter().enumerate() {
                let value = output.get(column_index).cloned().unwrap_or_default();
                output[column_index] = match role {
                    ColumnRole::DirectIdentifier => {
                        if is_empty_value(&value) {
                            value
                        } else {
                            REDACTED_VALUE.to_string()
                        }
                    }
                    ColumnRole::Exclude => String::new(),
                    ColumnRole::QuasiIdentifier if plan.quasi_indices.contains(&column_index) => {
                        generalize_value(
                            &value,
                            plan.columns
                                .get(column_index)
                                .map(|column| column.detected_type)
                                .unwrap_or(DataType::Unknown),
                            plan.levels.get(&column_index).copied().unwrap_or_default(),
                        )
                    }
                    _ => value,
                };
            }
            write_record(writer, output.iter().map(String::as_str))?;
            rows_written += 1;
            report_progress(control, rows_written);
        }
        Ok(())
    })
}

fn formal_model_reports(
    dataset: &CsvDataset,
    columns: &[ColumnMetadata],
    quasi_indices: &[usize],
    sensitive_indices: &[usize],
    levels: &HashMap<usize, u8>,
    suppressed_rows: &HashSet<usize>,
    config: &FormalPrivacyConfig,
) -> Vec<PrivacyModelReport> {
    let released_classes =
        released_equivalence_classes(dataset, columns, quasi_indices, levels, suppressed_rows);
    let min_size = min_class_size(&released_classes).unwrap_or(0);
    let data_rows = dataset.data_row_count();
    let k_satisfied = data_rows == 0 || min_size >= config.k;
    let mut reports = vec![PrivacyModelReport {
        model: PrivacyModel::KAnonymity,
        satisfied: k_satisfied,
        actual: min_size.to_string(),
        threshold: config.k.to_string(),
        message: if k_satisfied {
            "Every released equivalence class meets the configured k threshold.".to_string()
        } else {
            "Some released equivalence classes remain below the configured k threshold.".to_string()
        },
    }];

    if let Some(l) = config.l_diversity {
        let actual = min_distinct_sensitive_values(dataset, sensitive_indices, &released_classes);
        let satisfied = !sensitive_indices.is_empty() && actual >= l;
        reports.push(PrivacyModelReport {
            model: PrivacyModel::LDiversity,
            satisfied,
            actual: actual.to_string(),
            threshold: l.to_string(),
            message: if sensitive_indices.is_empty() {
                "l-diversity requires at least one sensitive column role.".to_string()
            } else if satisfied {
                "Every released equivalence class has enough distinct sensitive values.".to_string()
            } else {
                "Some released equivalence classes have too few distinct sensitive values."
                    .to_string()
            },
        });
    }

    if let Some(t) = config.t_closeness {
        let actual = max_sensitive_total_variation(dataset, sensitive_indices, &released_classes);
        let satisfied = !sensitive_indices.is_empty() && actual <= t;
        reports.push(PrivacyModelReport {
            model: PrivacyModel::TCloseness,
            satisfied,
            actual: format!("{actual:.3}"),
            threshold: format!("{t:.3}"),
            message: if sensitive_indices.is_empty() {
                "t-closeness requires at least one sensitive column role.".to_string()
            } else if satisfied {
                "Each released class is within the categorical distance threshold.".to_string()
            } else {
                "At least one released class exceeds the categorical distance threshold."
                    .to_string()
            },
        });
    }

    reports
}

fn released_equivalence_classes(
    dataset: &CsvDataset,
    columns: &[ColumnMetadata],
    quasi_indices: &[usize],
    levels: &HashMap<usize, u8>,
    suppressed_rows: &HashSet<usize>,
) -> HashMap<Vec<String>, Vec<usize>> {
    let all = equivalence_classes(dataset, columns, quasi_indices, levels);
    all.into_iter()
        .filter_map(|(key, rows)| {
            let released = rows
                .into_iter()
                .filter(|row_index| !suppressed_rows.contains(row_index))
                .collect::<Vec<_>>();
            (!released.is_empty()).then_some((key, released))
        })
        .collect()
}

fn min_distinct_sensitive_values(
    dataset: &CsvDataset,
    sensitive_indices: &[usize],
    classes: &HashMap<Vec<String>, Vec<usize>>,
) -> usize {
    if sensitive_indices.is_empty() || classes.is_empty() {
        return 0;
    }
    classes
        .values()
        .map(|rows| {
            sensitive_indices
                .iter()
                .map(|column_index| {
                    rows.iter()
                        .filter_map(|row_index| dataset.rows.get(*row_index))
                        .filter_map(|row| row.values.get(*column_index))
                        .filter(|value| !is_empty_value(value))
                        .collect::<BTreeSet<_>>()
                        .len()
                })
                .min()
                .unwrap_or(0)
        })
        .min()
        .unwrap_or(0)
}

fn max_sensitive_total_variation(
    dataset: &CsvDataset,
    sensitive_indices: &[usize],
    classes: &HashMap<Vec<String>, Vec<usize>>,
) -> f64 {
    if sensitive_indices.is_empty() || classes.is_empty() {
        return 0.0;
    }
    let mut max_distance = 0.0;
    for column_index in sensitive_indices {
        let overall = distribution_for_rows(
            dataset,
            *column_index,
            dataset
                .rows
                .iter()
                .enumerate()
                .filter_map(|(index, row)| (!row.is_blank).then_some(index)),
        );
        for rows in classes.values() {
            let class_distribution =
                distribution_for_rows(dataset, *column_index, rows.iter().copied());
            max_distance = f64::max(
                max_distance,
                total_variation_distance(&overall, &class_distribution),
            );
        }
    }
    max_distance
}

fn distribution_for_rows(
    dataset: &CsvDataset,
    column_index: usize,
    row_indices: impl Iterator<Item = usize>,
) -> BTreeMap<String, f64> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0_usize;
    for row_index in row_indices {
        let value = dataset
            .rows
            .get(row_index)
            .and_then(|row| row.values.get(column_index))
            .cloned()
            .unwrap_or_default();
        if is_empty_value(&value) {
            continue;
        }
        *counts.entry(value).or_default() += 1;
        total += 1;
    }
    if total == 0 {
        return BTreeMap::new();
    }
    counts
        .into_iter()
        .map(|(value, count)| (value, count as f64 / total as f64))
        .collect()
}

fn total_variation_distance(
    overall: &BTreeMap<String, f64>,
    class_distribution: &BTreeMap<String, f64>,
) -> f64 {
    let keys = overall
        .keys()
        .chain(class_distribution.keys())
        .collect::<BTreeSet<_>>();
    keys.into_iter()
        .map(|key| {
            (overall.get(key).copied().unwrap_or_default()
                - class_distribution.get(key).copied().unwrap_or_default())
            .abs()
        })
        .sum::<f64>()
        / 2.0
}

fn formal_notes(
    columns: &[ColumnMetadata],
    config: &PrivacyConfig,
    released_count: usize,
) -> Vec<String> {
    let mut notes = vec![
        "Formal tabular release mode generalizes quasi-identifiers, redacts direct identifiers, and can suppress rows below the k threshold."
            .to_string(),
        format!("{released_count} row(s) were released after formal privacy checks."),
    ];
    push_unselected_column_note(&mut notes, columns);
    if config.formal.l_diversity.is_some() {
        notes.push(
            "l-diversity is evaluated with distinct sensitive values per equivalence class."
                .to_string(),
        );
    }
    if config.formal.t_closeness.is_some() {
        notes.push(
            "t-closeness uses categorical total variation distance in this local MVP.".to_string(),
        );
    }
    notes
}
