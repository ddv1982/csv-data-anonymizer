use super::*;
use crate::types::{AnonymizationStrategy, ColumnMetadata, Confidence, EmptyFormat, PiiRisk};

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
        strategy: AnonymizationStrategy::Auto,
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
fn uuid_random_mode_generates_different_valid_uuid() {
    let mut random_context = context("seed");
    random_context.deterministic = false;
    let original = "550e8400-e29b-41d4-a716-446655440000";

    let first = transform_value(original, &column(DataType::Uuid), &random_context);
    let second = transform_value(original, &column(DataType::Uuid), &random_context);

    assert_ne!(first, original);
    assert_ne!(first, second);
    assert_eq!(first.len(), original.len());
    assert_eq!(&first[14..15], "4");
    assert!(matches!(&first[19..20], "8" | "9" | "a" | "b"));
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

#[test]
fn numeric_id_all_zero_value_is_replaced() {
    let result = transform_value("0000", &column(DataType::NumericId), &context("seed"));
    assert_eq!(result.len(), 4);
    assert_ne!(result, "0000");
    assert!(result.chars().all(|character| character.is_ascii_digit()));
}

#[test]
fn numeric_string_fallback_currently_uses_generic_string_strategy() {
    let result = transform_value("123", &column(DataType::String), &context("seed"));
    assert_ne!(result, "123");
    assert!(result.chars().any(|character| !character.is_ascii_digit()));
}

#[test]
fn numeric_value_preserves_integer_shape() {
    let result = transform_value("007", &column(DataType::NumericValue), &context("seed"));
    assert_eq!(result.len(), 3);
    assert!(result.starts_with("00"));
    assert!(result.chars().all(|character| character.is_ascii_digit()));
}

#[test]
fn numeric_value_preserves_signed_decimal_shape() {
    let result = transform_value("-12.50", &column(DataType::NumericValue), &context("seed"));
    assert_eq!(result.len(), 6);
    assert!(result.starts_with('-'));
    assert_eq!(
        result.chars().filter(|character| *character == '.').count(),
        1
    );
    assert_eq!(result.split_once('.').unwrap().1.len(), 2);
    assert!(
        result
            .chars()
            .filter(|character| *character != '-' && *character != '.')
            .all(|character| character.is_ascii_digit())
    );
}

#[test]
fn phone_preserves_punctuation_shape() {
    let result = transform_value("555-867-5309", &column(DataType::Phone), &context("seed"));
    assert_ne!(result, "555-867-5309");
    assert_eq!(result.len(), "555-867-5309".len());
    assert_eq!(
        result.chars().filter(|character| *character == '-').count(),
        2
    );
    assert!(
        result
            .chars()
            .filter(|character| *character != '-')
            .all(|character| character.is_ascii_digit())
    );
}

#[test]
fn first_and_last_names_use_plausible_name_values() {
    let first = transform_value("Alice", &column(DataType::FirstName), &context("seed"));
    let last = transform_value("Smith", &column(DataType::LastName), &context("seed"));

    assert_ne!(first, "Alice");
    assert_ne!(last, "Smith");
    assert!(first.chars().all(|character| character.is_alphabetic()));
    assert!(last.chars().all(|character| character.is_alphabetic()));
}

#[test]
fn name_tokens_do_not_preserve_original_pool_values() {
    let first = transform_value("Dana", &column(DataType::FirstName), &context("seed"));
    let full = transform_value("Dana Morgan", &column(DataType::FullName), &context("seed"));

    assert_ne!(first, "Dana");
    assert!(!full.split_whitespace().any(|token| {
        token.eq_ignore_ascii_case("Dana") || token.eq_ignore_ascii_case("Morgan")
    }));
}

#[test]
fn full_name_excludes_original_tokens_across_seed_variations() {
    for index in 0..100 {
        let seed = format!("seed-{index}");
        let result = transform_value("Dana Morgan", &column(DataType::FullName), &context(&seed));

        assert!(!result.split_whitespace().any(|token| {
            token.eq_ignore_ascii_case("Dana") || token.eq_ignore_ascii_case("Morgan")
        }));
    }
}

#[test]
fn full_name_preserves_token_shape_with_plausible_names() {
    let result = transform_value("Alice Smith", &column(DataType::FullName), &context("seed"));
    assert_ne!(result, "Alice Smith");
    assert_eq!(result.split_whitespace().count(), 2);
    assert!(
        result
            .split_whitespace()
            .all(|token| token.chars().all(|character| character.is_alphabetic()))
    );
}

#[test]
fn full_name_uses_alphabetic_name_tokens() {
    let result = transform_value(
        "Carol O'Neil",
        &column(DataType::FullName),
        &context("name-quality-seed"),
    );

    assert_ne!(result, "Carol O'Neil");
    assert_eq!(result.split_whitespace().count(), 2);
    assert!(
        result
            .chars()
            .all(|character| character.is_alphabetic() || character.is_whitespace())
    );
    assert!(
        !result
            .chars()
            .any(|character| character.is_ascii_digit() || matches!(character, '_' | '-'))
    );
}

#[test]
fn full_name_reuses_first_and_last_token_pseudonyms() {
    let first = transform_value(
        "Alice",
        &column(DataType::FirstName),
        &context("consistent-name-seed"),
    );
    let last = transform_value(
        "Smith",
        &column(DataType::LastName),
        &context("consistent-name-seed"),
    );
    let full = transform_value(
        "Alice Smith",
        &column(DataType::FullName),
        &context("consistent-name-seed"),
    );

    assert_eq!(full, format!("{first} {last}"));
}

#[test]
fn full_name_preserves_one_token_outlier_shape() {
    let result = transform_value("Alice", &column(DataType::FullName), &context("seed"));
    assert_eq!(result.split_whitespace().count(), 1);
    assert!(result.chars().all(|character| character.is_alphabetic()));
}

#[test]
fn country_code_and_enum_are_currently_pass_through() {
    assert_eq!(
        transform_value("US", &column(DataType::CountryCode), &context("seed")),
        "US"
    );
    assert_eq!(
        transform_value("active", &column(DataType::Enum), &context("seed")),
        "active"
    );
}

#[test]
fn unknown_values_use_generic_string_strategy() {
    let result = transform_value("mystery", &column(DataType::Unknown), &context("seed"));
    assert_ne!(result, "mystery");
    assert!(
        result
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    );
}

#[test]
fn strategy_overrides_can_mask_or_pass_through() {
    let mut masked = column(DataType::Email);
    masked.strategy = AnonymizationStrategy::Mask;
    assert_eq!(
        transform_value("john@example.com", &masked, &context("seed")),
        "****************"
    );

    let mut pass_through = column(DataType::Email);
    pass_through.strategy = AnonymizationStrategy::PassThrough;
    assert_eq!(
        transform_value("john@example.com", &pass_through, &context("seed")),
        "john@example.com"
    );
}
