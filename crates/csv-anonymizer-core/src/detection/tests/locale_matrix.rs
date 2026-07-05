//! Per-locale triples: native headers, English headers, and no headers must
//! produce identical column classifications (design: Testing section).
//!
//! This module is the definition of done for the value-first detection goal:
//! the same file classifies the same way regardless of the header language,
//! because every sensitive type is decided from the cell *values*.
//!
//! Person-name columns are deliberately omitted from these fixtures: name
//! detection remains header-taxonomy-dependent by design. Bundled name
//! datasets were rejected on data-minimization grounds (user decision,
//! 2026-07-06), so header-independent name classification is future work
//! pending a user-approved data source. The remaining columns (national ID,
//! phone, postal, locale context, benign look-alike) exercise the full
//! value-first pipeline.
//!
//! How each fixture vector was verified (see task-12-report.md for detail):
//! - National IDs are the checksum-proven vectors from `national_id.rs`'s
//!   probe tests (idsmith-validated); the US ID column uses formatted SSNs
//!   (`XXX-XX-XXXX`) accepted by the `ssn` crate. All-valid columns → High.
//! - Phones use `+<country>` international format (region-independent `+`
//!   fast path) validated by libphonenumber.
//! - Postal formats: NL/PL/BR are context-free; DE/FR/IT/ES/US/JP are
//!   `requires_context`, so those fixtures carry a context-bearing column
//!   (a country IBAN, or — for US/JP which have no IBAN — a dominant
//!   country-code column). The phone column does not contribute context.
//! - The benign look-alike is an 8-digit order id: it stays `NumericId`
//!   (it does not match any 5-digit postal format, so it is never mistaken
//!   for a postal code even inside a locale-tagged file).

use crate::metadata::build_column_metadata;
use crate::types::DataType;

struct LocaleFixture {
    locale: &'static str,
    native_headers: &'static [&'static str],
    english_headers: &'static [&'static str],
    /// Machine-style headers stand in for "no headers": col1, col2, ...
    rows: &'static [&'static [&'static str]],
    expected: &'static [DataType],
}

fn assert_triple(fixture: &LocaleFixture) {
    let generic_headers: Vec<String> = (0..fixture.expected.len())
        .map(|index| format!("col{}", index + 1))
        .collect();
    for (label, headers) in [
        ("native", to_owned(fixture.native_headers)),
        ("english", to_owned(fixture.english_headers)),
        ("headerless", generic_headers),
    ] {
        let rows: Vec<Vec<String>> = fixture
            .rows
            .iter()
            .map(|row| row.iter().map(|cell| cell.to_string()).collect())
            .collect();
        let metadata = build_column_metadata(&headers, &rows);
        for (index, expected) in fixture.expected.iter().enumerate() {
            assert_eq!(
                metadata[index].detected_type, *expected,
                "{} / {} / column {}",
                fixture.locale, label, index
            );
        }
    }
}

fn to_owned(headers: &[&str]) -> Vec<String> {
    headers.iter().map(|header| header.to_string()).collect()
}

// The benign look-alike is an 8-digit order id (`48210000`..`48210011`),
// inlined per row below. Being 8 digits it matches neither any 5-digit postal
// format nor any national-ID checksum, so it stays `NumericId` even inside a
// locale-tagged file.

#[test]
fn nl_triple() {
    // NL postcodes (`1012 AB`) are letters+digits: context-free, no IBAN needed.
    assert_triple(&LocaleFixture {
        locale: "NL",
        native_headers: &["bsn", "telefoonnummer", "postcode", "artikelnr"],
        english_headers: &["national id", "phone", "postal code", "order id"],
        rows: &[
            &["111222333", "+31 6 12345678", "1012 AB", "48210000"],
            &["123456782", "+31 6 12345678", "2511 CV", "48210001"],
            &["111222333", "+31 6 12345678", "3011 ED", "48210002"],
            &["123456782", "+31 6 12345678", "9711 LM", "48210003"],
            &["111222333", "+31 6 12345678", "5611 EM", "48210004"],
            &["123456782", "+31 6 12345678", "6511 KL", "48210005"],
            &["111222333", "+31 6 12345678", "7511 JE", "48210006"],
            &["123456782", "+31 6 12345678", "8011 NW", "48210007"],
            &["111222333", "+31 6 12345678", "1053 ZK", "48210008"],
            &["123456782", "+31 6 12345678", "2033 AB", "48210009"],
            &["111222333", "+31 6 12345678", "3512 JE", "48210010"],
            &["123456782", "+31 6 12345678", "4811 HE", "48210011"],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
        ],
    });
}

