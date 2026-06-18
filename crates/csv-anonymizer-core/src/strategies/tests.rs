use super::*;
use crate::types::{ColumnMetadata, Confidence, EmptyFormat, PiiRisk};

fn column(detected_type: DataType) -> ColumnMetadata {
    ColumnMetadata {
        name: "value".to_string(),
        index: 0,
        detected_type,
        confidence: Confidence::High,
        pii_risk: PiiRisk::Medium,
        sample_values: vec![],
        empty_format: EmptyFormat::EmptyString,
        is_selected: true,
    }
}

fn context<'a>(seed: &'a str) -> TransformContext<'a> {
    TransformContext {
        column_name: "value",
        column_index: 0,
        row_index: 0,
        seed,
        deterministic: true,
        empty_format: EmptyFormat::EmptyString,
    }
}

#[test]
fn email_preserves_domain() {
    let result = transform_value(
        "john.doe@example.com",
        &column(DataType::Email),
        &context("seed"),
    );
    assert!(result.ends_with("@example.com"));
    assert_ne!(result, "john.doe@example.com");
}

#[test]
fn uuid_preserves_uppercase() {
    let result = transform_value(
        "550E8400-E29B-41D4-A716-446655440000",
        &column(DataType::Uuid),
        &context("seed"),
    );
    assert_eq!(result, result.to_uppercase());
}

#[test]
fn timestamp_preserves_time() {
    let result = transform_value(
        "2024-06-15 10:30:45.123456",
        &column(DataType::Timestamp),
        &context("seed"),
    );
    assert!(result.ends_with(" 10:30:45.123456"));
    assert_ne!(result, "2024-06-15 10:30:45.123456");
}

#[test]
fn numeric_id_preserves_leading_zeros() {
    let result = transform_value("001234", &column(DataType::NumericId), &context("seed"));
    assert!(result.starts_with("00"));
    assert_eq!(result.len(), 6);
}
