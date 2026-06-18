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
fn detects_formatted_phone_numbers() {
    let result = detect_column_type(&strings(&[
        "(555) 123-4567",
        "555-867-5309",
        "+1 555 234 9876",
    ]));
    assert_eq!(result.data_type, DataType::Phone);
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
fn value_only_names_remain_strings() {
    let result = detect_column_type(&strings(&["Alice", "Bob", "Carol"]));
    assert_eq!(result.data_type, DataType::String);
}
