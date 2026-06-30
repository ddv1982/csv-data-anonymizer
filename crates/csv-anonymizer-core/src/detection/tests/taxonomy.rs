use super::*;
use crate::types::PrivacyFindingKind;

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
fn generic_name_header_with_single_names_detects_first_name() {
    let result = detect_column_type_with_name("name", &strings(&["Alice", "Bob", "Carol"]));
    assert_eq!(result.data_type, DataType::FirstName);
    assert_eq!(result.confidence, Confidence::High);
}

#[test]
fn restores_compact_and_legacy_header_aliases() {
    let secret_values = strings(&["abcdefghi"]);
    let secret_analysis = analyze("apikey", &secret_values);
    assert_eq!(secret_analysis.pii_risk, PiiRisk::High);
    assert!(secret_analysis.evidence.iter().any(|summary| {
        summary.kind == PrivacyFindingKind::CredentialOrSecret
            && summary.detector.starts_with("header:taxonomy")
    }));

    for header in ["homephone", "workphone"] {
        let result = detect_column_type_with_name(header, &strings(&["+1-555-0123"]));
        assert_eq!(result.data_type, DataType::Phone, "header {header}");
        assert_eq!(result.confidence, Confidence::High, "header {header}");
    }

    for header in ["person_id", "record_id"] {
        let result = detect_column_type_with_name(header, &strings(&["1", "2", "3"]));
        assert_eq!(result.data_type, DataType::NumericId, "header {header}");
        assert_eq!(result.confidence, Confidence::High, "header {header}");
    }
}

fn assert_header_taxonomy_cases(cases: &[(&str, &[&str], DataType)]) {
    for (header, values, expected) in cases {
        let result = detect_column_type_with_name(header, &strings(values));
        assert_eq!(result.data_type, *expected, "header {header}");
        assert_ne!(result.confidence, Confidence::Low, "header {header}");
        if *expected != DataType::Timestamp {
            assert!(
                result
                    .trace
                    .as_ref()
                    .is_some_and(|trace| trace.selected_reason.contains("Header taxonomy term")),
                "header {header} should explain taxonomy evidence"
            );
        }
    }
}

#[test]
fn detects_multilingual_contact_and_address_header_taxonomy_terms() {
    assert_header_taxonomy_cases(&[
        (
            "telefoon",
            &["+31 6 12345678", "+31 20 123 4567"],
            DataType::Phone,
        ),
        (
            "teléfono",
            &["+34 612 345 678", "+34 611 111 111"],
            DataType::Phone,
        ),
        (
            "telefonnummer",
            &["+49 30 123456", "+49 89 123456"],
            DataType::Phone,
        ),
        (
            "adresse",
            &["12 Rue de Rivoli", "5 Avenue Victor Hugo"],
            DataType::Address,
        ),
        (
            "dirección",
            &["Calle Mayor 10", "Avenida Libertad 22"],
            DataType::Address,
        ),
        (
            "adres",
            &["Kerkstraat 12", "Marktplein 8"],
            DataType::Address,
        ),
        (
            "住所",
            &["東京都渋谷区1-2-3", "大阪市北区4-5-6"],
            DataType::Address,
        ),
    ]);
}

#[test]
fn detects_multilingual_date_name_and_postal_header_taxonomy_terms() {
    assert_header_taxonomy_cases(&[
        (
            "geboortedatum",
            &["1980-01-02", "1991-03-04"],
            DataType::Timestamp,
        ),
        (
            "geburtsdatum",
            &["1980-01-02", "1991-03-04"],
            DataType::Timestamp,
        ),
        (
            "fecha_nacimiento",
            &["1980-01-02", "1991-03-04"],
            DataType::Timestamp,
        ),
        ("voornaam", &["Renée", "Søren"], DataType::FirstName),
        ("achternaam", &["Jansen", "Müller"], DataType::LastName),
        (
            "nom_complet",
            &["Renée Martin", "Søren Müller"],
            DataType::FullName,
        ),
        ("postcode", &["1012 AB", "3011 AA"], DataType::PostalCode),
        ("codigo_postal", &["28013", "08002"], DataType::PostalCode),
        ("plz", &["10115", "80331"], DataType::PostalCode),
    ]);
}

