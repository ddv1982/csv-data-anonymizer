use crate::csv_io::{
    process_csv_data, read_csv_detection_sample_from_str, read_csv_sample_from_str,
};
use crate::detection::CandidateDetector;
use crate::error::Result;
use crate::metadata::{build_column_metadata, build_column_metadata_with_candidate_detector};
use crate::service::{display_row_count, select_columns};
use crate::smart::{
    SmartReplacementProvider, prepare_smart_replacements_from_rows,
    reusable_preview_smart_replacements,
};
use crate::types::{
    DetectionCoverage, PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, ProcessOptions,
};
use std::time::Instant;

use super::shared::{
    PreviewSelection, bounded_preview_sample_count, paste_detection_sample_rows,
    paste_transform_data, preview_rows_with_smart_provider,
};

pub(super) fn analyze_csv_text(content: &str, sample_row_count: usize) -> Result<PasteAnalyzeData> {
    analyze_csv_text_with_coverage(content, sample_row_count).map(|(analysis, _)| analysis)
}

pub(super) fn analyze_csv_text_with_candidate_detector(
    content: &str,
    sample_row_count: usize,
    detector: &mut dyn CandidateDetector,
) -> Result<PasteAnalyzeData> {
    analyze_csv_text_with_coverage_and_candidate_detector(content, sample_row_count, Some(detector))
        .map(|(analysis, _)| analysis)
}

/// [`analyze_csv_text`] plus how much of the paste it classified, in the crate's own
/// coverage type.
///
/// Still split even though the DTO now carries a summary of the same figures: the
/// transform path feeds `build_privacy_report`, which takes a [`DetectionCoverage`],
/// and rebuilding one from the wire summary would re-open the invariant that type's
/// constructor exists to hold.
fn analyze_csv_text_with_coverage(
    content: &str,
    sample_row_count: usize,
) -> Result<(PasteAnalyzeData, DetectionCoverage)> {
    analyze_csv_text_with_coverage_and_candidate_detector(content, sample_row_count, None)
}

fn analyze_csv_text_with_coverage_and_candidate_detector(
    content: &str,
    sample_row_count: usize,
    detector: Option<&mut dyn CandidateDetector>,
) -> Result<(PasteAnalyzeData, DetectionCoverage)> {
    let sample_row_count = paste_detection_sample_rows(sample_row_count)?;
    // Spread the sample over the whole paste: pasted content can exceed the
    // sample cap, and a head window would leave detection blind to values that
    // only appear in the tail.
    let sample = read_csv_detection_sample_from_str(content, sample_row_count)?;
    let (columns, detector_status) =
        build_column_metadata_with_candidate_detector(&sample.headers, &sample.rows, detector);
    let coverage = DetectionCoverage::from_detection_sample(&sample);

    Ok((
        PasteAnalyzeData {
            format: PasteDataFormat::Csv,
            row_count: sample.data_rows_scanned,
            row_count_is_complete: sample.scanned_entire_input,
            detection_coverage: coverage.summary(),
            detection_run_summary: crate::service::detection_run_summary(detector_status),
            columns,
            prepared_analysis: None,
        },
        coverage,
    ))
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
        PreviewSelection::from_params(&input, sample_count, provider),
    )
}

pub(super) fn transform_csv_text_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let (analysis, coverage) =
        analyze_csv_text_with_coverage(&input.content, input.sample_row_count)?;
    let metadata = select_columns(&analysis.columns, &input.columns, &input.controls)?;
    let rows = read_csv_sample_from_str(&input.content, usize::MAX)?.rows;
    let existing_smart_replacements =
        reusable_preview_smart_replacements(&input.preview_smart_replacements, &metadata);
    let smart_replacements = prepare_smart_replacements_from_rows(
        &rows,
        &metadata,
        existing_smart_replacements.as_ref(),
        provider,
    )?
    .if_active();
    let start_time = Instant::now();
    let (output, result) = process_csv_data(
        &input.content,
        &metadata,
        ProcessOptions {
            smart_replacements: smart_replacements.as_ref(),
            mapping_entry_ceiling: None,
        },
    )?;

    Ok(paste_transform_data(
        output,
        result.row_count,
        &metadata,
        result.transform_report,
        coverage,
        start_time,
    ))
}
