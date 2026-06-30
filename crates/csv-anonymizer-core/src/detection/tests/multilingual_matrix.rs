use super::*;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum FixtureMode {
    Value,
    Header,
    HeaderAndValue,
}

impl FixtureMode {
    fn label(self) -> &'static str {
        match self {
            FixtureMode::Value => "value",
            FixtureMode::Header => "header",
            FixtureMode::HeaderAndValue => "header+value",
        }
    }
}

struct PositiveFixture {
    language: &'static str,
    entity: &'static str,
    mode: FixtureMode,
    header: &'static str,
    values: &'static [&'static str],
    expected: DataType,
}

struct NegativeFixture {
    language: &'static str,
    entity: &'static str,
    mode: FixtureMode,
    header: &'static str,
    values: &'static [&'static str],
    rejected: &'static [DataType],
}

#[test]
fn multilingual_detection_matrix_covers_headers_values_and_contextual_pairs() {
    let positives = positive_fixtures();

    for fixture in positives {
        let result = detect_column_type_with_name(fixture.header, &strings(fixture.values));
        assert_eq!(
            result.data_type,
            fixture.expected,
            "{} {} {} header {}",
            fixture.language,
            fixture.mode.label(),
            fixture.entity,
            fixture.header
        );
        assert_ne!(
            result.confidence,
            Confidence::Low,
            "{} {} {} should not be low confidence",
            fixture.language,
            fixture.mode.label(),
            fixture.entity
        );
        assert!(
            result.trace.as_ref().is_some_and(|trace| {
                !trace.selected_reason.is_empty() && !trace.candidates.is_empty()
            }),
            "{} {} {} should include trace evidence",
            fixture.language,
            fixture.mode.label(),
            fixture.entity
        );
    }
}

#[test]
fn multilingual_detection_matrix_covers_claimed_language_labels() {
    let languages = positive_fixtures()
        .iter()
        .map(|fixture| fixture.language)
        .collect::<std::collections::HashSet<_>>();

    for expected_language in ["en", "nl", "de", "fr", "es", "pt", "it", "ja"] {
        assert!(
            languages.contains(expected_language),
            "missing positive fixture for {expected_language}"
        );
    }
}

#[test]
fn multilingual_detection_matrix_rejects_ambiguous_headers_and_vat_near_misses() {
    for fixture in negative_fixtures() {
        let result = detect_column_type_with_name(fixture.header, &strings(fixture.values));
        assert!(
            !fixture.rejected.contains(&result.data_type),
            "{} {} {} header {} should not detect {:?}, got {:?}",
            fixture.language,
            fixture.mode.label(),
            fixture.entity,
            fixture.header,
            fixture.rejected,
            result.data_type
        );
    }
}

