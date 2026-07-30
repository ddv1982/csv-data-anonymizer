use super::*;
use crate::smart::SmartReplacementMap;
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, Confidence, EmptyFormat, PiiRisk,
    PrivacyEvidenceSummary, PrivacyFindingKind,
};

mod ledger_invariant;
mod mapping_budget;

/// Every `AnonymizationStrategy`, in declaration order.
///
/// The successor `match` below has no wildcard arm, so adding a variant to
/// `AnonymizationStrategy` stops this file compiling until an arm for it is written.
/// A hand-written array would have compiled fine and simply not been walked, which
/// would skip the one strategy nobody had cross-checked yet — the same strategy whose
/// behaviour was just edited. Written the same way as `all_data_types` in
/// `detection/tests/privacy.rs`, for the same reason.
///
/// What the compiler guarantees is the arm, not its position: an arm no other arm
/// points at leaves its variant off the walk. So chain a new variant in where it is
/// declared, and read the compile error as the instruction to do so rather than as a
/// stray arm to fill in. The `None` arm marks the last variant, and the assertion
/// below rejects a chain that revisits a strategy.
///
/// Lives here, in the parent of the test modules, because more than one of them walks
/// the strategies: `ledger_invariant` cross-checks the ledger against the cardinality
/// warning, and `mapping_budget` cross-checks the projected entry cost against the
/// entries a transform records.
fn all_strategies() -> impl Iterator<Item = AnonymizationStrategy> {
    fn next_strategy(current: AnonymizationStrategy) -> Option<AnonymizationStrategy> {
        // No `_ =>` arm. Adding a variant must break this match.
        match current {
            AnonymizationStrategy::Auto => Some(AnonymizationStrategy::Pseudonymize),
            AnonymizationStrategy::Pseudonymize => Some(AnonymizationStrategy::Tokenize),
            AnonymizationStrategy::Tokenize => Some(AnonymizationStrategy::LocalAi),
            AnonymizationStrategy::LocalAi => Some(AnonymizationStrategy::Mask),
            AnonymizationStrategy::Mask => Some(AnonymizationStrategy::Label),
            AnonymizationStrategy::Label => Some(AnonymizationStrategy::Redact),
            AnonymizationStrategy::Redact => Some(AnonymizationStrategy::PassThrough),
            AnonymizationStrategy::PassThrough => None,
        }
    }

    let mut seen = Vec::new();
    std::iter::successors(Some(AnonymizationStrategy::Auto), |current| {
        next_strategy(*current)
    })
    .inspect(move |&strategy| {
        // A variant chained back into the middle of the sequence would make the walk
        // cycle forever instead of failing, so the repeat is caught here.
        assert!(
            !seen.contains(&strategy),
            "{strategy:?} appears twice in the successor chain, so the walk does not \
             terminate and cannot cover every strategy"
        );
        seen.push(strategy);
    })
}

