use crate::csv_io::{count_csv_data_rows_from_reader, process_csv_text, read_csv_sample_from_str};
use crate::error::Result;
use crate::metadata::build_column_metadata;
use crate::service::{build_privacy_report, count_transforming_selected_columns};
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::types::{
    PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, ProcessOptions,
};
use std::time::Instant;

use super::shared::{
    PreviewSelection, bounded_analysis_sample_count, bounded_preview_sample_count,
    prepare_selected_metadata, preview_rows_with_smart_provider,
    preview_smart_replacements_for_transform,
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

pub(super) fn preview_csv_text_with_smart_provider(
    input: PastePreviewParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let sample = read_csv_sample_from_str(&input.content, sample_count.saturating_mul(2).max(1))?;
    let metadata = build_column_metadata(&sample.headers, &sample.rows);
    preview_rows_with_smart_provider(
        &sample.rows,
        &metadata,
        PreviewSelection {
            columns: &input.columns,
            controls: &input.controls,
            sample_count,
            deterministic: input.deterministic,
            seed: &input.seed,
            provider,
        },
    )
}

pub(super) fn transform_csv_text_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let analysis = analyze_csv_text(&input.content, 100)?;
    let metadata = prepare_selected_metadata(&analysis.columns, &input.columns, &input.controls)?;
    let rows = read_csv_sample_from_str(&input.content, usize::MAX)?.rows;
    let existing_smart_replacements = preview_smart_replacements_for_transform(&input, &metadata);
    let smart_replacements = prepare_smart_replacements_from_rows(
        &rows,
        &metadata,
        input.deterministic,
        &input.seed,
        existing_smart_replacements.as_ref(),
        provider,
    )?;
    let smart_replacements = (!smart_replacements.is_empty()).then_some(smart_replacements);
    let start_time = Instant::now();
    let (output, result) = process_csv_text(
        &input.content,
        &metadata,
        ProcessOptions {
            deterministic: input.deterministic,
            seed: &input.seed,
            smart_replacements: smart_replacements.as_ref(),
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
