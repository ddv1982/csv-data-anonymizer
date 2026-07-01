use super::*;
use crate::smart::SmartReplacementMap;
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, Confidence, EmptyFormat, PiiRisk,
    PrivacyEvidenceSummary, PrivacyFindingKind,
};

fn column(detected_type: DataType) -> ColumnMetadata {
    ColumnMetadata {
        name: "value".to_string(),
        source_path: None,
        index: 0,
        detected_type,
        confidence: Confidence::High,
        detection_trace: None,
        privacy_findings: Vec::new(),
        privacy_evidence: Vec::new(),
        pii_risk: PiiRisk::Medium,
        sample_values: vec![],
        empty_format: EmptyFormat::EmptyString,
        is_selected: true,
        strategy: AnonymizationStrategy::Auto,
    }
}

fn context() -> TransformContext<'static> {
    TransformContext {
        column_name: "value",
        column_index: 0,
        row_index: 0,
        empty_format: EmptyFormat::EmptyString,
    }
}

#[test]
fn email_preserves_domain() {
    let result = transform_value("john.doe@example.com", &column(DataType::Email), &context());
    assert!(result.ends_with("@example.com"));
    assert_ne!(result, "john.doe@example.com");
}

#[test]
fn uuid_preserves_uppercase() {
    let result = transform_value(
        "550E8400-E29B-41D4-A716-446655440000",
        &column(DataType::Uuid),
        &context(),
    );
    assert_eq!(result, result.to_uppercase());
}

#[test]
fn uuid_random_mode_generates_different_valid_uuid() {
    let original = "550e8400-e29b-41d4-a716-446655440000";

    let first = transform_value(original, &column(DataType::Uuid), &context());
    let second = transform_value(original, &column(DataType::Uuid), &context());

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
        &context(),
    );
    assert!(result.ends_with(" 10:30:45.123456"));
    assert_ne!(result, "2024-06-15 10:30:45.123456");
}

#[test]
fn numeric_id_preserves_leading_zeros() {
    let result = transform_value("001234", &column(DataType::NumericId), &context());
    assert_ne!(result, "001234");
    assert!(result.starts_with("00"));
    assert_eq!(result.len(), 6);
}

#[test]
fn numeric_id_all_zero_value_is_replaced() {
    let result = transform_value("0000", &column(DataType::NumericId), &context());
    assert_eq!(result.len(), 4);
    assert_ne!(result, "0000");
    assert!(result.chars().all(|character| character.is_ascii_digit()));
}

#[test]
fn numeric_string_fallback_currently_uses_generic_string_strategy() {
    let result = transform_value("123", &column(DataType::String), &context());
    assert_ne!(result, "123");
    assert!(result.chars().any(|character| !character.is_ascii_digit()));
}

#[test]
fn numeric_value_preserves_integer_shape() {
    let result = transform_value("007", &column(DataType::NumericValue), &context());
    assert_ne!(result, "007");
    assert_eq!(result.len(), 3);
    assert!(result.starts_with("00"));
    assert!(result.chars().all(|character| character.is_ascii_digit()));
}

