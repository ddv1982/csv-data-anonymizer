use csv_anonymizer_core::{ColumnMetadata, PiiRisk};

pub(crate) fn should_auto_select(column: &ColumnMetadata) -> bool {
    !column.sample_values.is_empty() && matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium)
}
