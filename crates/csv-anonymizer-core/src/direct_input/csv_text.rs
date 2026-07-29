use crate::csv_io::{
    process_csv_text, read_csv_detection_sample_from_str, read_csv_sample_from_str,
};
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
    PreviewSelection, bounded_preview_sample_count, display_row_count, paste_detection_sample_rows,
    prepare_selected_metadata, preview_rows_with_smart_provider,
    preview_smart_replacements_for_transform,
};

pub(super) fn analyze_csv_text(content: &str, sample_row_count: usize) -> Result<PasteAnalyzeData> {
    let sample_row_count = paste_detection_sample_rows(sample_row_count)?;
    // Spread the sample over the whole paste: pasted content can exceed the
    // sample cap, and a head window would leave detection blind to values that
    // only appear in the tail.
    let sample = read_csv_detection_sample_from_str(content, sample_row_count)?;
    let columns = build_column_metadata(&sample.headers, &sample.rows);

    Ok(PasteAnalyzeData {
        format: PasteDataFormat::Csv,
        row_count: sample.data_rows_scanned,
        row_count_is_complete: sample.scanned_entire_input,
        columns,
    })
}

pub(super) fn preview_csv_text_with_smart_provider(
    input: PastePreviewParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    // Detect on the same rows, and the same number of them, as `analyze_csv_text`
    // and the transform. A head window here would classify only the paste's
    // opening rows, and the display count is no basis for classifying at all, so
    // either one lets a preview of a long paste show a different type — and
    // therefore a different strategy — than the run then applies.
    let detection_sample = read_csv_detection_sample_from_str(
        &input.content,
        paste_detection_sample_rows(input.sample_row_count)?,
    )?;
    let metadata = build_column_metadata(&detection_sample.headers, &detection_sample.rows);
    // Displayed rows stay head-anchored: the user expects the preview to show
    // the paste's opening rows, not the detection spread.
    let display = read_csv_sample_from_str(&input.content, display_row_count(sample_count))?;
    preview_rows_with_smart_provider(
        &display.rows,
        &metadata,
        // A pasted CSV is read whole, so this is the paste's true row count.
        detection_sample.data_rows_scanned,
        PreviewSelection {
            columns: &input.columns,
            controls: &input.controls,
            sample_count,
            provider,
        },
    )
}

pub(super) fn transform_csv_text_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let analysis = analyze_csv_text(&input.content, input.sample_row_count)?;
    let metadata = prepare_selected_metadata(&analysis.columns, &input.columns, &input.controls)?;
    let rows = read_csv_sample_from_str(&input.content, usize::MAX)?.rows;
    let existing_smart_replacements = preview_smart_replacements_for_transform(&input, &metadata);
    let smart_replacements = prepare_smart_replacements_from_rows(
        &rows,
        &metadata,
        existing_smart_replacements.as_ref(),
        provider,
    )?;
    let smart_replacements = smart_replacements
        .has_activity()
        .then_some(smart_replacements);
    let start_time = Instant::now();
    let (output, result) = process_csv_text(
        &input.content,
        &metadata,
        ProcessOptions {
            smart_replacements: smart_replacements.as_ref(),
            mapping_entry_ceiling: None,
        },
    )?;

    Ok(PasteTransformData {
        output,
        row_count: result.row_count,
        columns_anonymized: count_transforming_selected_columns(&metadata),
        duration_ms: start_time.elapsed().as_millis(),
        privacy_report: build_privacy_report(&metadata, result.transform_report),
    })
}
