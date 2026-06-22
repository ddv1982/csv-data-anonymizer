mod dataset;
mod differential_privacy;
mod formal;
mod generalization;
mod roles;
mod synthetic;

use crate::error::{AnonymizerError, Result};
use crate::types::{ColumnMetadata, PrivacyConfig, PrivacyReport, ProcessControl, ReleaseMode};
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