#[test]
fn numeric_value_preserves_signed_decimal_shape() {
    let result = transform_value("-12.50", &column(DataType::NumericValue), &context());
    assert_ne!(result, "-12.50");
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
    let result = transform_value("555-867-5309", &column(DataType::Phone), &context());
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
fn redact_uses_typed_placeholders() {
    let mut email_column = column(DataType::Email);
    email_column.strategy = AnonymizationStrategy::Redact;
    assert_eq!(
        transform_value("john.doe@example.com", &email_column, &context()),
        "[EMAIL]"
    );

    let mut name_column = column(DataType::FirstName);
    name_column.strategy = AnonymizationStrategy::Redact;
    assert_eq!(transform_value("Ada", &name_column, &context()), "[PERSON]");

    let mut date_column = column(DataType::Timestamp);
    date_column.strategy = AnonymizationStrategy::Redact;
    assert_eq!(
        transform_value("2024-06-15", &date_column, &context()),
        "[DATE]"
    );

    let mut username_column = column(DataType::String);
    username_column.strategy = AnonymizationStrategy::Redact;
    username_column.privacy_evidence = vec![PrivacyEvidenceSummary {
        kind: PrivacyFindingKind::AccountOrFinancialId,
        data_type: DataType::String,
        confidence: Confidence::Medium,
        match_count: 1,
        sample_count: 1,
        score: 76,
        detector: "header:taxonomy:account-identifier".to_string(),
        reason: "Header terms suggest an account or user identifier.".to_string(),
        detectors: vec!["header:taxonomy:account-identifier".to_string()],
    }];
    assert_eq!(
        transform_value("johndoe", &username_column, &context()),
        "[ACCOUNT_ID]"
    );
}

#[test]
fn first_and_last_names_use_plausible_name_values() {
    let first = transform_value("Alice", &column(DataType::FirstName), &context());
    let last = transform_value("Smith", &column(DataType::LastName), &context());

    assert_ne!(first, "Alice");
    assert_ne!(last, "Smith");
    assert!(first.chars().all(|character| character.is_alphabetic()));
    assert!(last.chars().all(|character| character.is_alphabetic()));
}

#[test]
fn name_tokens_do_not_preserve_original_pool_values() {
    let first = transform_value("Dana", &column(DataType::FirstName), &context());
    let full = transform_value("Dana Morgan", &column(DataType::FullName), &context());

    assert_ne!(first, "Dana");
    assert!(!full.split_whitespace().any(|token| {
        token.eq_ignore_ascii_case("Dana") || token.eq_ignore_ascii_case("Morgan")
    }));
}

#[test]
fn full_name_excludes_original_tokens_across_random_draws() {
    for _ in 0..100 {
        let result = transform_value("Dana Morgan", &column(DataType::FullName), &context());

        assert!(!result.split_whitespace().any(|token| {
            token.eq_ignore_ascii_case("Dana") || token.eq_ignore_ascii_case("Morgan")
        }));
    }
}

#[test]
fn full_name_preserves_token_shape_with_plausible_names() {
    let result = transform_value("Alice Smith", &column(DataType::FullName), &context());
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
    let result = transform_value("Carol O'Neil", &column(DataType::FullName), &context());

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
    let mut state = TransformState::new();
    let context = context();
    let first =
        transform_value_with_state("Alice", &column(DataType::FirstName), &context, &mut state);
    let last =
        transform_value_with_state("Smith", &column(DataType::LastName), &context, &mut state);
    let full = transform_value_with_state(
        "Alice Smith",
        &column(DataType::FullName),
        &context,
        &mut state,
    );

    assert_eq!(full, format!("{first} {last}"));
}

#[test]
fn stateful_name_mapping_keeps_distinct_sources_unique_while_pool_has_capacity() {
    let mut state = TransformState::new();
    let first_name_column = column(DataType::FirstName);
    let context = context();
    let originals = [
        "Alice", "Bianca", "Celine", "Daphne", "Elise", "Freya", "Gemma", "Helena", "Iris",
        "Jenna", "Keira", "Lena", "Mara", "Nadia", "Opal", "Priya", "Rhea", "Selah", "Talia",
        "Una",
    ];

    let outputs = originals
        .iter()
        .map(|name| transform_value_with_state(name, &first_name_column, &context, &mut state))
        .collect::<Vec<_>>();
    let unique_outputs = outputs.iter().collect::<std::collections::HashSet<_>>();

    assert_eq!(unique_outputs.len(), originals.len());
    assert!(
        outputs
            .iter()
            .all(|name| name.chars().all(|character| character.is_alphabetic()))
    );
    assert_eq!(state.report().unique_pseudonym_values, originals.len());
    assert_eq!(state.report().exhausted_pseudonym_pools, 0);
}

#[test]
fn stateful_name_mapping_reuses_existing_source_mapping() {
    let mut state = TransformState::new();
    let first_name_column = column(DataType::FirstName);
    let context = context();

    let first = transform_value_with_state("Alice", &first_name_column, &context, &mut state);
    let second = transform_value_with_state("Alice", &first_name_column, &context, &mut state);
    let third = transform_value_with_state("Bianca", &first_name_column, &context, &mut state);

    assert_eq!(first, second);
    assert_ne!(first, third);
    assert_eq!(state.report().unique_pseudonym_values, 2);
    assert_eq!(state.report().reused_pseudonym_values, 1);
}

#[test]
fn stateful_full_name_reuses_first_and_last_domains() {
    let mut state = TransformState::new();
    let first_name_column = column(DataType::FirstName);
    let last_name_column = column(DataType::LastName);
    let full_name_column = column(DataType::FullName);
    let context = context();

    let first = transform_value_with_state("Alice", &first_name_column, &context, &mut state);
    let last = transform_value_with_state("Smith", &last_name_column, &context, &mut state);
    let full = transform_value_with_state("Alice Smith", &full_name_column, &context, &mut state);

    assert_eq!(full, format!("{first} {last}"));
    assert_eq!(state.report().reused_pseudonym_values, 2);
}

#[test]
fn full_name_preserves_one_token_outlier_shape() {
    let result = transform_value("Alice", &column(DataType::FullName), &context());
    assert_eq!(result.split_whitespace().count(), 1);
    assert!(result.chars().all(|character| character.is_alphabetic()));
}

#[test]
fn country_code_and_enum_are_currently_pass_through() {
    assert_eq!(
        transform_value("US", &column(DataType::CountryCode), &context()),
        "US"
    );
    assert_eq!(
        transform_value("active", &column(DataType::Enum), &context()),
        "active"
    );
}

#[test]
fn unknown_values_use_generic_string_strategy() {
    let result = transform_value("mystery", &column(DataType::Unknown), &context());
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
        transform_value("john@example.com", &masked, &context()),
        "****************"
    );

    let mut pass_through = column(DataType::Email);
    pass_through.strategy = AnonymizationStrategy::PassThrough;
    assert_eq!(
        transform_value("john@example.com", &pass_through, &context()),
        "john@example.com"
    );
}

