use crate::detection::collect_privacy_spans;
use crate::error::{AnonymizerError, Result};
use crate::service::select_columns;
use crate::smart::SmartReplacementProvider;
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{
    DataType, PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, TransformContext,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use super::shared::{
    FieldSampleLimits, FieldSamples, PASTE_MAX_TEXT_MATCHES, PreviewSelection,
    analysis_from_fields, bounded_preview_sample_count, next_row_index,
    paste_detection_sample_rows, paste_transform_data, preview_field_sample_limits,
    preview_from_fields_with_smart_provider, push_typed_field_sample, selected_columns_by_source,
    smart_replacements_for_fields,
};

pub(super) fn analyze_text_content(
    content: &str,
    format: PasteDataFormat,
    sample_row_count: usize,
) -> Result<PasteAnalyzeData> {
    let sample_row_count = paste_detection_sample_rows(sample_row_count)?;
    let matches = collect_text_matches(content)?;
    let fields = text_fields_from_matches(
        &matches,
        FieldSampleLimits::detection_only(sample_row_count),
    )?;
    // Free text is the input the coverage disclosure matters most on: a long log whose
    // detection window is thinned would otherwise report its types with no caveat, and
    // the user would find out only after the output existed. It comes with the analysis
    // here rather than being computed separately.
    Ok(analysis_from_fields(format, &fields, matches.len()).0)
}

pub(super) fn preview_text_content_with_smart_provider(
    input: PastePreviewParams,
    _format: PasteDataFormat,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let detection_sample_rows = paste_detection_sample_rows(input.sample_row_count)?;
    let matches = collect_text_matches(&input.content)?;
    let fields = text_fields_from_matches(
        &matches,
        preview_field_sample_limits(sample_count, detection_sample_rows),
    )?;
    preview_from_fields_with_smart_provider(
        &fields,
        PreviewSelection::from_params(&input, sample_count, provider),
    )
}

pub(super) fn transform_text_with_smart_provider(
    input: PasteTransformParams,
    format: PasteDataFormat,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let detection_sample_rows = paste_detection_sample_rows(input.sample_row_count)?;
    let matches = collect_text_matches(&input.content)?;
    let fields = text_fields_from_matches(
        &matches,
        FieldSampleLimits::detection_only(detection_sample_rows),
    )?;
    let (analysis, coverage) = analysis_from_fields(format, &fields, matches.len());
    let metadata = select_columns(&analysis.columns, &input.columns, &input.controls)?;
    let selected_by_name = selected_columns_by_source(&metadata);
    // Every match, not the detection window: a span the sample dropped would reach the
    // transform without a replacement of its own.
    let smart_fields = text_fields_from_matches(
        &matches,
        FieldSampleLimits::detection_only(matches.len().max(1)),
    )?;
    let smart_replacements =
        smart_replacements_for_fields(&smart_fields, &metadata, &input, provider)?;
    let start_time = Instant::now();
    let mut state = TransformState::with_smart_replacements_if_active(smart_replacements);
    let mut row_indices = HashMap::new();
    let mut output = String::with_capacity(input.content.len());
    let mut last_end = 0;

    for token_match in matches {
        output.push_str(&input.content[last_end..token_match.start]);
        if let Some(column) = selected_by_name.get(token_match.name) {
            let row_index = next_row_index(&mut row_indices, token_match.name);
            let context = TransformContext::for_column(column, row_index);
            output.push_str(&transform_value_with_state(
                token_match.value,
                column,
                &context,
                &mut state,
            ));
        } else {
            output.push_str(token_match.value);
        }
        last_end = token_match.end;
    }
    output.push_str(&input.content[last_end..]);

    Ok(paste_transform_data(
        output,
        analysis.row_count,
        &metadata,
        state.report(),
        coverage,
        start_time,
    ))
}
struct TextMatch<'a> {
    name: &'static str,
    data_type: DataType,
    start: usize,
    end: usize,
    value: &'a str,
}

pub(super) fn looks_like_logs(content: &str) -> bool {
    content.lines().take(20).any(|line| {
        timestamp_regex().is_match(line)
            || log_level_regex().is_match(line)
            || line.contains(" request_id=")
            || line.contains(" trace_id=")
    })
}

fn text_fields_from_matches(
    matches: &[TextMatch<'_>],
    limits: FieldSampleLimits,
) -> Result<Vec<FieldSamples>> {
    let mut fields = Vec::new();
    for token_match in matches {
        push_typed_field_sample(
            &mut fields,
            token_match.name,
            token_match.data_type,
            token_match.value,
            limits,
        )?;
    }
    Ok(fields)
}

fn collect_text_matches(content: &str) -> Result<Vec<TextMatch<'_>>> {
    // collect_privacy_spans already returns start-sorted, non-overlapping
    // spans, so only the total cap needs enforcing here.
    let mut matches = Vec::new();
    for span in collect_privacy_spans(content) {
        if matches.len() >= PASTE_MAX_TEXT_MATCHES {
            return Err(AnonymizerError::input_parse(
                "pasted data",
                format!(
                    "Detected more than {PASTE_MAX_TEXT_MATCHES} text values. Use a smaller paste or the CSV file workflow."
                ),
            ));
        }
        matches.push(TextMatch {
            name: span.field_name,
            data_type: span.data_type,
            start: span.start,
            end: span.end,
            value: span.value,
        });
    }
    Ok(matches)
}

fn timestamp_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\b\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?)?\b").unwrap()
    })
}

fn log_level_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"\b(?:TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL)\b").unwrap())
}
