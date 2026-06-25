use crate::csv_io::{count_csv_data_rows_from_reader, process_csv_text, read_csv_sample_from_str};
use crate::error::Result;
use crate::metadata::build_column_metadata;
use crate::service::{build_privacy_report, count_transforming_selected_columns};
use crate::types::{
    PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, ProcessOptions,
};
use std::time::Instant;

use super::shared::{
    bounded_analysis_sample_count, bounded_preview_sample_count, prepare_selected_metadata,
    preview_rows,
};

pub(super) fn analyze_csv_text(content: &str, sample_row_count: usize) -> Result<PasteAnalyzeData> {
    let sample_row_count = bounded_analysis_sample_count(sample_row_count)?;
    let sample = read_csv_sample_from_str(content, sample_row_count)?;
    let row_count = count_csv_data_rows_from_reader(content.as_bytes())?;
    let columns = build_column_metadata(&sample.headers, &sample.rows);

    Ok(PasteAnalyzeData {
        format: PasteDataFormat::Csv,
        row_count,
        row_count_is_complete: true,
        columns,
    })
}

pub(super) fn preview_csv_text(input: PastePreviewParams) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let sample = read_csv_sample_from_str(&input.content, sample_count.saturating_mul(2).max(1))?;
    let metadata = build_column_metadata(&sample.headers, &sample.rows);
    preview_rows(
        &sample.rows,
        &metadata,
        &input.columns,
        &input.controls,
        input.deterministic,
        &input.seed,
        sample_count,
    )
}

pub(super) fn transform_csv_text(input: PasteTransformParams) -> Result<PasteTransformData> {
    let analysis = analyze_csv_text(&input.content, 100)?;
    let metadata = prepare_selected_metadata(&analysis.columns, &input.columns, &input.controls)?;
    let start_time = Instant::now();
    let (output, result) = process_csv_text(
        &input.content,
        &metadata,
        ProcessOptions {
            deterministic: input.deterministic,
            seed: &input.seed,
            smart_replacements: None,
        },
    )?;

    Ok(PasteTransformData {
        output,
        row_count: result.row_count,
        columns_anonymized: count_transforming_selected_columns(&metadata),
        duration_ms: start_time.elapsed().as_millis(),
        privacy_report: build_privacy_report(
            &metadata,
            result.transform_report,
            input.deterministic,
        ),
    })
}
