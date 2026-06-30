use super::*;
use crate::types::PrivacyFindingKind;

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
fn formatted_numeric_codes_are_not_headerless_phone_numbers() {
    let result = detect_column_type(&strings(&["2024-06-1234", "2025-07-5678"]));

    assert_ne!(result.data_type, DataType::Phone);
}

#[test]
fn detects_url_values() {
    assert_eq!(
        detect_column_type(&strings(&[
            "https://example.com",
            "http://test.local/path",
            "www.example.org"
        ]))
        .data_type,
        DataType::Url
    );
}

#[test]
fn parser_validators_handle_email_url_and_card_boundaries() {
    assert!(is_email("ada.lovelace+test@example.co.uk"));
    assert!(!is_email("ada@localhost"));
    assert!(!is_email("Ada <ada@example.com>"));
    assert!(!is_email("ada@[127.0.0.1]"));

    assert!(is_url("https://example.com/path?q=1"));
    assert!(is_url("www.example.org/report"));
    assert!(!is_url("ftp://example.com/file.csv"));
    assert!(!is_url("https://exa mple.com"));
    assert!(!is_url("mailto:ada@example.com"));

    assert!(is_payment_card_number("4111111111111111"));
    assert!(!is_payment_card_number("1000000000000"));
    assert!(!is_payment_card_number("1234567890123"));

    assert!(is_tax_id("123-45-6789"));
    assert!(is_tax_id("12-3456789"));
    assert!(!is_tax_id("000-00-0000"));
    assert!(!is_tax_id("00-1234567"));
}

#[test]
fn detects_tax_values() {
    assert_eq!(
        detect_column_type(&strings(&["123-45-6789", "987-65-4321", "111-22-3333"])).data_type,
        DataType::TaxId
    );
    assert_eq!(
        detect_column_type(&strings(&["12-3456789", "98-7654321", "11-1223333"])).data_type,
        DataType::TaxId
    );
    assert_eq!(
        detect_column_type_with_name("ssn", &strings(&["123456789", "234567890", "345678901"]))
            .data_type,
        DataType::TaxId
    );
    assert_eq!(
        detect_column_type_with_name("ein", &strings(&["123456789", "987654321", "111223333"]))
            .data_type,
        DataType::TaxId
    );
}

#[test]
fn invalid_us_tax_ids_do_not_match_shape_only() {
    assert_ne!(
        detect_column_type_with_name(
            "ssn",
            &strings(&["000-00-0000", "666-00-0000", "900-00-0000"])
        )
        .data_type,
        DataType::TaxId
    );
    assert_ne!(
        detect_column_type_with_name("ein", &strings(&["00-1234567", "07-1234567", "96-1234567"]))
            .data_type,
        DataType::TaxId
    );
    assert_ne!(
        detect_column_type(&strings(&["000-00-0000", "00-1234567"])).data_type,
        DataType::TaxId
    );
}

