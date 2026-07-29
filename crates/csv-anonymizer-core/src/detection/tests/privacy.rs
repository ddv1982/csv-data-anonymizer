use super::*;
use crate::types::PrivacyFindingKind;

#[test]
fn vat_detection_adds_specific_privacy_evidence() {
    let prefixed_values = strings(&["NL000099998B57"]);
    let prefixed_analysis = analyze("btw_nummer", &prefixed_values);
    assert!(prefixed_analysis.evidence.iter().any(|summary| {
        summary.kind == PrivacyFindingKind::GovernmentId
            && summary.detectors.contains(&"validator:vat".to_string())
    }));

    let bare_values = strings(&["123456789B01"]);
    let bare_analysis = analyze("btw_nummer", &bare_values);
    assert!(bare_analysis.evidence.iter().any(|summary| {
        summary.kind == PrivacyFindingKind::GovernmentId
            && summary
                .detectors
                .contains(&"pattern:tax-id:nl-btw-tax-number".to_string())
    }));
}

#[test]
fn username_header_adds_account_identifier_evidence() {
    let values = strings(&["johndoe"]);
    let detection = detect_column_type_with_name("username", &values);
    let analysis = analyze_column_privacy(
        "username",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    assert_eq!(detection.data_type, DataType::String);
    assert_eq!(analysis.pii_risk, PiiRisk::High);
    assert!(analysis.evidence.iter().any(|summary| {
        summary.kind == PrivacyFindingKind::AccountOrFinancialId
            && summary.data_type == DataType::String
    }));
}

#[test]
fn private_and_user_event_dates_have_private_date_evidence() {
    let date_of_birth_values = strings(&["1989-07-01"]);
    let date_of_birth_detection =
        detect_column_type_with_name("dateOfBirth", &date_of_birth_values);
    let date_of_birth_analysis = analyze_column_privacy(
        "dateOfBirth",
        0,
        &date_of_birth_values,
        date_of_birth_detection.data_type,
        date_of_birth_detection.confidence,
    );

    assert_eq!(date_of_birth_detection.data_type, DataType::Timestamp);
    assert_eq!(date_of_birth_analysis.pii_risk, PiiRisk::Medium);

    let last_login_values = strings(&["2024-12-15T14:22:00"]);
    let last_login_detection = detect_column_type_with_name("lastLoginAt", &last_login_values);
    let last_login_analysis = analyze_column_privacy(
        "lastLoginAt",
        0,
        &last_login_values,
        last_login_detection.data_type,
        last_login_detection.confidence,
    );

    assert_eq!(last_login_detection.data_type, DataType::Timestamp);
    assert_eq!(last_login_analysis.pii_risk, PiiRisk::Medium);
    assert!(
        last_login_analysis
            .evidence
            .iter()
            .any(|summary| summary.kind == PrivacyFindingKind::PrivateDate)
    );
}

#[test]
fn avoids_private_date_false_positive_for_birth_substrings() {
    let values = strings(&["2024-01-01"]);
    let detection = detect_column_type_with_name("candidateOfBirth", &values);
    let analysis = analyze_column_privacy(
        "candidateOfBirth",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    assert_eq!(detection.data_type, DataType::Timestamp);
    assert_eq!(analysis.pii_risk, PiiRisk::Low);
    assert!(
        !analysis
            .findings
            .iter()
            .any(|finding| finding.detector.starts_with("header:"))
    );
}

#[test]
fn privacy_spans_detect_contact_secret_account_and_network_values() {
    let spans = collect_privacy_spans(
        "email ada@example.com api_key=sk_test_1234567890 card 4111 1111 1111 1111 ip 192.168.1.20",
    );

    assert!(
        spans
            .iter()
            .any(|span| span.kind == PrivacyFindingKind::Contact
                && span.data_type == DataType::Email
                && span.value == "ada@example.com")
    );
    assert!(
        spans
            .iter()
            .any(|span| span.kind == PrivacyFindingKind::CredentialOrSecret
                && span.value == "sk_test_1234567890")
    );
    assert!(
        spans
            .iter()
            .any(|span| span.kind == PrivacyFindingKind::AccountOrFinancialId
                && span.value == "4111 1111 1111 1111"
                && span.detector == "validator:card")
    );
    assert!(
        spans
            .iter()
            .any(|span| span.kind == PrivacyFindingKind::NetworkOrDeviceId
                && span.data_type == DataType::IpAddress
                && span.value == "192.168.1.20")
    );
}

#[test]
fn privacy_spans_do_not_treat_benign_numeric_ids_as_payment_cards() {
    let spans = collect_privacy_spans("order_id=1234567890123 account=1000000000000");

    assert!(
        spans
            .iter()
            .all(|span| span.kind != PrivacyFindingKind::AccountOrFinancialId)
    );
}

#[test]
fn column_privacy_analysis_summarizes_header_and_span_evidence() {
    let values = strings(&[
        "contact ada@example.com",
        "contact grace@example.com",
        "contact alan@example.com",
    ]);
    let detection = detect_column_type_with_name("notes", &values);
    let analysis = analyze_column_privacy(
        "notes",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    assert_eq!(analysis.suggested_data_type, Some(DataType::Email));
    assert_eq!(analysis.pii_risk, PiiRisk::High);
    assert!(
        analysis
            .evidence
            .iter()
            .any(|summary| summary.kind == PrivacyFindingKind::Contact
                && summary.match_count == 3
                && summary.sample_count == 3)
    );
}

#[test]
fn column_privacy_analysis_counts_matched_rows_not_spans() {
    let values = strings(&["primary ada@example.com backup alan@example.com"]);
    let detection = detect_column_type_with_name("notes", &values);
    let analysis = analyze_column_privacy(
        "notes",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    let summary = analysis
        .evidence
        .iter()
        .find(|summary| {
            summary.kind == PrivacyFindingKind::Contact && summary.data_type == DataType::Email
        })
        .expect("email evidence summary");
    assert_eq!(summary.match_count, 1);
    assert_eq!(summary.sample_count, 1);
}

#[test]
fn privacy_findings_use_utf16_offsets_for_frontend_redaction() {
    let values = strings(&["🔒 ada@example.com"]);
    let detection = detect_column_type_with_name("notes", &values);
    let analysis = analyze_column_privacy(
        "notes",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    let finding = analysis
        .findings
        .iter()
        .find(|finding| finding.data_type == DataType::Email)
        .expect("email finding");
    assert_eq!(finding.start, 3);
    assert_eq!(finding.end, 18);
    assert_eq!(finding.match_value, "ada@example.com");
}

#[test]
fn full_cell_privacy_findings_use_utf16_end_offsets() {
    let values = strings(&["Renée"]);
    let analysis = analyze_column_privacy(
        "first_name",
        0,
        &values,
        DataType::FirstName,
        Confidence::High,
    );

    let finding = analysis
        .findings
        .iter()
        .find(|finding| finding.kind == PrivacyFindingKind::Person)
        .expect("person finding");
    assert_eq!(finding.start, 0);
    assert_eq!(finding.end, 5);
}

#[test]
fn low_confidence_date_spans_do_not_raise_default_privacy_risk() {
    let values = strings(&["created 2026-06-29"]);
    let detection = detect_column_type_with_name("event_notes", &values);
    let analysis = analyze_column_privacy(
        "event_notes",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    assert_eq!(analysis.pii_risk, PiiRisk::Low);
    assert!(
        analysis
            .evidence
            .iter()
            .any(|summary| summary.kind == PrivacyFindingKind::PrivateDate
                && summary.confidence == Confidence::Low)
    );
}

/// A bare digit run inside a cell must not talk a column into High risk.
///
/// The inline phone regex accepts an unformatted ten-digit run, so every cell of a
/// notes column full of order references used to carry a High-confidence
/// Contact/phone finding — and `pii_risk` takes the maximum over findings, so the
/// column came out High on nothing but a regex hit. The run is still reported, at
/// Low confidence, which `pii_risk` ignores. A formatted number in the same shape
/// of column still reaches High, which is what keeps this a precision fix rather
/// than a hole.
#[test]
fn unformatted_digit_runs_do_not_raise_a_column_to_high_risk() {
    let unformatted = analyze(
        "notes",
        &strings(&[
            "order reference 4155550100 shipped",
            "order reference 2125550101 shipped",
            "order reference 3125550102 shipped",
        ]),
    );
    assert_eq!(
        unformatted.pii_risk,
        PiiRisk::Low,
        "unformatted runs should not be risk evidence, got {:?}",
        unformatted.evidence
    );

    let formatted = analyze(
        "notes",
        &strings(&[
            "call (415) 555-0100 back",
            "call (212) 555-0101 back",
            "call (312) 555-0102 back",
        ]),
    );
    assert_eq!(
        formatted.pii_risk,
        PiiRisk::High,
        "a formatted number in free text is still contact evidence"
    );
}

/// Phone detection survives the region-sweep gate.
///
/// `can_be_swept_for_phone_region` skips libphonenumber for values whose letters are
/// not an extension suffix, which is what turned 157 seconds of analysis into 25
/// milliseconds. The saving is only worth having if it does not quietly stop finding
/// phone numbers, so the forms that must keep working are pinned here alongside the
/// one form that is deliberately given up.
///
/// The extension rows are the ones this test used to lack. Every case was all-digits
/// or `+`-prefixed, so a gate that rejected *any* letter passed the test while
/// classifying `020 1234567 ext 45` as `String` — Low risk, not auto-selected, copied
/// to the output verbatim. A column of business numbers with direct dials is an
/// ordinary file, not an edge case.
#[test]
fn phone_columns_are_still_detected_after_the_region_sweep_gate() {
    for values in [
        &["0612345678", "0612345679", "0612345670"][..],
        &["4155550100", "2125550101", "3125550102"][..],
        &["+31 6 1234 5678", "+31 6 1234 5679", "+31 6 1234 5670"][..],
        &["(415) 555-0100", "(212) 555-0101", "(312) 555-0102"][..],
        &[
            "020 1234567 ext 45",
            "020 1234568 ext 46",
            "020 1234569 ext 47",
        ][..],
        &[
            "020 1234567 ext. 45",
            "020 1234568 ext. 46",
            "020 1234569 ext. 47",
        ][..],
        &[
            "(415) 234-0100 x89",
            "(212) 234-0101 x90",
            "(312) 234-0102 x91",
        ][..],
        &[
            "020 1234567 extension 4501",
            "020 1234568 extension 4502",
            "020 1234569 extension 4503",
        ][..],
    ] {
        let detection = detect_column_type_with_name("phone", &strings(values));
        assert_eq!(
            detection.data_type,
            DataType::Phone,
            "phone column {values:?} should still be detected"
        );
    }

    // Given up deliberately: a vanity number with no country prefix. Recognizing it
    // meant letting libphonenumber vanity-expand every value in every
    // phone-labeled column, which is the cost that made wide files unusable.
    let vanity = detect_column_type_with_name("phone", &strings(&["1800FLOWERS", "1800CONTACTS"]));
    assert_ne!(vanity.data_type, DataType::Phone);
}

/// A phone-labeled column of prose must not cost a region sweep per value.
///
/// This is the other half of `can_be_swept_for_phone_region`, and the half no
/// correctness test can cover: removing the gate does not change what this column is
/// classified as — prose is `String` either way — it changes how long the app takes to
/// say so. Measured before the gate, 20 such columns took 157 seconds, which is not a
/// slow analysis but a frozen window, and the only thing that would have caught it is
/// a person waiting.
///
/// The budget is a ratio against the same values under a header the phone rule
/// ignores, not a wall-clock figure, because an absolute bound has to be either loose
/// enough to survive a loaded machine or tight enough to detect the sweep, and it
/// cannot be both — a 5-second bound was tried first and the ungated cost slipped
/// under it. Measured here: about 2x with the gate, 28x without (954ms against 34ms).
/// The 8x budget sits between those with room on each side, and the absolute floor
/// keeps a machine fast enough to make both figures tiny from failing on noise.
#[test]
fn phone_labeled_prose_does_not_pay_for_a_region_sweep() {
    let values: Vec<String> = (0..200)
        .map(|n| format!("the customer called about invoice {n} and asked for a callback"))
        .collect();

    let started = std::time::Instant::now();
    let detection = detect_column_type_with_name("phone", &values);
    let with_phone_header = started.elapsed();

    // The same work minus the phone rule, as the machine's own baseline.
    let started = std::time::Instant::now();
    detect_column_type_with_name("comment", &values);
    let without_phone_header = started.elapsed();

    assert_ne!(
        detection.data_type,
        DataType::Phone,
        "prose is not a phone number, whatever the header says"
    );

    let budget = (without_phone_header * 8).max(std::time::Duration::from_millis(50));
    assert!(
        with_phone_header < budget,
        "200 prose values took {with_phone_header:?} under a phone header against \
         {without_phone_header:?} without one. The region sweep is running on values that \
         cannot validate; 20 columns of this shape took 157 seconds before the gate."
    );
}

/// A surrogate key is Medium risk, and both risk sources agree on that.
///
/// They used to disagree: `classify_pii_risk` said Medium while `NumericId` also
/// carried an `AccountOrFinancialId` finding worth High, and since callers combine
/// the two with `max_pii_risk`, every column of order numbers reported as financial
/// data at High risk — the Medium was unreachable. `NumericId` now maps to
/// `RecordIdentifier`, which is Medium, so the two sources say the same thing.
///
/// Nothing is less protected by this: Medium is auto-selected and defaults to
/// Redact exactly as High does, which the assertions below check rather than assume.
/// What changes is the label — the column reads as a record identifier instead of a
/// bank account, and it drops out of the "select high risk" quick action, which is
/// the point of having that action mean something.
#[test]
fn surrogate_key_columns_are_medium_risk_from_both_risk_sources() {
    assert_eq!(classify_pii_risk(DataType::NumericId), PiiRisk::Medium);

    for (header, values) in [
        ("seq", &["1000001", "1000002", "1000003", "1000004"][..]),
        ("order_id", &["1001", "1002", "1003"][..]),
        // Also at Medium detection confidence, so this is not an artefact of a
        // cleanly typed column.
        ("mixed", &["1001", "abc", "1003", "xyz", "1005"][..]),
        // `user id` used to sit in the `account_number` taxonomy kind as well as
        // `numeric_id`, so its header alone produced a financial finding.
        ("user_id", &["1001", "1002", "1003"][..]),
    ] {
        let column_values = strings(values);
        let detection = detect_column_type_with_name(header, &column_values);
        assert_eq!(detection.data_type, DataType::NumericId, "{header}");

        let analysis = analyze_column_privacy(
            header,
            0,
            &column_values,
            detection.data_type,
            detection.confidence,
        );
        let risk = max_pii_risk(classify_pii_risk(detection.data_type), analysis.pii_risk);

        assert_eq!(
            risk,
            PiiRisk::Medium,
            "{header} evidence {:?}",
            analysis.evidence
        );
        assert!(
            !analysis
                .evidence
                .iter()
                .any(|summary| summary.kind == PrivacyFindingKind::AccountOrFinancialId),
            "{header} must not be reported as an account or financial identifier: {:?}",
            analysis.evidence
        );
    }
}

/// The Medium that a surrogate key now gets still protects the column.
///
/// Downgrading `NumericId` from High would be a real loss if Medium meant "leave it
/// alone". It does not: both the auto-selection rule and the default-strategy rule
/// treat Medium and High identically, so the column is still offered and still
/// redacted. This is the assertion that makes the reclassification safe.
#[test]
fn medium_risk_columns_are_still_auto_selected_and_still_redacted() {
    use crate::metadata::{default_strategy_for_pii_risk, should_auto_select_column};
    use crate::types::AnonymizationStrategy;

    let metadata = crate::metadata::build_column_metadata(
        &["order_id".to_string()],
        &[
            vec!["1001".to_string()],
            vec!["1002".to_string()],
            vec!["1003".to_string()],
        ],
    );
    let column = &metadata[0];

    assert_eq!(column.pii_risk, PiiRisk::Medium);
    assert!(should_auto_select_column(column));
    assert_eq!(
        default_strategy_for_pii_risk(column.pii_risk),
        AnonymizationStrategy::Redact
    );
}

/// No `DataType` may be given two different risks by the two sources that assign
/// risk, because callers combine them with `max_pii_risk` and the lower one then
/// simply never applies.
///
/// Three types used to disagree, each hiding a category error:
/// `NumericId` said Medium but claimed an `AccountOrFinancialId` finding, so order
/// numbers reported as financial data; `PostalCode` said Medium but claimed
/// `PrivateAddress`, so a zip column reported as a home address; `FirstName` and
/// `LastName` said Medium while carrying a `Person` finding, which is High and is the
/// right answer for a column of people's names. All three are now consistent — two
/// by correcting the finding kind, one by correcting the mapping.
///
/// This is the test that keeps them that way. A disagreement here is not cosmetic: it
/// means one of the two mappings is dead code, and the dead one is the one a reader
/// is most likely to trust.
#[test]
fn every_data_type_gets_one_consistent_risk_from_both_sources() {
    let mut disagreements = Vec::new();

    for data_type in all_data_types() {
        let Some((kind, _)) = data_type.privacy_finding_kind_and_reason() else {
            // No finding, so the mapping is the only source and cannot conflict.
            continue;
        };
        let from_mapping = classify_pii_risk(data_type);
        let from_finding = crate::detection::privacy::risk_for_privacy_kind_in_tests(kind);

        if from_mapping != from_finding {
            disagreements.push(format!(
                "{data_type:?}: classify_pii_risk says {from_mapping:?} but its {kind:?} \
                 finding says {from_finding:?} (resolved: {:?})",
                max_pii_risk(from_mapping, from_finding)
            ));
        }
    }

    assert!(
        disagreements.is_empty(),
        "the two risk sources disagree, so the weaker answer is unreachable:\n  {}",
        disagreements.join("\n  ")
    );
}

/// Every risk the app can assign has to keep protecting the column.
///
/// Reclassifying a type only stays safe while Medium behaves like High for selection
/// and for the default strategy. If Medium ever became "leave it alone", every
/// Medium reclassification in this file would silently turn into a leak, so the
/// property is asserted rather than assumed.
#[test]
fn medium_risk_protects_a_column_exactly_as_high_does() {
    use crate::metadata::default_strategy_for_pii_risk;
    use crate::types::AnonymizationStrategy;

    for risk in [PiiRisk::High, PiiRisk::Medium] {
        assert_eq!(
            default_strategy_for_pii_risk(risk),
            AnonymizationStrategy::Redact,
            "{risk:?} must still default to redaction"
        );
    }
    assert_eq!(
        default_strategy_for_pii_risk(PiiRisk::Low),
        AnonymizationStrategy::Auto
    );
}

/// Every `DataType`, in declaration order.
///
/// The successor `match` below has no wildcard arm, so adding a variant to `DataType`
/// stops this file compiling until an arm for it is written. That is the whole point of
/// writing it this way: the previous version was a hand-written array, and a hand-written
/// array cannot fail. A 23rd variant would have compiled fine and simply not been walked,
/// so the one test that catches a type whose two risk sources disagree would have skipped
/// exactly the type nobody had checked yet.
///
/// What the compiler guarantees is the arm, not its position: an arm that no other arm
/// points at leaves its variant off the walk, and no dep-free construct can catch that
/// (deriving the iterator would, but that means a runtime dependency on the enum's own
/// declaration for the sake of one test). So chain a new variant in where it is declared,
/// and read the compile error as the instruction to do so rather than as a stray arm to
/// fill in. The `None` arm marks the last variant and the assertion below rejects a chain
/// that revisits a type.
fn all_data_types() -> impl Iterator<Item = DataType> {
    fn next_data_type(current: DataType) -> Option<DataType> {
        // No `_ =>` arm. Adding a variant must break this match.
        match current {
            DataType::Email => Some(DataType::Uuid),
            DataType::Uuid => Some(DataType::Timestamp),
            DataType::Timestamp => Some(DataType::NumericId),
            DataType::NumericId => Some(DataType::NumericValue),
            DataType::NumericValue => Some(DataType::PostalCode),
            DataType::PostalCode => Some(DataType::Address),
            DataType::Address => Some(DataType::IpAddress),
            DataType::IpAddress => Some(DataType::Url),
            DataType::Url => Some(DataType::MacAddress),
            DataType::MacAddress => Some(DataType::TaxId),
            DataType::TaxId => Some(DataType::Boolean),
            DataType::Boolean => Some(DataType::Currency),
            DataType::Currency => Some(DataType::Percentage),
            DataType::Percentage => Some(DataType::CountryCode),
            DataType::CountryCode => Some(DataType::Phone),
            DataType::Phone => Some(DataType::FirstName),
            DataType::FirstName => Some(DataType::LastName),
            DataType::LastName => Some(DataType::FullName),
            DataType::FullName => Some(DataType::Enum),
            DataType::Enum => Some(DataType::String),
            DataType::String => Some(DataType::Unknown),
            DataType::Unknown => None,
        }
    }

    let mut seen = Vec::new();
    std::iter::successors(Some(DataType::Email), |current| next_data_type(*current)).inspect(
        move |&data_type| {
            // A variant chained back into the middle of the sequence would make the
            // walk cycle forever instead of failing, so the repeat is caught here.
            assert!(
                !seen.contains(&data_type),
                "{data_type:?} appears twice in the successor chain, so the walk does not \
                 terminate and cannot cover every type"
            );
            seen.push(data_type);
        },
    )
}
