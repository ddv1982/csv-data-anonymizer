use criterion::{Criterion, criterion_group, criterion_main};
use csv_anonymizer_core::detection::detect_column_type_with_name;
use std::hint::black_box;

#[derive(Clone, Copy)]
struct FixtureCase {
    header: &'static str,
    values: &'static [&'static str],
}

fn bench_detector_matrix(c: &mut Criterion) {
    let fixtures = fixtures();

    c.bench_function("detector_matrix/current_fixture_set", |b| {
        b.iter(|| {
            for fixture in black_box(fixtures) {
                let values = fixture
                    .values
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect::<Vec<_>>();
                black_box(detect_column_type_with_name(fixture.header, &values));
            }
        })
    });

    // This family protects the performance of the semantic refinement added on
    // top of UUID shape detection. All cases have the same values; only header
    // context changes. A regression here therefore points at header taxonomy or
    // evidence aggregation rather than parsing or validators.
    let uuid_fixtures = uuid_context_fixtures();
    c.bench_function("detector_matrix/uuid_semantic_context", |b| {
        b.iter(|| {
            for fixture in black_box(uuid_fixtures) {
                let values = fixture
                    .values
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect::<Vec<_>>();
                black_box(detect_column_type_with_name(fixture.header, &values));
            }
        })
    });
}

fn fixtures() -> &'static [FixtureCase] {
    &[
        FixtureCase {
            header: "",
            values: &["ada@example.com", "grace@example.org"],
        },
        FixtureCase {
            header: "phone_number",
            values: &["+1 415 555 0100", "+1 212 555 0101"],
        },
        FixtureCase {
            header: "",
            values: &[
                "550e8400-e29b-41d4-a716-446655440000",
                "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            ],
        },
        FixtureCase {
            header: "BTW",
            values: &["123456789B01", "987654321B99"],
        },
        FixtureCase {
            header: "",
            values: &["NL000099998B57", "DE111111125"],
        },
        FixtureCase {
            header: "geburtsdatum",
            values: &["1980-01-02", "1991-03-04"],
        },
        FixtureCase {
            header: "adresse",
            values: &["Hauptstrasse 12", "Marktplatz 5"],
        },
        FixtureCase {
            header: "code postal",
            values: &["75001", "69002"],
        },
        FixtureCase {
            header: "teléfono",
            values: &["+34 612 345 678", "+34 611 111 111"],
        },
        FixtureCase {
            header: "endereço",
            values: &["Rua Augusta 10", "Avenida Brasil 22"],
        },
        FixtureCase {
            header: "codice_postale",
            values: &["00118", "20121"],
        },
        FixtureCase {
            header: "電話番号",
            values: &["+81 90 1234 5678", "+81 80 2345 6789"],
        },
        FixtureCase {
            header: "naam",
            values: &["active", "inactive", "pending"],
        },
        FixtureCase {
            header: "vat_number",
            values: &["NL000099998B56", "DE111111126"],
        },
        FixtureCase {
            header: "",
            values: &["123456789B01", "987654321B99"],
        },
    ]
}

fn uuid_context_fixtures() -> &'static [FixtureCase] {
    const UUID_VALUES: &[&str] = &[
        "550e8400-e29b-41d4-a716-446655440000",
        "6ba7b810-9dad-11d1-80b4-00c04fd430c8",
        "123e4567-e89b-12d3-a456-426614174000",
        "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    ];

    &[
        FixtureCase {
            header: "",
            values: UUID_VALUES,
        },
        FixtureCase {
            header: "entity_id",
            values: UUID_VALUES,
        },
        FixtureCase {
            header: "customer_id",
            values: UUID_VALUES,
        },
        FixtureCase {
            header: "device_id",
            values: UUID_VALUES,
        },
        FixtureCase {
            header: "hardware_identifier",
            values: UUID_VALUES,
        },
        FixtureCase {
            header: "network_id",
            values: UUID_VALUES,
        },
        FixtureCase {
            header: "city_name",
            values: UUID_VALUES,
        },
        FixtureCase {
            header: "identifiant_appareil",
            values: UUID_VALUES,
        },
    ]
}

criterion_group!(benches, bench_detector_matrix);
criterion_main!(benches);