#[test]
fn tokenize_strategy_emits_consistent_opaque_tokens() {
    let mut token_column = column(DataType::Email);
    token_column.strategy = AnonymizationStrategy::Tokenize;
    let mut state = TransformState::new();
    let context = context();

    let first =
        transform_value_with_state("alice@example.com", &token_column, &context, &mut state);
    let repeated =
        transform_value_with_state("alice@example.com", &token_column, &context, &mut state);
    let second = transform_value_with_state("bob@example.com", &token_column, &context, &mut state);

    assert_eq!(first, repeated);
    assert_ne!(first, second);
    assert!(first.starts_with("tok_"));
    assert_eq!(state.report().opaque_token_values, 2);
}

#[test]
fn local_ai_strategy_uses_validated_replacement_map() {
    let mut local_ai_column = column(DataType::FullName);
    local_ai_column.strategy = AnonymizationStrategy::LocalAi;
    let mut replacements = SmartReplacementMap::default();
    replacements.insert(0, "Alice Smith", "Maya Carter");
    let mut state = TransformState::with_smart_replacements(replacements);
    let context = context();

    let result = transform_value_with_state("Alice Smith", &local_ai_column, &context, &mut state);

    assert_eq!(result, "Maya Carter");
    assert_eq!(state.report().smart_replacement_values, 1);
    assert_eq!(state.report().smart_replacement_fallbacks, 0);
}

#[test]
fn local_ai_strategy_falls_back_when_map_is_missing() {
    let mut local_ai_column = column(DataType::FirstName);
    local_ai_column.strategy = AnonymizationStrategy::LocalAi;
    let mut state = TransformState::new();
    let context = context();

    let result = transform_value_with_state("Alice", &local_ai_column, &context, &mut state);

    assert_ne!(result, "Alice");
    assert!(result.chars().all(|character| character.is_alphabetic()));
    assert_eq!(state.report().smart_replacement_fallbacks, 1);
}

