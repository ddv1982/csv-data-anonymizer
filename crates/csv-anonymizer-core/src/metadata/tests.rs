use super::*;
use crate::types::DataType;

#[test]
fn builds_metadata_for_all_columns() {
    let headers = vec!["email".to_string(), "id".to_string(), "country".to_string()];
    let samples = vec![
        vec![
            "john@example.com".to_string(),
            "1001".to_string(),
            "US".to_string(),
        ],
        vec![
            "jane@test.org".to_string(),
            "1002".to_string(),
            "GB".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata.len(), 3);
    assert_eq!(metadata[0].detected_type, DataType::Email);
    assert_eq!(metadata[1].detected_type, DataType::NumericId);
    assert_eq!(metadata[2].detected_type, DataType::CountryCode);
}

#[test]
fn detects_name_types_from_header_context() {
    let headers = vec![
        "first_name".to_string(),
        "last_name".to_string(),
        "full_name".to_string(),
        "name".to_string(),
    ];
    let samples = vec![
        vec![
            "Alice".to_string(),
            "Smith".to_string(),
            "Alice Smith".to_string(),
            "Alice".to_string(),
        ],
        vec![
            "Bob".to_string(),
            "Jones".to_string(),
            "Bob Jones".to_string(),
            "Bob".to_string(),
        ],
        vec![
            "Carol".to_string(),
            "O'Neil".to_string(),
            "Carol O'Neil".to_string(),
            "Carol".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata[0].detected_type, DataType::FirstName);
    assert_eq!(metadata[1].detected_type, DataType::LastName);
    assert_eq!(metadata[2].detected_type, DataType::FullName);
    assert_eq!(metadata[3].detected_type, DataType::FirstName);
}

#[test]
fn does_not_detect_names_without_header_context() {
    let headers = vec!["status".to_string()];
    let samples = vec![
        vec!["Alice".to_string()],
        vec!["Bob".to_string()],
        vec!["Carol".to_string()],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata[0].detected_type, DataType::String);
}

#[test]
fn applies_column_selection_without_mutating_source() {
    let metadata = vec![ColumnMetadata {
        name: "email".to_string(),
        source_path: None,
        index: 0,
        detected_type: DataType::Email,
        confidence: crate::types::Confidence::High,
        detection_trace: None,
        privacy_findings: Vec::new(),
        privacy_evidence: Vec::new(),
        pii_risk: PiiRisk::High,
        sample_values: vec![],
        empty_format: crate::types::EmptyFormat::EmptyString,
        is_selected: false,
        strategy: AnonymizationStrategy::Auto,
    }];

    let selected = apply_column_selection(&metadata, &[0]);

    assert!(selected[0].is_selected);
    assert!(!metadata[0].is_selected);
}

#[test]
fn auto_selection_tracks_current_pii_risk_contract() {
    let headers = vec![
        "email".to_string(),
        "id".to_string(),
        "country".to_string(),
        "status".to_string(),
    ];
    let samples = vec![
        vec![
            "john@example.com".to_string(),
            "1001".to_string(),
            "US".to_string(),
            "active".to_string(),
        ],
        vec![
            "jane@example.com".to_string(),
            "1002".to_string(),
            "GB".to_string(),
            "inactive".to_string(),
        ],
        vec![
            "jo@example.com".to_string(),
            "1003".to_string(),
            "DE".to_string(),
            "pending".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);
    let metadata = auto_select_pii_columns(&metadata);

    assert!(metadata[0].is_selected);
    assert!(metadata[1].is_selected);
    assert!(!metadata[2].is_selected);
    assert!(!metadata[3].is_selected);
}

#[test]
fn should_auto_select_requires_samples_and_detected_risk() {
    let high_risk = column_metadata(PiiRisk::High, vec!["person@example.com".to_string()]);
    let medium_risk = column_metadata(PiiRisk::Medium, vec!["10001".to_string()]);
    let low_risk = column_metadata(PiiRisk::Low, vec!["active".to_string()]);
    let empty_high_risk = column_metadata(PiiRisk::High, vec![]);

    assert!(should_auto_select_column(&high_risk));
    assert!(should_auto_select_column(&medium_risk));
    assert!(!should_auto_select_column(&low_risk));
    assert!(!should_auto_select_column(&empty_high_risk));
}

#[test]
fn default_strategy_redacts_medium_and_high_risk_columns() {
    let headers = vec![
        "email".to_string(),
        "date_of_birth".to_string(),
        "country".to_string(),
        "status".to_string(),
    ];
    let samples = vec![
        vec![
            "john@example.com".to_string(),
            "1980-01-02".to_string(),
            "US".to_string(),
            "active".to_string(),
        ],
        vec![
            "jane@example.com".to_string(),
            "1991-03-04".to_string(),
            "GB".to_string(),
            "inactive".to_string(),
        ],
        vec![
            "jo@example.com".to_string(),
            "1975-05-06".to_string(),
            "DE".to_string(),
            "pending".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata[0].pii_risk, PiiRisk::High);
    assert_eq!(metadata[0].strategy, AnonymizationStrategy::Redact);
    assert_eq!(metadata[1].pii_risk, PiiRisk::Medium);
    assert_eq!(metadata[1].strategy, AnonymizationStrategy::Redact);
    assert_eq!(metadata[2].pii_risk, PiiRisk::Low);
    assert_eq!(metadata[2].strategy, AnonymizationStrategy::Auto);
    assert_eq!(metadata[3].pii_risk, PiiRisk::Low);
    assert_eq!(metadata[3].strategy, AnonymizationStrategy::Auto);
}

fn column_metadata(pii_risk: PiiRisk, sample_values: Vec<String>) -> ColumnMetadata {
    ColumnMetadata {
        name: "field".to_string(),
        source_path: None,
        index: 0,
        detected_type: DataType::String,
        confidence: crate::types::Confidence::High,
        detection_trace: None,
        privacy_findings: Vec::new(),
        privacy_evidence: Vec::new(),
        pii_risk,
        sample_values,
        empty_format: crate::types::EmptyFormat::EmptyString,
        is_selected: false,
        strategy: AnonymizationStrategy::Auto,
    }
}

#[test]
fn auto_selection_includes_sensitive_new_types_only() {
    let headers = vec![
        "ip".to_string(),
        "tax_id".to_string(),
        "zip".to_string(),
        "street_address".to_string(),
        "website".to_string(),
        "mac".to_string(),
        "active".to_string(),
        "price".to_string(),
        "discount".to_string(),
    ];
    let samples = vec![
        vec![
            "192.168.1.1".to_string(),
            "123-45-6789".to_string(),
            "94105".to_string(),
            "123 Main St".to_string(),
            "https://example.com".to_string(),
            "00:1A:2B:3C:4D:5E".to_string(),
            "true".to_string(),
            "$1200.00".to_string(),
            "10%".to_string(),
        ],
        vec![
            "10.0.0.2".to_string(),
            "987-65-4321".to_string(),
            "10001".to_string(),
            "44 Market Road".to_string(),
            "www.example.org".to_string(),
            "00-1A-2B-3C-4D-5F".to_string(),
            "false".to_string(),
            "$999.99".to_string(),
            "25%".to_string(),
        ],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));

    assert_eq!(metadata[0].detected_type, DataType::IpAddress);
    assert_eq!(metadata[1].detected_type, DataType::TaxId);
    assert_eq!(metadata[2].detected_type, DataType::PostalCode);
    assert_eq!(metadata[3].detected_type, DataType::Address);
    assert_eq!(metadata[4].detected_type, DataType::Url);
    assert_eq!(metadata[5].detected_type, DataType::MacAddress);
    assert_eq!(metadata[6].detected_type, DataType::Boolean);
    assert_eq!(metadata[7].detected_type, DataType::Currency);
    assert_eq!(metadata[8].detected_type, DataType::Percentage);
    for column in metadata.iter().take(6) {
        assert!(column.is_selected);
    }
    for column in metadata.iter().take(9).skip(6) {
        assert!(!column.is_selected);
    }
}

#[test]
fn metadata_lifts_embedded_span_findings_into_column_evidence() {
    let headers = vec!["notes".to_string()];
    let samples = vec![
        vec!["contact ada@example.com".to_string()],
        vec!["contact grace@example.com".to_string()],
        vec!["contact alan@example.com".to_string()],
    ];

    let metadata = build_column_metadata(&headers, &samples);
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::String);
    assert_eq!(column.pii_risk, PiiRisk::High);
    assert_eq!(column.strategy, AnonymizationStrategy::Redact);
    assert_eq!(column.privacy_evidence[0].match_count, 3);
    assert_eq!(column.privacy_findings[0].start, "contact ".len());
}

#[test]
fn metadata_adds_header_evidence_for_private_dates_and_secrets() {
    let headers = vec!["date_of_birth".to_string(), "api_token".to_string()];
    let samples = vec![
        vec!["1990-01-01".to_string(), "abc123secret".to_string()],
        vec!["1982-06-29".to_string(), "def456secret".to_string()],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert!(
        metadata[0]
            .privacy_evidence
            .iter()
            .any(
                |summary| summary.kind == crate::types::PrivacyFindingKind::PrivateDate
                    && summary.confidence == crate::types::Confidence::Medium
            )
    );
    assert!(
        metadata[1]
            .privacy_evidence
            .iter()
            .any(
                |summary| summary.kind == crate::types::PrivacyFindingKind::CredentialOrSecret
                    && summary.match_count == 2
            )
    );
}

#[test]
fn metadata_auto_selects_multilingual_pii_columns() {
    let headers = vec![
        "voornaam".to_string(),
        "achternaam".to_string(),
        "teléfono".to_string(),
        "adresse".to_string(),
        "geboortedatum".to_string(),
        "status".to_string(),
    ];
    let samples = vec![
        vec![
            "Renée".to_string(),
            "Jansen".to_string(),
            "+34 612 345 678".to_string(),
            "12 Rue de Rivoli".to_string(),
            "1980-01-02".to_string(),
            "active".to_string(),
        ],
        vec![
            "Søren".to_string(),
            "Müller".to_string(),
            "+34 611 111 111".to_string(),
            "5 Avenue Victor Hugo".to_string(),
            "1991-03-04".to_string(),
            "inactive".to_string(),
        ],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));

    assert_eq!(metadata[0].detected_type, DataType::FirstName);
    assert_eq!(metadata[1].detected_type, DataType::LastName);
    assert_eq!(metadata[2].detected_type, DataType::Phone);
    assert_eq!(metadata[3].detected_type, DataType::Address);
    assert_eq!(metadata[4].detected_type, DataType::Timestamp);
    assert_eq!(metadata[5].detected_type, DataType::String);

    for column in metadata.iter().take(5) {
        assert!(
            column.is_selected,
            "column {} should be selected",
            column.name
        );
    }
    assert!(!metadata[5].is_selected);

    assert!(
        metadata[2]
            .detection_trace
            .as_ref()
            .is_some_and(|trace| trace.selected_reason.contains("Header taxonomy term"))
    );
    assert!(metadata[4].privacy_evidence.iter().any(|evidence| {
        evidence
            .detectors
            .contains(&"header:taxonomy:private-date".to_string())
    }));
}

#[test]
fn metadata_uses_iban_validator_without_english_header_context() {
    let headers = vec!["rekening".to_string()];
    let samples = vec![
        vec!["GB82 WEST 1234 5698 7654 32".to_string()],
        vec!["NL91ABNA0417164300".to_string()],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::String);
    assert_eq!(column.pii_risk, PiiRisk::High);
    assert_eq!(column.strategy, AnonymizationStrategy::Redact);
    assert!(column.is_selected);
    assert!(column.privacy_evidence.iter().any(|evidence| {
        evidence
            .reason
            .contains("IBAN account identifier passed checksum validation")
    }));
}

#[test]
fn metadata_promotes_headerless_vat_values_to_tax_id() {
    let headers = vec!["business_number".to_string()];
    let samples = vec![
        vec!["NL000099998B57".to_string()],
        vec!["DE111111125".to_string()],
        vec!["FR61954506077".to_string()],
    ];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::TaxId);
    assert_eq!(column.pii_risk, PiiRisk::High);
    assert_eq!(column.strategy, AnonymizationStrategy::Redact);
    assert!(column.is_selected);
    assert!(
        column
            .privacy_evidence
            .iter()
            .any(|evidence| evidence.detectors.contains(&"validator:vat".to_string()))
    );
}

#[test]
fn low_confidence_date_evidence_does_not_auto_select_column() {
    let headers = vec!["event_notes".to_string()];
    let samples = vec![vec!["created 2026-06-29".to_string()]];

    let metadata = auto_select_pii_columns(&build_column_metadata(&headers, &samples));
    let column = &metadata[0];

    assert_eq!(column.detected_type, DataType::String);
    assert_eq!(column.pii_risk, PiiRisk::Low);
    assert!(!column.is_selected);
    assert!(column.privacy_evidence.iter().any(|summary| summary.kind
        == crate::types::PrivacyFindingKind::PrivateDate
        && summary.confidence == crate::types::Confidence::Low));
}
