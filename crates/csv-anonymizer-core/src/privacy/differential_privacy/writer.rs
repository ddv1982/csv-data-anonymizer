use super::super::dataset::{CsvDataset, check_canceled, report_progress, write_atomically};
use super::aggregate::{aggregate_label, build_aggregates, noisy_aggregate};
use super::budget::format_epsilon;
use crate::error::{Result, csv_error};
use crate::types::{ColumnMetadata, DifferentialPrivacyConfig, ProcessControl};
use std::path::{Path, PathBuf};

pub(super) fn write_dp_aggregate_release(
    output_path: &Path,
    dataset: &CsvDataset,
    columns: &[ColumnMetadata],
    config: &DifferentialPrivacyConfig,
    seed: &str,
    control: Option<&mut ProcessControl<'_>>,
) -> Result<(PathBuf, usize)> {
    let aggregates = build_aggregates(dataset, config)?;
    let released_row_count = aggregates.len();
    let output_path = write_atomically(output_path, control, |writer, control| {
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
            let noisy = noisy_aggregate(&group, &aggregate, config, seed);
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
    })?;
    Ok((output_path, released_row_count))
}