#[test]
fn multilingual_detection_matrix_quality_gate_reports_precision_recall_and_entities() {
    let start = Instant::now();
    let mut true_positive = 0;
    let mut true_negative = 0;
    let mut false_negative = Vec::new();
    let mut false_positive = Vec::new();
    let mut entity_counts = BTreeMap::<&'static str, usize>::new();

    for fixture in positive_fixtures() {
        let result = detect_column_type_with_name(fixture.header, &strings(fixture.values));
        if result.data_type == fixture.expected {
            true_positive += 1;
            *entity_counts.entry(fixture.entity).or_default() += 1;
        } else {
            false_negative.push(format!(
                "{} {} {} expected {:?}, got {:?}",
                fixture.language,
                fixture.mode.label(),
                fixture.entity,
                fixture.expected,
                result.data_type
            ));
        }
    }

    for fixture in negative_fixtures() {
        let result = detect_column_type_with_name(fixture.header, &strings(fixture.values));
        if fixture.rejected.contains(&result.data_type) {
            false_positive.push(format!(
                "{} {} {} rejected {:?}, got {:?}",
                fixture.language,
                fixture.mode.label(),
                fixture.entity,
                fixture.rejected,
                result.data_type
            ));
        } else {
            true_negative += 1;
        }
    }

    let false_positive_count = false_positive.len();
    let false_negative_count = false_negative.len();
    let precision = true_positive as f64 / (true_positive + false_positive_count) as f64;
    let recall = true_positive as f64 / (true_positive + false_negative_count) as f64;

    assert!(
        false_negative.is_empty(),
        "detector matrix false negatives:\n{}",
        false_negative.join("\n")
    );
    assert!(
        false_positive.is_empty(),
        "detector matrix false positives:\n{}",
        false_positive.join("\n")
    );
    assert_eq!(true_positive, positive_fixtures().len());
    assert_eq!(true_negative, negative_fixtures().len());
    assert_eq!(precision, 1.0, "precision should remain fixture-perfect");
    assert_eq!(recall, 1.0, "recall should remain fixture-perfect");

    for entity in [
        "email",
        "phone",
        "uuid",
        "btw",
        "prefixed_vat",
        "birth_date",
        "address",
        "postal_code",
        "full_name",
    ] {
        assert!(
            entity_counts.contains_key(entity),
            "quality gate missing per-entity count for {entity}; got {entity_counts:?}"
        );
    }

    assert!(
        start.elapsed() < Duration::from_secs(2),
        "detector matrix took {:?} for {} representative columns",
        start.elapsed(),
        positive_fixtures().len() + negative_fixtures().len()
    );
}

fn positive_fixtures() -> &'static [PositiveFixture] {
    &[
        PositiveFixture {
            language: "en",
            entity: "email",
            mode: FixtureMode::Value,
            header: "",
            values: &["ada@example.com", "grace@example.org"],
            expected: DataType::Email,
        },
        PositiveFixture {
            language: "en",
            entity: "phone",
            mode: FixtureMode::HeaderAndValue,
            header: "phone_number",
            values: &["+1 415 555 0100", "+1 212 555 0101"],
            expected: DataType::Phone,
        },
        PositiveFixture {
            language: "en",
            entity: "uuid",
            mode: FixtureMode::Value,
            header: "",
            values: &[
                "550e8400-e29b-41d4-a716-446655440000",
                "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            ],
            expected: DataType::Uuid,
        },
        PositiveFixture {
            language: "nl",
            entity: "phone",
            mode: FixtureMode::HeaderAndValue,
            header: "telefoon",
            values: &["+31 6 12345678", "+31 20 123 4567"],
            expected: DataType::Phone,
        },
        PositiveFixture {
            language: "nl",
            entity: "btw",
            mode: FixtureMode::HeaderAndValue,
            header: "BTW",
            values: &["123456789B01", "987654321B99"],
            expected: DataType::TaxId,
        },
        PositiveFixture {
            language: "nl",
            entity: "prefixed_vat",
            mode: FixtureMode::Value,
            header: "",
            values: &["NL000099998B57", "DE111111125"],
            expected: DataType::TaxId,
        },
        PositiveFixture {
            language: "de",
            entity: "birth_date",
            mode: FixtureMode::Header,
            header: "geburtsdatum",
            values: &["1980-01-02", "1991-03-04"],
            expected: DataType::Timestamp,
        },
        PositiveFixture {
            language: "de",
            entity: "address",
            mode: FixtureMode::HeaderAndValue,
            header: "adresse",
            values: &["Hauptstrasse 12", "Marktplatz 5"],
            expected: DataType::Address,
        },
        PositiveFixture {
            language: "fr",
            entity: "postal_code",
            mode: FixtureMode::HeaderAndValue,
            header: "code postal",
            values: &["75001", "69002"],
            expected: DataType::PostalCode,
        },
        PositiveFixture {
            language: "fr",
            entity: "full_name",
            mode: FixtureMode::Header,
            header: "nom_complet",
            values: &["Marie Dubois", "Luc Martin"],
            expected: DataType::FullName,
        },
        PositiveFixture {
            language: "es",
            entity: "phone",
            mode: FixtureMode::HeaderAndValue,
            header: "teléfono",
            values: &["+34 612 345 678", "+34 611 111 111"],
            expected: DataType::Phone,
        },
        PositiveFixture {
            language: "es",
            entity: "birth_date",
            mode: FixtureMode::Header,
            header: "fecha_nacimiento",
            values: &["1980-01-02", "1991-03-04"],
            expected: DataType::Timestamp,
        },
        PositiveFixture {
            language: "pt",
            entity: "address",
            mode: FixtureMode::HeaderAndValue,
            header: "endereço",
            values: &["Rua Augusta 10", "Avenida Brasil 22"],
            expected: DataType::Address,
        },
        PositiveFixture {
            language: "pt",
            entity: "full_name",
            mode: FixtureMode::Header,
            header: "nome_completo",
            values: &["Ana Silva", "João Pereira"],
            expected: DataType::FullName,
        },
        PositiveFixture {
            language: "it",
            entity: "postal_code",
            mode: FixtureMode::HeaderAndValue,
            header: "codice_postale",
            values: &["00118", "20121"],
            expected: DataType::PostalCode,
        },
        PositiveFixture {
            language: "it",
            entity: "full_name",
            mode: FixtureMode::Header,
            header: "nome_e_cognome",
            values: &["Giulia Rossi", "Marco Bianchi"],
            expected: DataType::FullName,
        },
        PositiveFixture {
            language: "ja",
            entity: "phone",
            mode: FixtureMode::HeaderAndValue,
            header: "電話番号",
            values: &["+81 90 1234 5678", "+81 80 2345 6789"],
            expected: DataType::Phone,
        },
        PositiveFixture {
            language: "ja",
            entity: "address",
            mode: FixtureMode::HeaderAndValue,
            header: "住所",
            values: &["東京都渋谷区1-2-3", "大阪市北区4-5-6"],
            expected: DataType::Address,
        },
        PositiveFixture {
            language: "ja",
            entity: "birth_date",
            mode: FixtureMode::Header,
            header: "生年月日",
            values: &["1980-01-02", "1991-03-04"],
            expected: DataType::Timestamp,
        },
    ]
}