#[test]
fn de_triple() {
    // DE postcodes are bare 5-digit (`requires_context`): the IBAN column
    // establishes DE context so the postal voter beats the numeric-id shape.
    assert_triple(&LocaleFixture {
        locale: "DE",
        native_headers: &[
            "steuer-id",
            "telefonnummer",
            "postleitzahl",
            "bestellnr",
            "iban",
        ],
        english_headers: &["tax id", "phone", "postal code", "order id", "iban"],
        rows: &[
            &[
                "86095742719",
                "+49 30 12345678",
                "10115",
                "48210000",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "20095",
                "48210001",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "80331",
                "48210002",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "50667",
                "48210003",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "60311",
                "48210004",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "70173",
                "48210005",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "01067",
                "48210006",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "04109",
                "48210007",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "28195",
                "48210008",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "30159",
                "48210009",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "40213",
                "48210010",
                "DE89370400440532013000",
            ],
            &[
                "86095742719",
                "+49 30 12345678",
                "90402",
                "48210011",
                "DE89370400440532013000",
            ],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
            DataType::String, // IBAN column: context-only, classified String by the IBAN voter.
        ],
    });
}

#[test]
fn fr_triple() {
    // FR postcodes are bare 5-digit (`requires_context`): FR IBAN gives context.
    assert_triple(&LocaleFixture {
        locale: "FR",
        native_headers: &[
            "numéro de sécurité sociale",
            "téléphone",
            "code postal",
            "numéro de commande",
            "iban",
        ],
        english_headers: &["national id", "phone", "postal code", "order id", "iban"],
        rows: &[
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "75001",
                "48210000",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "69002",
                "48210001",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "13001",
                "48210002",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "31000",
                "48210003",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "44000",
                "48210004",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "33000",
                "48210005",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "59000",
                "48210006",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "67000",
                "48210007",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "34000",
                "48210008",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "06000",
                "48210009",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "21000",
                "48210010",
                "FR1420041010050500013M02606",
            ],
            &[
                "255081416802538",
                "+33 1 42 68 53 00",
                "51100",
                "48210011",
                "FR1420041010050500013M02606",
            ],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
            DataType::String,
        ],
    });
}

#[test]
fn pl_triple() {
    // PL postcodes (`00-001`) are `NN-NNN`: context-free, no IBAN needed.
    assert_triple(&LocaleFixture {
        locale: "PL",
        native_headers: &["pesel", "telefon", "kod pocztowy", "numer zamówienia"],
        english_headers: &["national id", "phone", "postal code", "order id"],
        rows: &[
            &["44051401359", "+48 22 621 02 05", "00-001", "48210000"],
            &["44051401359", "+48 22 621 02 05", "31-042", "48210001"],
            &["44051401359", "+48 22 621 02 05", "80-001", "48210002"],
            &["44051401359", "+48 22 621 02 05", "50-001", "48210003"],
            &["44051401359", "+48 22 621 02 05", "61-001", "48210004"],
            &["44051401359", "+48 22 621 02 05", "90-001", "48210005"],
            &["44051401359", "+48 22 621 02 05", "20-001", "48210006"],
            &["44051401359", "+48 22 621 02 05", "40-001", "48210007"],
            &["44051401359", "+48 22 621 02 05", "70-001", "48210008"],
            &["44051401359", "+48 22 621 02 05", "35-001", "48210009"],
            &["44051401359", "+48 22 621 02 05", "15-001", "48210010"],
            &["44051401359", "+48 22 621 02 05", "10-001", "48210011"],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
        ],
    });
}

#[test]
fn it_triple() {
    // IT postcodes (CAP) are bare 5-digit (`requires_context`): IT IBAN gives context.
    assert_triple(&LocaleFixture {
        locale: "IT",
        native_headers: &["codice fiscale", "telefono", "cap", "numero ordine", "iban"],
        english_headers: &["national id", "phone", "postal code", "order id", "iban"],
        rows: &[
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "00118",
                "48210000",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "20121",
                "48210001",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "10121",
                "48210002",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "80121",
                "48210003",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "50122",
                "48210004",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "40121",
                "48210005",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "16121",
                "48210006",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "70121",
                "48210007",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "90133",
                "48210008",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "37121",
                "48210009",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "35121",
                "48210010",
                "IT60X0542811101000000123456",
            ],
            &[
                "RSSMRA85T10A562S",
                "+39 06 6982 1234",
                "34121",
                "48210011",
                "IT60X0542811101000000123456",
            ],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
            DataType::String,
        ],
    });
}

#[test]
fn es_triple() {
    // ES postcodes are bare 5-digit (`requires_context`): ES IBAN gives context.
    assert_triple(&LocaleFixture {
        locale: "ES",
        native_headers: &[
            "dni",
            "teléfono",
            "código postal",
            "número de pedido",
            "iban",
        ],
        english_headers: &["national id", "phone", "postal code", "order id", "iban"],
        rows: &[
            &[
                "12345678Z",
                "+34 612 345 678",
                "28001",
                "48210000",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "08001",
                "48210001",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "41001",
                "48210002",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "46001",
                "48210003",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "50001",
                "48210004",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "29001",
                "48210005",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "48001",
                "48210006",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "30001",
                "48210007",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "03001",
                "48210008",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "35001",
                "48210009",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "15001",
                "48210010",
                "ES9121000418450200051332",
            ],
            &[
                "12345678Z",
                "+34 612 345 678",
                "47001",
                "48210011",
                "ES9121000418450200051332",
            ],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
            DataType::String,
        ],
    });
}

