use criterion::{Criterion, criterion_group, criterion_main};
use csv_anonymizer_core::{
    AnonymizationStrategy, AnonymizeParams, AnonymizerService, ColumnControl, DataType,
};
use std::hint::black_box;
use std::path::Path;

fn bench_standard_csv_streaming(c: &mut Criterion) {
    let temp_dir = tempfile::tempdir().expect("benchmark temp dir should be created");
    let input_path = temp_dir.path().join("large-standard.csv");
    write_large_csv(&input_path, 10_000);
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
                    deterministic: true,
                    seed: "bench-seed".to_string(),
                    force: true,
                    preview_smart_replacements: vec![],
                })
                .expect("standard anonymization should succeed")
        })
    });
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

criterion_group!(benches, bench_standard_csv_streaming);
criterion_main!(benches);