#[test]
fn email_without_at_sign_falls_back_to_generic_pseudonym() {
    let mut state = TransformState::new();
    let result = transform_value_with_state(
        "jane.doe at gmail",
        &column(DataType::Email),
        &context(),
        &mut state,
    );
    assert_ne!(result, "jane.doe at gmail");
    assert!(!result.contains("jane"));
    assert_eq!(state.report().shape_fallback_values, 1);
}

#[test]
fn timestamp_multibyte_value_does_not_panic_and_falls_back() {
    let mut state = TransformState::new();
    let result = transform_value_with_state(
        "2024年3月4日",
        &column(DataType::Timestamp),
        &context(),
        &mut state,
    );
    assert_ne!(result, "2024年3月4日");
    assert_eq!(state.report().shape_fallback_values, 1);
}

#[test]
fn timestamp_non_iso_value_falls_back_instead_of_passing_through() {
    let mut state = TransformState::new();
    let result = transform_value_with_state(
        "06/15/2024",
        &column(DataType::Timestamp),
        &context(),
        &mut state,
    );
    assert_ne!(result, "06/15/2024");
    assert_eq!(state.report().shape_fallback_values, 1);
}

#[test]
fn phone_with_surrounding_text_falls_back_instead_of_leaking_text() {
    let mut state = TransformState::new();
    let result = transform_value_with_state(
        "John Doe (555) 123-4567",
        &column(DataType::Phone),
        &context(),
        &mut state,
    );
    assert!(!result.contains("John"));
    assert!(!result.contains("Doe"));
    assert_eq!(state.report().shape_fallback_values, 1);
}

#[test]
fn phone_without_enough_digits_falls_back() {
    let mut state = TransformState::new();
    let result = transform_value_with_state(
        "call after 5",
        &column(DataType::Phone),
        &context(),
        &mut state,
    );
    assert!(!result.contains("call"));
    assert_eq!(state.report().shape_fallback_values, 1);
}

#[test]
fn phone_with_extension_marker_keeps_phone_shape() {
    let mut state = TransformState::new();
    let result = transform_value_with_state(
        "555-867-5309 ext 22",
        &column(DataType::Phone),
        &context(),
        &mut state,
    );
    assert!(result.contains("ext"));
    assert_eq!(state.report().shape_fallback_values, 0);
}

#[test]
fn padded_duplicate_row_values_map_to_the_same_pseudonym() {
    let columns = vec![column(DataType::Email)];
    let mut state = TransformState::new();
    let first = transform_row_with_state(
        &["john.doe@example.com".to_string()],
        &columns,
        0,
        &mut state,
    );
    let second = transform_row_with_state(
        &["  john.doe@example.com  ".to_string()],
        &columns,
        1,
        &mut state,
    );
    assert_eq!(first[0], second[0]);
}

#[test]
fn padded_null_cell_is_preserved_not_transformed() {
    let columns = vec![column(DataType::String)];
    let mut state = TransformState::new();
    let row = transform_row_with_state(&[" null ".to_string()], &columns, 0, &mut state);
    assert_eq!(row[0], " null ");
}

#[test]
fn padded_timestamp_cell_is_transformed_from_trimmed_value() {
    let columns = vec![column(DataType::Timestamp)];
    let mut state = TransformState::new();
    let row = transform_row_with_state(&[" 2024-06-15".to_string()], &columns, 0, &mut state);
    assert_ne!(row[0], " 2024-06-15");
    // The trimmed value is a valid ISO date, so the transform must keep the
    // ISO shape rather than corrupting it through byte-offset math.
    assert!(
        chrono::NaiveDate::parse_from_str(&row[0], "%Y-%m-%d").is_ok(),
        "expected ISO date, got {}",
        row[0]
    );
    assert_eq!(state.report().shape_fallback_values, 0);
}