#[test]
fn detects_portuguese_italian_and_account_header_taxonomy_terms() {
    assert_header_taxonomy_cases(&[
        (
            "telefone",
            &["+351 21 123 4567", "+351 91 234 5678"],
            DataType::Phone,
        ),
        (
            "telemóvel",
            &["+351 91 234 5678", "+351 96 765 4321"],
            DataType::Phone,
        ),
        (
            "endereço",
            &["Rua Augusta 10", "Avenida Brasil 22"],
            DataType::Address,
        ),
        (
            "indirizzo",
            &["Via Roma 12", "Piazza Duomo 3"],
            DataType::Address,
        ),
        (
            "data_nascimento",
            &["1980-01-02", "1991-03-04"],
            DataType::Timestamp,
        ),
        (
            "data_di_nascita",
            &["1980-01-02", "1991-03-04"],
            DataType::Timestamp,
        ),
        (
            "nome_completo",
            &["Ana Silva", "João Pereira"],
            DataType::FullName,
        ),
        (
            "nome_e_cognome",
            &["Giulia Rossi", "Marco Bianchi"],
            DataType::FullName,
        ),
        ("sobrenome", &["Silva", "Pereira"], DataType::LastName),
        ("cognome", &["Rossi", "Bianchi"], DataType::LastName),
        (
            "codigo_postal",
            &["1000-001", "4000-002"],
            DataType::PostalCode,
        ),
        ("codice_postale", &["00118", "20121"], DataType::PostalCode),
        ("rekeningnummer", &["123", "987"], DataType::NumericId),
        ("kontonummer", &["123", "987"], DataType::NumericId),
    ]);
}

#[test]
fn ambiguous_multilingual_headers_still_need_value_shape() {
    assert_ne!(
        detect_column_type_with_name("naam", &strings(&["active", "inactive", "pending"]))
            .data_type,
        DataType::FirstName
    );
    assert_ne!(
        detect_column_type_with_name("code", &strings(&["blue", "green", "red"])).data_type,
        DataType::NumericId
    );
    for header in ["nr", "number", "id", "code", "naam"] {
        let result = detect_column_type_with_name(header, &strings(&["active", "inactive"]));
        assert!(
            !matches!(
                result.data_type,
                DataType::NumericId | DataType::FirstName | DataType::FullName
            ),
            "header {header}"
        );
    }
}

#[test]
fn fuzzy_header_taxonomy_matches_long_typos_with_value_shape() {
    let phone = detect_column_type_with_name(
        "telefoonnumer",
        &strings(&["+31 6 12345678", "+31 20 123 4567"]),
    );
    assert_eq!(phone.data_type, DataType::Phone);
    assert!(phone.trace.as_ref().is_some_and(|trace| {
        trace
            .selected_reason
            .contains("approximately matched taxonomy term")
    }));

    let tax =
        detect_column_type_with_name("btw_numner", &strings(&["NL000099998B57", "DE111111125"]));
    assert_eq!(tax.data_type, DataType::TaxId);
    assert!(tax.trace.as_ref().is_some_and(|trace| {
        trace
            .selected_reason
            .contains("approximately matched taxonomy term")
    }));

    let address = detect_column_type_with_name(
        "street_adress",
        &strings(&["123 Main St", "44 Market Road"]),
    );
    assert_eq!(address.data_type, DataType::Address);
}

#[test]
fn fuzzy_header_taxonomy_does_not_expand_short_ambiguous_headers() {
    for header in ["idx", "codde", "nam", "niff", "headphone"] {
        let result = detect_column_type_with_name(header, &strings(&["active", "inactive"]));
        assert!(
            !matches!(
                result.data_type,
                DataType::NumericId | DataType::TaxId | DataType::FirstName | DataType::Phone
            ),
            "header {header}"
        );
    }
}

#[test]
fn avoids_phone_false_positive_for_unrelated_phone_suffix_headers() {
    let result = detect_column_type_with_name("headphone", &strings(&["1234567"]));

    assert_ne!(result.data_type, DataType::Phone);
}
