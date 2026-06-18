use csv_anonymizer_core::{ColumnMetadata, PiiRisk};
use std::path::{Path, PathBuf};

pub(crate) fn default_output_path_with_suffix(input_path: &Path, suffix: &str) -> PathBuf {
    let suffix = if suffix.trim().is_empty() {
        "_anonymized"
    } else {
        suffix.trim()
    };
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let file_name = match input_path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => format!("{stem}{suffix}.{extension}"),
        _ => format!("{stem}{suffix}"),
    };
    input_path.with_file_name(file_name)
}

pub(crate) fn should_auto_select(column: &ColumnMetadata) -> bool {
    !column.sample_values.is_empty() && matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_output_path_with_custom_suffix() {
        assert_eq!(
            default_output_path_with_suffix(Path::new("/tmp/data.csv"), "_private"),
            PathBuf::from("/tmp/data_private.csv")
        );
    }
}
