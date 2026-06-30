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
