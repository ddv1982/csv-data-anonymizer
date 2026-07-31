//! Streaming CSV transform benchmarks.
//!
//! Run them all with `cargo bench -p csv-anonymizer-core --bench csv_streaming`,
//! or one family with a filter, e.g.
//! `cargo bench -p csv-anonymizer-core --bench csv_streaming -- cardinality`.

use criterion::{Criterion, criterion_group, criterion_main};
use csv_anonymizer_core::{
    AnonymizationStrategy, AnonymizeParams, AnonymizerService, ColumnControl, DataType,
};
use std::hint::black_box;
use std::path::Path;

/// Row count shared by every case here. Small enough that criterion can take a
/// usable number of samples per case, large enough that the per-row streaming
/// work dominates the fixed setup cost.
const BENCH_ROWS: usize = 10_000;

/// Distinct values in the low-cardinality fixture: heavy repeats, so the
/// per-distinct-value bookkeeping is ~1% of what the all-unique fixture asks for.
const REPEATED_DISTINCT_VALUES: usize = 100;

fn bench_standard_csv_streaming(c: &mut Criterion) {
    let temp_dir = tempfile::tempdir().expect("benchmark temp dir should be created");
    let input_path = temp_dir.path().join("large-standard.csv");
    write_large_csv(&input_path, BENCH_ROWS);
    let output_path = temp_dir.path().join("large-standard-output.csv");
    let service = AnonymizerService::new("bench");

    c.bench_function("count_csv_rows_10k", |b| {
        b.iter(|| {
            service
                .count_csv_rows(black_box(&input_path))
                .expect("row count should succeed")
        })
    });

    c.bench_function("anonymize_standard_streaming_10k", |b| {
        b.iter(|| {
            service
                .anonymize_csv(AnonymizeParams {
                    file_path: black_box(input_path.clone()),
                    output_path: output_path.clone(),
                    columns: vec![1, 2],
                    controls: vec![
                        ColumnControl {
                            column_index: 1,
                            type_override: Some(DataType::Email),
                            strategy: AnonymizationStrategy::Auto,
                        },
                        ColumnControl {
                            column_index: 2,
                            type_override: Some(DataType::FullName),
                            strategy: AnonymizationStrategy::Auto,
                        },
                    ],
                    force: true,
                    preview_smart_replacements: vec![],
                })
                .expect("standard anonymization should succeed")
        })
    });
}

/// Separates cardinality from row count: identical row count and strategy, only
/// the number of distinct values in the selected column changes.
///
/// Worth tracking because transform state is O(distinct values per column), not
/// O(1): every consistently pseudonymized value is recorded in a per-column value
/// ledger, and the pseudonymizing strategies additionally keep both directions of
/// the source/output mapping. `Redact` records neither, so it is the control.
///
/// What each pair isolates, measured on 1M-row inputs where the same three
/// strategies separate peak RSS into ~9 MB of streaming floor, ~155 MB of ledger
/// and ~325 MB of pseudonym maps:
///
/// - `redact_*`: the streaming floor and the constant column-derived placeholder
///   path. Both shapes should stay equal, and equal to each other over time;
///   drift here is a read/parse/write or placeholder-decision regression.
/// - `label_*`: the floor plus the ledger. The ledger hashes every value on every
///   row, so its *time* cost is per-row and both shapes pay it; only its memory
///   is per-distinct-value, which is why the RSS harness and not this bench is
///   what sizes it.
/// - `pseudonymize_*`: the floor plus the ledger plus the pseudonym maps. This is
///   the pair that separates: `unique` pays a map insert per row while `repeated`
///   pays a lookup, and that gap is where most of the per-distinct-value cost of
///   a transform lives.
fn bench_cardinality(c: &mut Criterion) {
    let temp_dir = tempfile::tempdir().expect("benchmark temp dir should be created");
    let unique_path = temp_dir.path().join("cardinality-unique.csv");
    let repeated_path = temp_dir.path().join("cardinality-repeated.csv");
    write_cardinality_csv(&unique_path, BENCH_ROWS, BENCH_ROWS);
    write_cardinality_csv(&repeated_path, BENCH_ROWS, REPEATED_DISTINCT_VALUES);
    let output_path = temp_dir.path().join("cardinality-output.csv");
    let service = AnonymizerService::new("bench");

    let mut group = c.benchmark_group("cardinality");
    // Six cases over a 10k-row transform each: the default 100 samples would put
    // this bench file well past a minute for no extra resolution.
    group.sample_size(20);

    for (strategy_label, strategy) in [
        ("redact", AnonymizationStrategy::Redact),
        ("label", AnonymizationStrategy::Label),
        ("pseudonymize", AnonymizationStrategy::Pseudonymize),
    ] {
        for (shape_label, input_path) in [("unique", &unique_path), ("repeated", &repeated_path)] {
            group.bench_function(format!("{strategy_label}_{shape_label}_10k"), |b| {
                b.iter(|| {
                    service
                        .anonymize_csv(AnonymizeParams {
                            file_path: black_box(input_path.clone()),
                            output_path: output_path.clone(),
                            columns: vec![1],
                            controls: vec![ColumnControl {
                                column_index: 1,
                                type_override: Some(DataType::String),
                                strategy,
                            }],
                            force: true,
                            preview_smart_replacements: vec![],
                        })
                        .expect("cardinality anonymization should succeed")
                })
            });
        }
    }

    group.finish();
}

/// Writes `rows` data rows whose single selected column cycles through
/// `distinct_values` fixed-width values.
///
/// Fixed-width so the two fixtures differ only in cardinality: an input where the
/// repeated fixture also had shorter values would confound value-length effects
/// with distinct-value effects.
fn write_cardinality_csv(path: &Path, rows: usize, distinct_values: usize) {
    let distinct_values = distinct_values.max(1);
    let mut writer = csv::Writer::from_path(path).expect("benchmark CSV should be writable");
    writer
        .write_record(["id", "value"])
        .expect("header should write");
    for index in 0..rows {
        writer
            .write_record([
                index.to_string(),
                format!("value_{:010}", index % distinct_values),
            ])
            .expect("row should write");
    }
    writer.flush().expect("benchmark CSV should flush");
}

fn write_large_csv(path: &Path, rows: usize) {
    let mut writer = csv::Writer::from_path(path).expect("benchmark CSV should be writable");
    writer
        .write_record(["id", "email", "full_name", "region", "amount"])
        .expect("header should write");
    for index in 0..rows {
        writer
            .write_record([
                index.to_string(),
                format!("user{index}@example.com"),
                format!("Person {index}"),
                match index % 4 {
                    0 => "north".to_string(),
                    1 => "south".to_string(),
                    2 => "east".to_string(),
                    _ => "west".to_string(),
                },
                format!("{}.{}", index % 100, index % 10),
            ])
            .expect("row should write");
    }
    writer.flush().expect("benchmark CSV should flush");
}

criterion_group!(benches, bench_standard_csv_streaming, bench_cardinality);
criterion_main!(benches);
