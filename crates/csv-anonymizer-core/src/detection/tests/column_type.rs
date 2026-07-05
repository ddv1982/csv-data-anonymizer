use super::*;

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
fn detects_network_identifiers() {
    assert_eq!(
        detect_column_type(&strings(&["192.168.1.1", "10.0.0.2", "172.16.0.3"])).data_type,
        DataType::IpAddress
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
fn invalid_ipv4_values_fall_back_to_numeric_or_string_detection() {
    let result = detect_column_type(&strings(&["999.168.1.1", "10.0.0.999", "172.16.0.3"]));
    assert_ne!(result.data_type, DataType::IpAddress);
}

#[test]
fn detects_boolean_currency_and_percentage_values() {
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
fn bsn_column_detects_as_tax_id_without_header() {
    let values: Vec<String> = vec!["111222333", "123456782", "111222333", "123456782"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("kolom3", &values);
    assert_eq!(result.data_type, DataType::TaxId);
    assert_eq!(result.confidence, Confidence::High);
}

#[test]
fn national_id_trace_reason_carries_country_label() {
    // A validator:idsmith selection (no agreeing header) labels its trace item
    // with the first matching country (BSN -> NL). Deferred from Task 5.
    let values: Vec<String> = vec!["111222333", "123456782", "111222333", "123456782"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("code", &values);
    assert_eq!(result.data_type, DataType::TaxId);
    let trace = result.trace.expect("trace present");
    assert!(
        trace
            .candidates
            .iter()
            .any(|item| item.reason == "validator:idsmith:NL"),
        "expected an idsmith:NL trace item, got {:?}",
        trace.candidates
    );
}

#[test]
fn checksum_column_beats_contradicting_header() {
    // Header says "code" (benign) but values are valid BSNs: validator wins.
    let values: Vec<String> = vec!["111222333", "123456782", "111222333", "123456782"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("code", &values);
    assert_eq!(result.data_type, DataType::TaxId);
}

#[test]
fn header_agreement_raises_validator_confidence_one_tier() {
    // 3 of 5 valid VAT ids -> Medium by ratio; matching header lifts to High.
    let values: Vec<String> = vec![
        "NL000099998B57",
        "NL000099998B57",
        "NL000099998B57",
        "pending",
        "pending",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let with_header = detect_column_type_with_name("btw nummer", &values);
    let without_header = detect_column_type_with_name("kolom", &values);
    assert_eq!(with_header.data_type, DataType::TaxId);
    assert_eq!(without_header.data_type, DataType::TaxId);
    assert_eq!(without_header.confidence, Confidence::Medium);
    assert_eq!(with_header.confidence, Confidence::High);
}

#[test]
fn header_rules_still_catch_what_values_alone_cannot() {
    // 7-digit local phone format only passes the header-gated fallback shape.
    let values: Vec<String> = vec!["555-0199", "555-0142", "555-0175"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("telefoonnummer", &values);
    assert_eq!(result.data_type, DataType::Phone);
}

#[test]
fn random_numeric_ids_do_not_become_tax_ids() {
    // 4+ digit sequence numbers: near-misses for every checksum scheme.
    let values: Vec<String> = vec!["100001", "100002", "100003", "100004"]
        .into_iter()
        .map(String::from)
        .collect();
    let result = detect_column_type_with_name("id", &values);
    assert_eq!(result.data_type, DataType::NumericId);
}

#[test]
fn sampling_caps_large_columns_but_spans_the_file() {
    // 1000 rows: first 500 emails, last 500 numbers. Even sampling must see both.
    let values: Vec<String> = (0..1000)
        .map(|i| {
            if i < 500 {
                format!("user{i}@example.com")
            } else {
                format!("{i}")
            }
        })
        .collect();
    let result = detect_column_type_with_name("", &values);
    // Neither type reaches the 80% bar on an even sample; must not be High.
    assert_ne!(result.confidence, Confidence::High);
    assert_eq!(result.total_samples, 1000);
    assert!(result.trace.as_ref().unwrap().total_non_empty <= 200);
}

#[test]
fn small_columns_are_scanned_in_full() {
    let values: Vec<String> = (0..50).map(|i| format!("user{i}@example.com")).collect();
    let result = detect_column_type_with_name("", &values);
    assert_eq!(result.data_type, DataType::Email);
    assert_eq!(result.confidence, Confidence::High);
}

#[test]
fn short_columns_get_names_from_header_corroboration() {
    // Name detection is header-gated: a short values-only column does not
    // classify as a name, but the same values under a name header do.
    let values: Vec<String> = vec!["Willem", "Anna", "Pieter"]
        .into_iter()
        .map(String::from)
        .collect();
    let headerless = detect_column_type_with_name("kolom5", &values);
    assert_ne!(headerless.data_type, DataType::FirstName);
    let with_header = detect_column_type_with_name("voornaam", &values);
    assert_eq!(with_header.data_type, DataType::FirstName);
}

#[test]
fn street_address_column_detected_without_header() {
    let values: Vec<String> = vec![
        "Kerkstraat 12",
        "Hoofdweg 3",
        "Dorpsplein 8",
        "Molenlaan 22",
        "Schoolstraat 1",
        "Stationsweg 45",
        "Julianalaan 7",
        "Beatrixstraat 19",
        "Wilhelminaweg 30",
        "Oranjelaan 5",
        "Parkweg 11",
        "Lindenstraat 4",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let result = detect_column_type_with_name("kolom7", &values);
    assert_eq!(result.data_type, DataType::Address);
}

#[test]
fn locale_context_tightens_header_postal_counting() {
    // NL locale context, built the same way build_column_metadata does: from
    // an IBAN column elsewhere in the file.
    let iban_column: Vec<String> = (0..12).map(|_| "NL91ABNA0417164300".to_string()).collect();
    let locale = infer_locale_context(&[iban_column]);
    assert_eq!(locale.countries(), ["NL"]);

    // 8 NL-format postcodes plus 4 values that pass the loose postal shape
    // check (3-12 alphanumeric chars with a digit) but match no country
    // format available in the NL context.
    let values: Vec<String> = [
        "1012 AB", "2511 CV", "3011 ED", "9711 LM", "5611 EM", "6511 KL", "7511 JE", "8011 NW",
        "12345678", "87654321", "11223344", "44332211",
    ]
    .iter()
    .map(|value| value.to_string())
    .collect();

    let result = detect_column_type_in_context("postcode", &values, &locale);
    assert_eq!(result.data_type, DataType::PostalCode);
    // Context-format counting: exactly the 8 NL-format values are counted,
    // not the 12 loose-shape matches, so confidence lands at Medium (8/12)
    // instead of High (12/12).
    assert_eq!(result.sample_matches, 8);
    assert_eq!(result.confidence, Confidence::Medium);
}

#[test]
fn five_digit_sku_column_is_not_postal_without_context() {
    let values: Vec<String> = (10000..10012).map(|n| n.to_string()).collect();
    let result = detect_column_type_with_name("artikel", &values);
    assert_ne!(result.data_type, DataType::PostalCode);
}
