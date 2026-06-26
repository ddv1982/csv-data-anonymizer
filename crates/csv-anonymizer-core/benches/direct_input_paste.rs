use criterion::{Criterion, criterion_group, criterion_main};
use csv_anonymizer_core::{
    AnonymizationStrategy, ColumnControl, DataType, PasteAnalyzeParams, PasteDataFormat,
    PasteTransformParams, direct_input,
};
use std::hint::black_box;

fn bench_pasted_json(c: &mut Criterion) {
    let content = build_json_payload(1_000);
    let analysis = direct_input::analyze_paste_data(PasteAnalyzeParams {
        content: content.clone(),
        format: PasteDataFormat::Json,
        sample_row_count: 1_000,
    })
    .expect("benchmark JSON analysis should succeed");
    let selected_columns =
        columns_for(&analysis.columns, &["[].email", "[].fullName", "[].userId"]);
    let controls = vec![
        control_for(&analysis.columns, "[].email", DataType::Email),
        control_for(&analysis.columns, "[].fullName", DataType::FullName),
        control_for(&analysis.columns, "[].userId", DataType::NumericId),
    ];

    c.bench_function("analyze_pasted_json_1k", |b| {
        b.iter(|| {
            direct_input::analyze_paste_data(PasteAnalyzeParams {
                content: black_box(content.clone()),
                format: PasteDataFormat::Json,
                sample_row_count: 1_000,
            })
            .expect("JSON analysis should succeed")
        })
    });

    c.bench_function("transform_pasted_json_1k", |b| {
        b.iter(|| {
            direct_input::transform_paste_data(PasteTransformParams {
                content: black_box(content.clone()),
                format: PasteDataFormat::Json,
                columns: selected_columns.clone(),
                controls: controls.clone(),
                deterministic: true,
                seed: "bench-seed".to_string(),
                preview_smart_replacements: Vec::new(),
            })
            .expect("JSON transform should succeed")
        })
    });
}

fn bench_pasted_xml(c: &mut Criterion) {
    let content = build_xml_payload(1_000);
    let analysis = direct_input::analyze_paste_data(PasteAnalyzeParams {
        content: content.clone(),
        format: PasteDataFormat::Xml,
        sample_row_count: 1_000,
    })
    .expect("benchmark XML analysis should succeed");
    let selected_columns =
        columns_for(&analysis.columns, &["users.user.@email", "users.user.name"]);
    let controls = vec![
        control_for(&analysis.columns, "users.user.@email", DataType::Email),
        control_for(&analysis.columns, "users.user.name", DataType::FullName),
    ];

    c.bench_function("analyze_pasted_xml_1k", |b| {
        b.iter(|| {
            direct_input::analyze_paste_data(PasteAnalyzeParams {
                content: black_box(content.clone()),
                format: PasteDataFormat::Xml,
                sample_row_count: 1_000,
            })
            .expect("XML analysis should succeed")
        })
    });

    c.bench_function("transform_pasted_xml_1k", |b| {
        b.iter(|| {
            direct_input::transform_paste_data(PasteTransformParams {
                content: black_box(content.clone()),
                format: PasteDataFormat::Xml,
                columns: selected_columns.clone(),
                controls: controls.clone(),
                deterministic: true,
                seed: "bench-seed".to_string(),
                preview_smart_replacements: Vec::new(),
            })
            .expect("XML transform should succeed")
        })
    });
}

fn bench_pasted_logs(c: &mut Criterion) {
    let content = build_log_payload(1_000);
    let analysis = direct_input::analyze_paste_data(PasteAnalyzeParams {
        content: content.clone(),
        format: PasteDataFormat::Logs,
        sample_row_count: 1_000,
    })
    .expect("benchmark log analysis should succeed");
    let selected_columns = columns_for(&analysis.columns, &["email", "ipAddress"]);
    let controls = vec![
        control_for(&analysis.columns, "email", DataType::Email),
        control_for(&analysis.columns, "ipAddress", DataType::IpAddress),
    ];

    c.bench_function("analyze_pasted_logs_1k", |b| {
        b.iter(|| {
            direct_input::analyze_paste_data(PasteAnalyzeParams {
                content: black_box(content.clone()),
                format: PasteDataFormat::Logs,
                sample_row_count: 1_000,
            })
            .expect("log analysis should succeed")
        })
    });

    c.bench_function("transform_pasted_logs_1k", |b| {
        b.iter(|| {
            direct_input::transform_paste_data(PasteTransformParams {
                content: black_box(content.clone()),
                format: PasteDataFormat::Logs,
                columns: selected_columns.clone(),
                controls: controls.clone(),
                deterministic: true,
                seed: "bench-seed".to_string(),
                preview_smart_replacements: Vec::new(),
            })
            .expect("log transform should succeed")
        })
    });
}

fn control_for(
    columns: &[csv_anonymizer_core::ColumnMetadata],
    name: &str,
    data_type: DataType,
) -> ColumnControl {
    let column = columns
        .iter()
        .find(|column| column.name == name)
        .unwrap_or_else(|| panic!("benchmark column {name} should exist"));
    ColumnControl {
        column_index: column.index,
        type_override: Some(data_type),
        strategy: AnonymizationStrategy::Auto,
    }
}

fn columns_for(columns: &[csv_anonymizer_core::ColumnMetadata], names: &[&str]) -> Vec<usize> {
    names
        .iter()
        .map(|name| {
            columns
                .iter()
                .find(|column| column.name == *name)
                .unwrap_or_else(|| panic!("benchmark column {name} should exist"))
                .index
        })
        .collect()
}

fn build_json_payload(rows: usize) -> String {
    let records = (0..rows)
        .map(|index| {
            format!(
                r#"{{"userId":{index},"email":"user{index}@example.com","fullName":"Person {index}","active":{}}}"#,
                index % 2 == 0
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{records}]")
}

fn build_xml_payload(rows: usize) -> String {
    let mut content = String::from("<users>");
    for index in 0..rows {
        content.push_str(&format!(
            r#"<user id="{index}" email="user{index}@example.com"><name>Person {index}</name><active>{}</active></user>"#,
            index % 2 == 0
        ));
    }
    content.push_str("</users>");
    content
}

fn build_log_payload(rows: usize) -> String {
    (0..rows)
        .map(|index| {
            format!(
                "2026-06-25T12:{:02}:00 INFO user=user{}@example.com ip=10.{}.{}.{} request_id={:08x}-0000-4000-8000-{:012x}",
                index % 60,
                index,
                index % 255,
                (index / 255) % 255,
                (index % 253) + 1,
                index,
                index
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

criterion_group!(
    benches,
    bench_pasted_json,
    bench_pasted_xml,
    bench_pasted_logs
);
criterion_main!(benches);
