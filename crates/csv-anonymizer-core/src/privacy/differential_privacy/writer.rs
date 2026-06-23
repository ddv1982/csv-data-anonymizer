use super::super::dataset::{
    CsvDataset, check_canceled, report_progress, write_atomically, write_record,
};
use super::aggregate::{aggregate_label, build_aggregates, noisy_aggregate};
use super::budget::format_epsilon;
use crate::error::Result;
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
            write_record(writer, [group_name, "aggregate", "noisyValue", "epsilon"])?;
        } else {
            write_record(writer, ["aggregate", "noisyValue", "epsilon"])?;
        }

        let mut rows_written = 0;
        for (group, aggregate) in aggregates {
            check_canceled(control)?;
            let noisy = noisy_aggregate(&group, &aggregate, config, seed);
            let noisy_value = format!("{noisy:.6}");
            let epsilon = format_epsilon(config.epsilon);
            if config.group_by_column.is_some() {
                write_record(
                    writer,
                    [
                        group.as_str(),
                        aggregate_label(config.aggregate),
                        noisy_value.as_str(),
                        epsilon.as_str(),
                    ],
                )?;
            } else {
                write_record(
                    writer,
                    [
                        aggregate_label(config.aggregate),
                        noisy_value.as_str(),
                        epsilon.as_str(),
                    ],
                )?;
            }
            rows_written += 1;
            report_progress(control, rows_written);
        }
        Ok(())
    })?;
    Ok((output_path, released_row_count))
}