fn negative_fixtures() -> &'static [NegativeFixture] {
    &[
        NegativeFixture {
            language: "nl",
            entity: "name_header_status_values",
            mode: FixtureMode::Header,
            header: "naam",
            values: &["active", "inactive", "pending"],
            rejected: &[DataType::FirstName, DataType::FullName],
        },
        NegativeFixture {
            language: "es",
            entity: "phone_header_status_values",
            mode: FixtureMode::Header,
            header: "teléfono",
            values: &["active", "inactive", "pending"],
            rejected: &[DataType::Phone],
        },
        NegativeFixture {
            language: "pt",
            entity: "tax_header_code_values",
            mode: FixtureMode::Header,
            header: "nif",
            values: &["blue", "green", "red"],
            rejected: &[DataType::TaxId],
        },
        NegativeFixture {
            language: "nl",
            entity: "bare_btw_without_header",
            mode: FixtureMode::Value,
            header: "",
            values: &["123456789B01", "987654321B99"],
            rejected: &[DataType::TaxId],
        },
        NegativeFixture {
            language: "nl",
            entity: "invalid_btw_suffix",
            mode: FixtureMode::HeaderAndValue,
            header: "btw_nummer",
            values: &["123456789B00", "987654321B00"],
            rejected: &[DataType::TaxId],
        },
        NegativeFixture {
            language: "eu",
            entity: "invalid_vat_checksum",
            mode: FixtureMode::HeaderAndValue,
            header: "vat_number",
            values: &["NL000099998B56", "DE111111126"],
            rejected: &[DataType::TaxId],
        },
        NegativeFixture {
            language: "eu",
            entity: "vat_like_business_codes",
            mode: FixtureMode::Header,
            header: "btw_nummer",
            values: &["FR2024Q2", "DEMO1234"],
            rejected: &[DataType::TaxId],
        },
    ]
}
