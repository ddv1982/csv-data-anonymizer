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
    ];
    let samples = vec![
        vec![
            "Alice".to_string(),
            "Smith".to_string(),
            "Alice Smith".to_string(),
        ],
        vec![
            "Bob".to_string(),
            "Jones".to_string(),
            "Bob Jones".to_string(),
        ],
        vec![
            "Carol".to_string(),
            "O'Neil".to_string(),
            "Carol O'Neil".to_string(),
        ],
    ];

    let metadata = build_column_metadata(&headers, &samples);

    assert_eq!(metadata[0].detected_type, DataType::FirstName);
    assert_eq!(metadata[1].detected_type, DataType::LastName);
    assert_eq!(metadata[2].detected_type, DataType::FullName);
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
        index: 0,
        detected_type: DataType::Email,
        confidence: crate::types::Confidence::High,
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