/// The one selected column the transformers are asked about, called `value` at index 0.
///
/// `Auto` rather than the `Default` of `PassThrough`, because these tests exist to observe
/// a transform and pass-through performs none: a fixture that defaulted the strategy would
/// leave every value unchanged and the assertions would be about nothing.
fn column(detected_type: DataType) -> ColumnMetadata {
    crate::test_support::selected_column(0, "value", detected_type, AnonymizationStrategy::Auto)
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

/// A numeric-looking `String` column gets the generic pseudonym, not the
/// numeric strategy's length- and digit-preserving treatment. The distinguishing
/// property is that the generic charset is alphanumeric, but any single draw can
/// legitimately come out all digits (about once in a hundred for a 3-character
/// value), so it is checked across draws rather than once.
#[test]
fn numeric_string_fallback_currently_uses_generic_string_strategy() {
    let mut saw_non_digit = false;
    for _ in 0..64 {
        let result = transform_value("123", &column(DataType::String), &context());
        assert_ne!(result, "123");
        saw_non_digit |= result.chars().any(|character| !character.is_ascii_digit());
    }

    assert!(
        saw_non_digit,
        "generic string strategy should draw from the alphanumeric charset"
    );
}

/// The generic string strategy draws a random value of roughly the original's
/// length, so for short inputs it can draw the original back. Returning it would
/// leave a source value sitting in the "anonymized" output.
#[test]
fn generic_string_strategy_never_returns_the_original_value() {
    for value in ["a", "7", "ab", "x1"] {
        for _ in 0..512 {
            let result = transform_value(value, &column(DataType::String), &context());
            assert_ne!(result, value, "generic pseudonym echoed the input {value}");
        }
    }
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

/// The risk model and the placeholder must read the same evidence the same way.
///
/// Both call `PrivacyEvidenceSummary::is_actionable`, and this runs the real detector so
/// that dropping either call fails here: if `analyze_column_privacy` stops filtering, the
/// risk assertion fails; if `placeholder_from_evidence` stops filtering, the placeholder
/// assertion does. A hand-built evidence fixture could not catch the first, because it
/// would assert only what I had already decided the evidence was.
#[test]
fn a_column_the_risk_model_leaves_at_low_gets_no_specific_placeholder() {
    use crate::detection::analyze_column_privacy;

    let values: Vec<String> = [
        "please ring 4915550123 tomorrow",
        "call 4915550124 about the order",
        "ring 4915550125 after noon",
    ]
    .iter()
    .map(ToString::to_string)
    .collect();

    let analysis = analyze_column_privacy("note", 0, &values, DataType::Enum, Confidence::High);

    assert!(
        !analysis.evidence.is_empty(),
        "the fixture must still produce evidence, or neither half of this test means anything"
    );
    assert!(
        analysis
            .evidence
            .iter()
            .all(|summary| summary.confidence == Confidence::Low),
        "the fixture must stay Low-only to exercise the all-Low case: {:?}",
        analysis.evidence
    );
    assert_eq!(
        analysis.pii_risk,
        PiiRisk::Low,
        "Low-only evidence must not raise the column's risk"
    );

    let mut column = column(DataType::Enum);
    column.strategy = AnonymizationStrategy::Redact;
    column.pii_risk = analysis.pii_risk;
    column.privacy_evidence = analysis.evidence;

    assert_eq!(
        transform_value(&values[0], &column, &context()),
        "[REDACTED]",
        "the placeholder must not claim a type the risk model declined to trust"
    );
}

/// The filter must not swallow evidence the app does act on.
///
/// Identical fixtures apart from `confidence`, so this isolates that field as the thing
/// deciding the outcome. Without it, a filter that had collapsed every specific
/// placeholder to `[REDACTED]` would still look correct.
#[test]
fn redact_names_a_placeholder_only_on_evidence_the_risk_model_trusts() {
    let phone_span_evidence = |confidence| {
        vec![PrivacyEvidenceSummary {
            kind: PrivacyFindingKind::Contact,
            data_type: DataType::Phone,
            confidence,
            match_count: 1,
            sample_count: 1,
            score: 55,
            detector: "pattern:phone-digits".to_string(),
            reason: "Digit run resembles a phone number.".to_string(),
            detectors: vec!["pattern:phone-digits".to_string()],
        }]
    };

    let mut low_only = column(DataType::Enum);
    low_only.strategy = AnonymizationStrategy::Redact;
    low_only.privacy_evidence = phone_span_evidence(Confidence::Low);
    assert_eq!(
        transform_value("please ring 4915550123 tomorrow", &low_only, &context()),
        "[REDACTED]",
        "a Low-only column must not claim the cell held a phone number"
    );

    let mut medium = column(DataType::Enum);
    medium.strategy = AnonymizationStrategy::Redact;
    medium.privacy_evidence = phone_span_evidence(Confidence::Medium);
    assert_eq!(
        transform_value("please ring 4915550123 tomorrow", &medium, &context()),
        "[PHONE]",
        "raising the same evidence to Medium must still name the specific placeholder"
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

/// A rejected Local AI candidate on a pass-through type is replaced, never released.
///
/// The leak this closes: `Enum`, `CountryCode`, `Boolean`, `Currency` and `Percentage`
/// are closed value domains, which is precisely what makes the smart-replacement leak
/// guard refuse nearly every candidate — a realistic replacement for one row is another
/// row's real value. The refused value then met the shared pass-through gate and was
/// written out verbatim, so a column the user had asked to anonymize was copied
/// through at close to a 100% rate.
#[test]
fn a_rejected_local_ai_value_is_replaced_even_on_a_pass_through_type() {
    for detected_type in [
        DataType::Enum,
        DataType::CountryCode,
        DataType::Boolean,
        DataType::Currency,
        DataType::Percentage,
    ] {
        let mut subject = column(detected_type);
        subject.strategy = AnonymizationStrategy::LocalAi;
        // An empty replacement map is the same state a rejected candidate leaves:
        // `smart_replacement` finds nothing and the transform takes the fallback.
        let mut state = TransformState::new();

        let result = transform_value_with_state("Netherlands", &subject, &context(), &mut state);

        assert_ne!(
            result, "Netherlands",
            "{detected_type:?} released the source value on the Local AI fallback"
        );
        assert_eq!(state.report().smart_replacement_fallbacks, 1);
    }
}

/// The exemption above is scoped to Local AI and nothing else.
///
/// Pass-through for closed domains is a deliberate utility choice — swapping `NL` for
/// another country code buys no privacy and destroys the column — so a user who chose
/// Auto or Pseudonymize must see no change from the Local AI fix.
#[test]
fn the_local_ai_exemption_does_not_reach_the_other_strategies() {
    for detected_type in [
        DataType::Enum,
        DataType::CountryCode,
        DataType::Boolean,
        DataType::Currency,
        DataType::Percentage,
    ] {
        for strategy in [
            AnonymizationStrategy::Auto,
            AnonymizationStrategy::Pseudonymize,
        ] {
            let mut subject = column(detected_type);
            subject.strategy = strategy;
            assert_eq!(
                transform_value("Netherlands", &subject, &context()),
                "Netherlands",
                "{strategy:?} on {detected_type:?} stopped passing through"
            );
        }
    }
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

/// The Local AI replacement map and the pseudonym maps key source values
/// through the same `value_identity_key`. They hold separate key spaces and never
/// query each other, so a divergence would not cause a cross-map miss — each map
/// would simply stop recognizing its own values as repeats, handing out a second
/// replacement for a value it has already seen. That is the quieter failure and
/// the reason both halves are pinned here rather than only the shared helper: the
/// helper being right is no use if a map stops routing its lookups through it.
#[test]
fn replacement_lookup_ignores_case_and_padding_on_both_map_kinds() {
    let mut local_ai_column = column(DataType::FullName);
    local_ai_column.strategy = AnonymizationStrategy::LocalAi;
    let mut replacements = SmartReplacementMap::default();
    replacements.insert(0, "Alice Smith", "Maya Carter");
    let mut state = TransformState::with_smart_replacements(replacements);
    let context = context();

    let result =
        transform_value_with_state("  alice SMITH  ", &local_ai_column, &context, &mut state);

    assert_eq!(result, "Maya Carter");
    assert_eq!(state.report().smart_replacement_fallbacks, 0);

    let name_column = column(DataType::FullName);
    let mut state = TransformState::new();
    let first = transform_value_with_state("Alice Smith", &name_column, &context, &mut state);
    let second = transform_value_with_state("  alice SMITH  ", &name_column, &context, &mut state);

    assert_eq!(first, second);
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
fn a_cell_past_the_metadata_is_blanked_rather_than_released() {
    // `csv_io` refuses a row this shape, so it is unreachable through the app. This
    // function is `pub`, so it is reachable from outside the crate, and returning the
    // original here published a raw value that no strategy chose and no privacy figure
    // counted.
    let columns = vec![column(DataType::Email)];
    let mut state = TransformState::new();
    let row = transform_row_with_state(
        &[
            "john.doe@example.com".to_string(),
            "secret".to_string(),
            "0612345678".to_string(),
        ],
        &columns,
        0,
        &mut state,
    );
    // The row keeps its length, so a caller writing it out gets the arity it handed in.
    assert_eq!(row.len(), 3);
    assert_ne!(row[0], "john.doe@example.com");
    assert_eq!(row[1], "");
    assert_eq!(row[2], "");
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

/// The ledger exists to answer two questions about one column, so both are pinned
/// here: which ordinal a value owns, and how often it appeared. An ordinal that
/// moved between rows would rename a value mid-file; an occurrence count that
/// missed repeats would understate the distribution a consistent pseudonym leaks.
#[test]
fn the_ledger_gives_each_distinct_value_one_stable_ordinal() {
    let mut state = TransformState::new();

    assert_eq!(state.record_pseudonymized_value(0, "alpha"), 1);
    assert_eq!(state.record_pseudonymized_value(0, "beta"), 2);
    assert_eq!(state.record_pseudonymized_value(0, "alpha"), 1);
    assert_eq!(state.record_pseudonymized_value(0, "gamma"), 3);

    let stats = state.report().column_value_distributions;
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].distinct_values, 3);
    assert_eq!(stats[0].total_values, 4);
    // `alpha` appeared twice, so only `beta` and `gamma` single a row out.
    assert_eq!(stats[0].singleton_values, 2);
}

/// The same value in two columns is two independent facts, so each column counts
/// from 1. Sharing one counter would make `[NOTES_7]` in a two-value column, which
/// both misreports the ordinal and leaks the other column's cardinality.
#[test]
fn the_ledger_scopes_ordinals_and_counts_per_column() {
    let mut state = TransformState::new();

    assert_eq!(state.record_pseudonymized_value(0, "shared"), 1);
    assert_eq!(state.record_pseudonymized_value(1, "shared"), 1);
    assert_eq!(state.record_pseudonymized_value(1, "other"), 2);

    let stats = state.report().column_value_distributions;
    assert_eq!(stats.len(), 2);
    assert_eq!(stats[0].column_index, 0);
    assert_eq!(stats[0].distinct_values, 1);
    assert_eq!(stats[1].column_index, 1);
    assert_eq!(stats[1].distinct_values, 2);
}

/// Values differing only in surrounding space or case are one value, matching the
/// `value_identity_key` rule every other per-run map already follows. A ledger that
/// disagreed would count `Alice` and `  alice ` as two distinct values and hand
/// them two ordinals, while the pseudonym maps correctly gave them one output.
#[test]
fn the_ledger_folds_values_the_pseudonym_maps_treat_as_equal() {
    let mut state = TransformState::new();

    assert_eq!(state.record_pseudonymized_value(0, "Alice Smith"), 1);
    assert_eq!(state.record_pseudonymized_value(0, "  alice SMITH  "), 1);

    let stats = state.report().column_value_distributions;
    assert_eq!(stats[0].distinct_values, 1);
    assert_eq!(stats[0].total_values, 2);
}

/// The cardinality report is only meaningful for output that preserves equality.
/// Mask rewrites every value independently, redact collapses the column to one
/// token, and pass-through does not transform at all — none exposes a distribution,
/// so a ledger entry for any of them would warn about a risk the column lacks.
#[test]
fn strategies_that_expose_no_distribution_record_nothing() {
    for strategy in [
        AnonymizationStrategy::Mask,
        AnonymizationStrategy::Redact,
        AnonymizationStrategy::PassThrough,
    ] {
        let mut subject = column(DataType::Email);
        subject.strategy = strategy;
        let mut state = TransformState::new();

        transform_value_with_state("alice@example.com", &subject, &context(), &mut state);

        assert!(
            state.report().column_value_distributions.is_empty(),
            "{strategy:?} recorded a ledger entry"
        );
    }
}

/// A pass-through *type* under a pseudonymizing strategy returns the value
/// unchanged, so it leaks no mapping either. This is the case the early return in
/// `transform_value_with_state` covers, and it is easy to regress by recording
/// before that check rather than after it.
#[test]
fn a_pass_through_type_records_nothing_even_under_pseudonymize() {
    let mut subject = column(DataType::CountryCode);
    subject.strategy = AnonymizationStrategy::Pseudonymize;
    let mut state = TransformState::new();

    let result = transform_value_with_state("US", &subject, &context(), &mut state);

    assert_eq!(result, "US");
    assert!(state.report().column_value_distributions.is_empty());
}

/// Every consistent-pseudonym path records exactly one entry per value, including
/// the two that return before reaching the shared recording point.
#[test]
fn each_consistent_pseudonym_path_records_one_entry_per_value() {
    let mut auto_column = column(DataType::Email);
    auto_column.strategy = AnonymizationStrategy::Auto;
    let mut state = TransformState::new();
    transform_value_with_state("alice@example.com", &auto_column, &context(), &mut state);
    transform_value_with_state("alice@example.com", &auto_column, &context(), &mut state);
    let stats = state.report().column_value_distributions;
    assert_eq!(stats[0].distinct_values, 1, "auto");
    assert_eq!(stats[0].total_values, 2, "auto");

    let mut token_column = column(DataType::Email);
    token_column.strategy = AnonymizationStrategy::Tokenize;
    let mut state = TransformState::new();
    transform_value_with_state("alice@example.com", &token_column, &context(), &mut state);
    let stats = state.report().column_value_distributions;
    assert_eq!(stats[0].distinct_values, 1, "tokenize");
    assert_eq!(stats[0].total_values, 1, "tokenize");

    // Local AI, replacement found: returns before the shared recording point.
    let mut local_ai_column = column(DataType::FullName);
    local_ai_column.strategy = AnonymizationStrategy::LocalAi;
    let mut replacements = SmartReplacementMap::default();
    replacements.insert(0, "Alice Smith", "Maya Carter");
    let mut state = TransformState::with_smart_replacements(replacements);
    transform_value_with_state("Alice Smith", &local_ai_column, &context(), &mut state);
    let stats = state.report().column_value_distributions;
    assert_eq!(stats[0].total_values, 1, "local ai hit");

    // Local AI, no replacement: falls through to the shared point, still once.
    let mut state = TransformState::new();
    transform_value_with_state("Alice Smith", &local_ai_column, &context(), &mut state);
    let stats = state.report().column_value_distributions;
    assert_eq!(stats[0].total_values, 1, "local ai fallback");
    assert_eq!(state.report().smart_replacement_fallbacks, 1);
}

#[test]
fn label_strategy_names_the_column_and_numbers_distinct_values() {
    let mut labelled = column(DataType::Unknown);
    labelled.name = "Customer Notes".to_string();
    labelled.strategy = AnonymizationStrategy::Label;
    let mut state = TransformState::new();
    let context = context();

    let first = transform_value_with_state("first note", &labelled, &context, &mut state);
    let second = transform_value_with_state("second note", &labelled, &context, &mut state);
    let first_again = transform_value_with_state("first note", &labelled, &context, &mut state);

    assert_eq!(first, "[CUSTOMER_NOTES_1]");
    assert_eq!(second, "[CUSTOMER_NOTES_2]");
    // The whole point of the ordinal: a repeated value is visibly the same value.
    assert_eq!(first_again, "[CUSTOMER_NOTES_1]");
}

/// Separator runs collapse to one underscore and the label never starts or ends
/// with one, so `  order // id ` reads as `ORDER_ID` rather than `__ORDER___ID_`.
#[test]
fn label_normalizes_separator_runs_and_edges() {
    for (header, expected) in [
        ("order // id", "[ORDER_ID_1]"),
        ("  padded  ", "[PADDED_1]"),
        ("snake_case_name", "[SNAKE_CASE_NAME_1]"),
        ("dotted.header.v2", "[DOTTED_HEADER_V2_1]"),
        ("MiXeD CaSe", "[MIXED_CASE_1]"),
    ] {
        let mut labelled = column(DataType::Unknown);
        labelled.name = header.to_string();
        labelled.strategy = AnonymizationStrategy::Label;
        let mut state = TransformState::new();

        let result = transform_value_with_state("value", &labelled, &context(), &mut state);

        assert_eq!(result, expected, "header {header:?}");
    }
}

/// Non-ASCII headers keep their letters rather than being punched full of
/// underscores. A Dutch or French header is an ordinary case, not an edge case.
#[test]
fn label_keeps_non_ascii_letters() {
    let mut labelled = column(DataType::Unknown);
    labelled.name = "geboortedatum ouder-é".to_string();
    labelled.strategy = AnonymizationStrategy::Label;
    let mut state = TransformState::new();

    let result = transform_value_with_state("value", &labelled, &context(), &mut state);

    assert_eq!(result, "[GEBOORTEDATUM_OUDER_É_1]");
}

/// A header contributing no letters or digits still has to produce a usable label,
/// and the column's position is the one identifier that always exists.
#[test]
fn label_falls_back_to_the_column_position_when_the_header_is_unusable() {
    for header in ["", "   ", "---", "***"] {
        let mut labelled = column(DataType::Unknown);
        labelled.name = header.to_string();
        labelled.index = 7;
        labelled.strategy = AnonymizationStrategy::Label;
        let mut state = TransformState::new();

        let result = transform_value_with_state("value", &labelled, &context(), &mut state);

        assert_eq!(result, "[COLUMN_7_1]", "header {header:?}");
    }
}

/// A pathological header is capped so the placeholder stays readable in a cell.
#[test]
fn label_caps_a_runaway_header() {
    let mut labelled = column(DataType::Unknown);
    labelled.name = "a".repeat(200);
    labelled.strategy = AnonymizationStrategy::Label;
    let mut state = TransformState::new();

    let result = transform_value_with_state("value", &labelled, &context(), &mut state);

    let label = result.trim_start_matches('[').trim_end_matches("_1]");
    assert_eq!(label.chars().count(), 40);
}

/// The ordinal is per column, so two columns sharing a header produce the same
/// label for different values. This is the documented consequence of scoping
/// identity to `(column, ordinal)` rather than to a document-wide namespace, and it
/// is pinned so the choice cannot be reversed by accident.
#[test]
fn label_ordinals_are_scoped_to_their_column_not_the_document() {
    let mut first_column = column(DataType::Unknown);
    first_column.name = "notes".to_string();
    first_column.index = 0;
    first_column.strategy = AnonymizationStrategy::Label;

    let mut second_column = first_column.clone();
    second_column.name = "remarks".to_string();
    second_column.index = 1;

    let mut state = TransformState::new();
    let context = context();

    let left = transform_value_with_state("alpha", &first_column, &context, &mut state);
    let right = transform_value_with_state("beta", &second_column, &context, &mut state);

    // Both start at 1: the second column does not continue the first one's count.
    assert_eq!(left, "[NOTES_1]");
    assert_eq!(right, "[REMARKS_1]");
}

/// Per-column ordinals are what make a duplicated header dangerous rather than
/// merely untidy: two columns named `notes` would both open at `[NOTES_1]` while
/// holding unrelated values, and a reader comparing those cells would read an
/// equality that was never measured. The position qualifies the label instead.
#[test]
fn duplicate_headers_are_qualified_by_position() {
    let mut first_column = column(DataType::Unknown);
    first_column.name = "notes".to_string();
    first_column.index = 0;
    first_column.strategy = AnonymizationStrategy::Label;
    first_column.header_label_is_ambiguous = true;

    let mut second_column = first_column.clone();
    second_column.index = 1;

    let mut state = TransformState::new();
    let context = context();

    let left = transform_value_with_state("alpha", &first_column, &context, &mut state);
    let right = transform_value_with_state("zulu", &second_column, &context, &mut state);

    assert_eq!(left, "[NOTES_0_1]");
    assert_eq!(right, "[NOTES_1_1]");
    assert_ne!(left, right);
}

/// The qualifier is positional, not a second counter: within one ambiguous column
/// the position stays put while the ordinal advances.
#[test]
fn a_qualified_label_still_numbers_its_own_distinct_values() {
    let mut labelled = column(DataType::Unknown);
    labelled.name = "notes".to_string();
    labelled.index = 3;
    labelled.strategy = AnonymizationStrategy::Label;
    labelled.header_label_is_ambiguous = true;

    let mut state = TransformState::new();
    let context = context();

    assert_eq!(
        transform_value_with_state("alpha", &labelled, &context, &mut state),
        "[NOTES_3_1]"
    );
    assert_eq!(
        transform_value_with_state("beta", &labelled, &context, &mut state),
        "[NOTES_3_2]"
    );
    // Repeats keep their label, qualified or not.
    assert_eq!(
        transform_value_with_state("alpha", &labelled, &context, &mut state),
        "[NOTES_3_1]"
    );
}

/// Label output is derived from the header and the ordinal, never from the value,
/// so unlike the generic pseudonym it cannot *draw* the input back by chance.
///
/// It can still coincide with it, when a cell already holds something
/// placeholder-shaped. That is accepted rather than worked around: the only values
/// affected are ones indistinguishable from a placeholder to begin with, and the
/// alternative — perturbing the label when it happens to match — would break the
/// property the strategy exists for, that one value has one label. The ordinary
/// case is pinned alongside so a regression that echoed real values would fail.
#[test]
fn label_output_comes_from_the_header_not_the_value() {
    let mut labelled = column(DataType::Unknown);
    labelled.name = "x".to_string();
    labelled.strategy = AnonymizationStrategy::Label;
    let mut state = TransformState::new();
    let context = context();

    for value in ["x", "1", "X_1", "some prose"] {
        let result = transform_value_with_state(value, &labelled, &context, &mut state);
        assert_ne!(result, value, "value {value:?}");
    }

    // The documented coincidence, stated as behaviour rather than left to surprise.
    let mut state = TransformState::new();
    assert_eq!(
        transform_value_with_state("[X_1]", &labelled, &context, &mut state),
        "[X_1]"
    );
}

/// Label is a consistent-pseudonym strategy, so it feeds the distribution report
/// like the others — that is what lets a labelled column be warned about.
#[test]
fn label_reports_its_value_distribution() {
    let mut labelled = column(DataType::Unknown);
    labelled.name = "department".to_string();
    labelled.strategy = AnonymizationStrategy::Label;
    let mut state = TransformState::new();
    let context = context();

    for value in ["sales", "sales", "sales", "legal"] {
        transform_value_with_state(value, &labelled, &context, &mut state);
    }

    let distributions = state.report().column_value_distributions;
    assert_eq!(distributions.len(), 1);
    assert_eq!(distributions[0].distinct_values, 2);
    assert_eq!(distributions[0].total_values, 4);
    assert_eq!(distributions[0].singleton_values, 1);
    assert_eq!(distributions[0].max_value_occurrences, 3);
}

/// One source value must get one replacement, whatever padding or case it arrives in.
///
/// This is the app's "repeated source values stay consistent within each run" promise,
/// and it is stated by the privacy report as a *measured* fact — the ledger folds case
/// and padding via `value_identity_key` before counting distinct values. So any
/// transform that folds differently from the ledger does not merely behave oddly: it
/// makes the report assert a consistency the output does not have. Three did.
/// `transform_numeric_id` and `transform_numeric_value` keyed on `value.len()` plus the
/// raw value, `transform_generic_string` keyed on the raw length too, and
/// `transform_email` folded its key correctly but then read the domain off the raw
/// value, carrying the source's trailing spaces into the output cell.
///
/// CSV input cannot reach these paths padded — the reader is configured with
/// `Trim::All` — which is exactly why this needs a test rather than a fixture: the
/// paths that can are the JSON, XML and YAML scalar ones, and they are not where
/// anyone looks for a numeric-pseudonym bug.
///
/// Each case is a type whose transform reads the value itself to build its replacement,
/// so a fold applied in one place and not the other shows up as two different outputs.
/// The ledger's own count is asserted alongside, because the two agreeing is the point
/// — a test on the outputs alone would still pass if both sides drifted together.
#[test]
fn one_source_value_gets_one_replacement_however_it_is_padded() {
    let cases: [(DataType, &str); 6] = [
        (DataType::NumericId, "4815162342"),
        (DataType::Email, "Maya.Carter@example.com"),
        (DataType::Uuid, "7f3d2b1a-4c5e-4f6a-8b9c-0d1e2f3a4b5c"),
        (DataType::Phone, "+31 6 1234 5678"),
        (DataType::FullName, "Maya Carter"),
        (DataType::String, "escalated to billing"),
    ];

    for (data_type, value) in cases {
        let mut subject = column(data_type);
        subject.strategy = AnonymizationStrategy::Pseudonymize;
        let mut state = TransformState::default();

        let plain = transform_value_with_state(value, &subject, &context(), &mut state);
        let padded =
            transform_value_with_state(&format!("  {value}  "), &subject, &context(), &mut state);

        assert_eq!(
            plain, padded,
            "{data_type:?} gave one source value two different replacements"
        );
        let distributions = state.report().column_value_distributions;
        assert_eq!(
            distributions.first().map(|entry| entry.distinct_values),
            Some(1),
            "{data_type:?}: the ledger disagrees with the mapping about how many \
             distinct values it saw"
        );
    }
}

/// A pseudonymized cell must never be the value it replaced.
///
/// `transform_generic_string_candidate` draws from a mixed-case charset and guards
/// against redrawing its input, which is what makes the invariant hold for the short
/// values where an exact redraw is likely at all. Folding the transform's key onto
/// `value_identity_key` moved the guard's comparison onto the *folded* value, so a
/// source value carrying any uppercase stopped being compared against: the draw `A`
/// was tested against `a`, passed, and was returned as the anonymization of `A`.
///
/// Measured at about one row in 122 for a single-character column before the guard was
/// folded to match — which is why this is a loop rather than one call. A single draw
/// asserts almost nothing here: the failure is probabilistic, and the test has to be
/// able to see a rate rather than an event.
///
/// A fresh state per draw on purpose. The mapping is memoized, so one state would
/// sample a single draw as many times as the loop runs and pass on any value that
/// happened to draw well the first time.
///
/// Single characters, then two, because both the draw length and the redraw both scale
/// with the input: at four characters an exact redraw is roughly one in sixteen million
/// and a loop this size would not distinguish a working guard from a missing one.
#[test]
fn a_pseudonymized_generic_string_is_never_its_own_source_value() {
    for value in ["A", "y", "N/", "Ab"] {
        let mut subject = column(DataType::String);
        subject.strategy = AnonymizationStrategy::Pseudonymize;

        for _ in 0..50_000 {
            let mut state = TransformState::default();
            let output = transform_value_with_state(value, &subject, &context(), &mut state);
            assert_ne!(
                output, value,
                "{value:?} was returned as its own anonymization"
            );
        }
    }
}