#[test]
fn br_triple() {
    // BR postcodes (CEP `NNNNN-NNN`) are context-free, no IBAN needed.
    assert_triple(&LocaleFixture {
        locale: "BR",
        native_headers: &["cpf", "telefone", "cep", "número do pedido"],
        english_headers: &["national id", "phone", "postal code", "order id"],
        rows: &[
            &["11144477735", "+55 11 91234 5678", "01310-100", "48210000"],
            &["11144477735", "+55 11 91234 5678", "20040-002", "48210001"],
            &["11144477735", "+55 11 91234 5678", "30140-071", "48210002"],
            &["11144477735", "+55 11 91234 5678", "40026-010", "48210003"],
            &["11144477735", "+55 11 91234 5678", "80010-000", "48210004"],
            &["11144477735", "+55 11 91234 5678", "90010-150", "48210005"],
            &["11144477735", "+55 11 91234 5678", "50030-230", "48210006"],
            &["11144477735", "+55 11 91234 5678", "60160-230", "48210007"],
            &["11144477735", "+55 11 91234 5678", "70040-010", "48210008"],
            &["11144477735", "+55 11 91234 5678", "13010-111", "48210009"],
            &["11144477735", "+55 11 91234 5678", "22041-011", "48210010"],
            &["11144477735", "+55 11 91234 5678", "88010-400", "48210011"],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
        ],
    });
}

#[test]
fn jp_triple() {
    // National ID: Japan's My Number has no checksum scheme in the idsmith
    // allowlist, so no JP-authentic national ID can be validated. National-ID
    // detection is value-first and locale-independent (the idsmith battery
    // tries every allowlisted country regardless of the file's inferred
    // locale), so a valid allowlisted ID vector still proves the intended
    // "national-ID column → TaxId" behavior here. We reuse the checksum-proven
    // DE Steuer-IdNr vector from `national_id.rs`.
    //
    // JP postcodes (`NNN-NNNN`) are `requires_context`; Japan has no IBAN, so a
    // dominant JP country-code column establishes the context.
    assert_triple(&LocaleFixture {
        locale: "JP",
        native_headers: &["マイナンバー", "電話番号", "郵便番号", "注文番号", "国"],
        english_headers: &["national id", "phone", "postal code", "order id", "country"],
        rows: &[
            &[
                "86095742719",
                "+81 90 1234 5678",
                "150-0002",
                "48210000",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "100-0001",
                "48210001",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "530-0001",
                "48210002",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "060-0001",
                "48210003",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "460-0001",
                "48210004",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "810-0001",
                "48210005",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "980-0001",
                "48210006",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "700-0011",
                "48210007",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "260-0001",
                "48210008",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "220-0011",
                "48210009",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "330-0801",
                "48210010",
                "JP",
            ],
            &[
                "86095742719",
                "+81 90 1234 5678",
                "020-0011",
                "48210011",
                "JP",
            ],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
            DataType::CountryCode, // context column: dominant JP codes.
        ],
    });
}

#[test]
fn us_triple() {
    // US ZIP codes are bare 5-digit (`requires_context`); the US has no IBAN, so
    // a dominant US country-code column establishes the context. The ID column
    // uses formatted SSNs (`XXX-XX-XXXX`), which the `is_tax_id` detector
    // validates; bare 9-digit SSNs stay NumericId by design.
    assert_triple(&LocaleFixture {
        locale: "US",
        native_headers: &["ssn", "phone", "zip code", "order id", "country"],
        english_headers: &["national id", "phone", "postal code", "order id", "country"],
        rows: &[
            &["446-72-2445", "+1 415 555 0100", "94103", "48210000", "US"],
            &["446-72-2445", "+1 415 555 0100", "10001", "48210001", "US"],
            &["446-72-2445", "+1 415 555 0100", "60601", "48210002", "US"],
            &["446-72-2445", "+1 415 555 0100", "77002", "48210003", "US"],
            &["446-72-2445", "+1 415 555 0100", "33101", "48210004", "US"],
            &["446-72-2445", "+1 415 555 0100", "98101", "48210005", "US"],
            &["446-72-2445", "+1 415 555 0100", "02108", "48210006", "US"],
            &["446-72-2445", "+1 415 555 0100", "19103", "48210007", "US"],
            &["446-72-2445", "+1 415 555 0100", "30303", "48210008", "US"],
            &["446-72-2445", "+1 415 555 0100", "48201", "48210009", "US"],
            &["446-72-2445", "+1 415 555 0100", "85001", "48210010", "US"],
            &["446-72-2445", "+1 415 555 0100", "20500", "48210011", "US"],
        ],
        expected: &[
            DataType::TaxId,
            DataType::Phone,
            DataType::PostalCode,
            DataType::NumericId,
            DataType::CountryCode,
        ],
    });
}
