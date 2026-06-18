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