#[test]
fn validates_iban_values_as_account_identifier_evidence() {
    let values = strings(&[
        "gb82 west 1234 5698 7654 32",
        "GB82\u{00A0}WEST\u{00A0}1234\u{00A0}5698\u{00A0}7654\u{00A0}32",
        "NL91ABNA0417164300",
    ]);
    let detection = detect_column_type_with_name("rekening", &values);
    let analysis = analyze_column_privacy(
        "rekening",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    assert_eq!(detection.data_type, DataType::String);
    assert_eq!(detection.confidence, Confidence::High);
    assert_eq!(analysis.pii_risk, PiiRisk::High);
    assert!(analysis.evidence.iter().any(|summary| summary.kind
        == PrivacyFindingKind::AccountOrFinancialId
        && summary.confidence == Confidence::High
        && summary.detector == "validator:iban"
        && summary.detectors.contains(&"validator:iban".to_string())));
}

#[test]
fn rejects_invalid_iban_near_misses() {
    let values = strings(&["GB82 WEST 1234 5698 7654 33", "GB82 WEST 1234"]);
    let detection = detect_column_type_with_name("notes", &values);
    let analysis = analyze_column_privacy(
        "notes",
        0,
        &values,
        detection.data_type,
        detection.confidence,
    );

    assert_eq!(detection.data_type, DataType::String);
    assert_ne!(analysis.pii_risk, PiiRisk::High);
    assert!(!analysis.evidence.iter().any(|summary| {
        summary.kind == PrivacyFindingKind::AccountOrFinancialId
            && summary.detector == "validator:iban"
    }));
}

#[test]
fn stdnum_vat_fixture_cases_are_enforced() {
    let fixtures = stdnum_vat_fixtures();

    for case in fixtures.valid_vat_ids {
        assert!(
            is_vat_id(&case.value),
            "valid {} VAT {}",
            case.country,
            case.value
        );
    }

    for case in fixtures.invalid_vat_ids {
        assert!(
            !is_vat_id(&case.value),
            "invalid {} VAT {}",
            case.country,
            case.value
        );
    }

    for value in fixtures.valid_dutch_btw_tax_numbers {
        assert!(
            is_dutch_btw_tax_number(&value),
            "Dutch BTW tax number {value}"
        );
        assert!(
            !is_vat_id(&value),
            "bare Dutch BTW tax number is not a prefixed VAT ID"
        );
    }
}

#[test]
fn detects_checksum_valid_prefixed_vat_values_as_tax_ids() {
    for header in ["btw_nummer", "BTW", "vat_number"] {
        let result = detect_column_type_with_name(
            header,
            &strings(&["NL000099998B57", "DE111111125", "FR61954506077"]),
        );

        assert_eq!(result.data_type, DataType::TaxId, "header {header}");
        assert_eq!(result.confidence, Confidence::High, "header {header}");
    }

    let headerless = detect_column_type(&strings(&[
        "NL000099998B57",
        "DE111111125",
        "FR61954506077",
    ]));
    assert_eq!(headerless.data_type, DataType::TaxId);
    assert_eq!(headerless.confidence, Confidence::High);
}

#[test]
fn detects_dutch_btw_tax_numbers_without_country_prefix() {
    for header in ["btw_nummer", "BTW", "omzetbelastingnummer"] {
        let result = detect_column_type_with_name(
            header,
            &strings(&["123456789B01", "111234567B01", "004495445B01"]),
        );

        assert_eq!(result.data_type, DataType::TaxId, "header {header}");
        assert_eq!(result.confidence, Confidence::High, "header {header}");
    }
}

#[test]
fn dutch_btw_tax_numbers_require_valid_b_suffix_range() {
    assert!(is_dutch_btw_tax_number("123456789B01"));
    assert!(is_dutch_btw_tax_number("123456789B99"));
    assert!(!is_dutch_btw_tax_number("123456789B00"));

    let result = detect_column_type_with_name(
        "btw_nummer",
        &strings(&["123456789B00", "111234567B00", "004495445B00"]),
    );
    assert_ne!(result.data_type, DataType::TaxId);
}

#[test]
fn invalid_vat_checksums_do_not_become_tax_ids() {
    for header in ["btw_nummer", "BTW", "vat_number"] {
        let result = detect_column_type_with_name(
            header,
            &strings(&["NL123456789B01", "NL000099998B58", "DE123456789"]),
        );

        assert_ne!(result.data_type, DataType::TaxId, "header {header}");
    }
}

#[test]
fn dutch_bare_btw_tax_numbers_require_dutch_btw_header_context() {
    for header in ["vat_number", "tax_number", "code"] {
        let result = detect_column_type_with_name(
            header,
            &strings(&["123456789B01", "111234567B01", "004495445B01"]),
        );

        assert_ne!(result.data_type, DataType::TaxId, "header {header}");
    }
}

#[test]
fn vat_like_business_codes_do_not_become_tax_ids() {
    for header in ["code", "btw_nummer", "BTW"] {
        let result = detect_column_type_with_name(header, &strings(&["FR2024Q2", "DEMO1234"]));
        assert_ne!(result.data_type, DataType::TaxId, "header {header}");
    }
}
