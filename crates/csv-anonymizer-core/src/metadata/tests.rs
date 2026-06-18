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
    }];

    let selected = apply_column_selection(&metadata, &[0]);

    assert!(selected[0].is_selected);
    assert!(!metadata[0].is_selected);
}
