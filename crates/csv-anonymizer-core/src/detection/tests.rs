use super::*;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

#[test]
fn detects_email_with_high_confidence() {
    let result = detect_column_type(&strings(&[
        "user1@example.com",
        "user2@example.com",
        "user3@example.com",
    ]));
    assert_eq!(result.data_type, DataType::Email);
    assert_eq!(result.confidence, Confidence::High);
}

#[test]
fn long_numeric_strings_are_numeric_ids_not_phone_numbers() {
    let result = detect_column_type(&strings(&[
        "1234567890123",
        "9876543210987",
        "1111222233334",
    ]));
    assert_eq!(result.data_type, DataType::NumericId);
}

#[test]
fn short_numeric_strings_are_numeric_values() {
    let result = detect_column_type(&strings(&["1", "2", "3"]));
    assert_eq!(result.data_type, DataType::NumericValue);
    assert_eq!(result.confidence, Confidence::High);
}

#[test]
fn decimal_and_signed_numeric_strings_are_numeric_values() {
    let result = detect_column_type(&strings(&["-12.50", "0.00", "42.75"]));
    assert_eq!(result.data_type, DataType::NumericValue);
}

#[test]
fn four_digit_padded_values_are_numeric_ids() {
    let result = detect_column_type(&strings(&["0001", "0002", "0010"]));
    assert_eq!(result.data_type, DataType::NumericId);
}

#[test]
fn short_identifier_columns_are_numeric_ids_from_header_context() {
    let result = detect_column_type_with_name("customer_id", &strings(&["1", "2", "3"]));
    assert_eq!(result.data_type, DataType::NumericId);
    assert_eq!(result.confidence, Confidence::High);
}

#[test]
fn detects_formatted_phone_numbers() {
    let result = detect_column_type(&strings(&[
        "(555) 123-4567",
        "555-867-5309",
        "+1 555 234 9876",
    ]));
    assert_eq!(result.data_type, DataType::Phone);
}

#[test]
fn detects_network_and_web_identifiers() {
    assert_eq!(
        detect_column_type(&strings(&["192.168.1.1", "10.0.0.2", "172.16.0.3"])).data_type,
        DataType::IpAddress
    );
    assert_eq!(
        detect_column_type(&strings(&[
            "https://example.com",
            "http://test.local/path",
            "www.example.org"
        ]))
        .data_type,
        DataType::Url
    );
    assert_eq!(
        detect_column_type(&strings(&[
            "00:1A:2B:3C:4D:5E",
            "00-1A-2B-3C-4D-5F",
            "aa:bb:cc:dd:ee:ff"
        ]))
        .data_type,
        DataType::MacAddress
    );
}

#[test]
fn detects_tax_boolean_currency_and_percentage_values() {
    assert_eq!(
        detect_column_type(&strings(&["123-45-6789", "987-65-4321", "111-22-3333"])).data_type,
        DataType::TaxId
    );
    assert_eq!(
        detect_column_type(&strings(&["12-3456789", "98-7654321", "11-1223333"])).data_type,
        DataType::TaxId
    );
    assert_eq!(
        detect_column_type_with_name("ssn", &strings(&["123456789", "987654321", "111223333"]))
            .data_type,
        DataType::TaxId
    );
    assert_eq!(
        detect_column_type_with_name("ein", &strings(&["123456789", "987654321", "111223333"]))
            .data_type,
        DataType::TaxId
    );
    assert_eq!(
        detect_column_type(&strings(&["true", "false", "yes", "no"])).data_type,
        DataType::Boolean
    );
    assert_eq!(
        detect_column_type(&strings(&["$12.50", "$1,200.00", "$1200.00"])).data_type,
        DataType::Currency
    );
    assert_eq!(
        detect_column_type(&strings(&["12%", "-5.5%", "+100%"])).data_type,
        DataType::Percentage
    );
}

#[test]
fn detects_postal_code_and_address_from_header_context() {
    assert_eq!(
        detect_column_type_with_name("zip_code", &strings(&["94105", "10001", "SW1A 1AA"]))
            .data_type,
        DataType::PostalCode
    );
    assert_eq!(
        detect_column_type_with_name(
            "street_address",
            &strings(&["123 Main St", "44 Market Road", "9 Sunset Ave"]),
        )
        .data_type,
        DataType::Address
    );
}

#[test]
fn detects_nested_zip_code_before_numeric_id() {
    let result = detect_column_type_with_name("address.zipCode", &strings(&["81711"]));

    assert_eq!(result.data_type, DataType::PostalCode);
}

#[test]
fn detects_phone_number_from_header_context() {
    let result = detect_column_type_with_name("phoneNumber", &strings(&["+1-555-0123"]));

    assert_eq!(result.data_type, DataType::Phone);
    assert_eq!(result.confidence, Confidence::High);
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
fn avoids_phone_false_positive_for_unrelated_phone_suffix_headers() {
    let result = detect_column_type_with_name("headphone", &strings(&["1234567"]));

    assert_ne!(result.data_type, DataType::Phone);
}

#[test]
fn invalid_ipv4_values_fall_back_to_numeric_or_string_detection() {
    let result = detect_column_type(&strings(&["999.168.1.1", "10.0.0.999", "172.16.0.3"]));
    assert_ne!(result.data_type, DataType::IpAddress);
}

#[test]
fn detects_enum_after_patterns() {
    let result = detect_column_type(&strings(&[
        "active", "inactive", "pending", "active", "inactive", "pending", "active", "inactive",
        "pending", "active", "inactive",
    ]));
    assert_eq!(result.data_type, DataType::Enum);
}

#[test]
fn detects_mixed_empty_format() {
    let result = detect_empty_format(&strings(&["", "null", "value"]));
    assert_eq!(result, EmptyFormat::Mixed);
}

#[test]
fn only_empty_and_null_are_ignored_for_detection() {
    let result = detect_column_type(&strings(&["", "null", "user@example.com"]));
    assert_eq!(result.data_type, DataType::Email);
    assert_eq!(result.sample_matches, 1);
    assert_eq!(result.total_samples, 3);
}

#[test]
fn value_only_names_remain_strings() {
    let result = detect_column_type(&strings(&["Alice", "Bob", "Carol"]));
    assert_eq!(result.data_type, DataType::String);
}

#[test]
fn generic_name_header_with_single_names_detects_first_name() {
    let result = detect_column_type_with_name("name", &strings(&["Alice", "Bob", "Carol"]));
    assert_eq!(result.data_type, DataType::FirstName);
    assert_eq!(result.confidence, Confidence::High);
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
                && span.value == "4111 1111 1111 1111")
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
