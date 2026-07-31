use crate::detection::{CandidateDetector, collect_privacy_spans};
use crate::error::{AnonymizerError, Result};
use crate::prepared_snapshot::{PreparedAnalysisSnapshot, PreparedSnapshotError};
use crate::service::select_columns;
use crate::smart::SmartReplacementProvider;
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, ColumnPreview, DataType, DetectionCoverage,
    PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, PrivacyFindingKind, SampleTransform, SmartReplacementEntry,
    TransformContext, TransformReport,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use super::shared::{
    FieldSampleLimits, FieldSamples, PASTE_MAX_TEXT_MATCHES, PreviewSelection,
    analysis_from_fields, analysis_from_fields_with_candidate_detector,
    bounded_preview_sample_count, next_row_index, paste_detection_sample_rows,
    paste_transform_data, preview_field_sample_limits, preview_from_fields_with_smart_provider,
    push_identified_field_sample, push_typed_field_sample, selected_columns_by_source,
    smart_replacements_for_fields,
};

const UNSTRUCTURED_TEXT_COLUMN: &str = "Unstructured text";
const PASTE_SOURCE_IDENTITY: &str = "paste";

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

pub(super) fn analyze_text_content_with_candidate_detector(
    content: &str,
    format: PasteDataFormat,
    sample_row_count: usize,
    detector: &mut dyn CandidateDetector,
) -> Result<PasteAnalyzeData> {
    // A line gives the model useful context and stable replay coordinates. Paste
    // bytes are bounded by the public entry point, so every line is retained.
    let line_count = content.split('\n').count();
    let limits = FieldSampleLimits::detection_only(line_count.max(1));
    let mut fields = Vec::with_capacity(1);
    for line in content.split('\n') {
        push_identified_field_sample(
            &mut fields,
            Some("$text"),
            UNSTRUCTURED_TEXT_COLUMN,
            line,
            limits,
        )?;
    }
    let (mut analysis, _) =
        analysis_from_fields_with_candidate_detector(format, &fields, line_count, Some(detector));
    if matches!(
        analysis.detection_run_summary.local_ner,
        crate::types::LocalNerRunStatus::Completed | crate::types::LocalNerRunStatus::Incomplete
    ) {
        analysis.prepared_analysis = Some(
            PreparedAnalysisSnapshot::new(
                PASTE_SOURCE_IDENTITY,
                format_name(format),
                content.as_bytes(),
                sample_row_count,
                analysis.columns.clone(),
                &analysis.detection_run_summary,
            )
            .map_err(snapshot_error)?,
        );
    }
    Ok(analysis)
}

/// Applies explicitly confirmed model spans without invoking the model again.
pub(super) fn replay_text_candidate_evidence(
    input: &PasteTransformParams,
    format: PasteDataFormat,
    snapshot: &PreparedAnalysisSnapshot,
    confirmed_candidate_ids: &[String],
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let start_time = Instant::now();
    let replay =
        execute_text_candidate_replay(input, format, snapshot, confirmed_candidate_ids, provider)?;
    let row_count = input.content.split('\n').count();
    Ok(paste_transform_data(
        replay.output,
        row_count,
        &replay.columns,
        replay.report,
        DetectionCoverage::values(row_count, row_count),
        start_time,
    ))
}

pub(super) fn preview_text_candidate_evidence(
    input: &PastePreviewParams,
    format: PasteDataFormat,
    snapshot: &PreparedAnalysisSnapshot,
    confirmed_candidate_ids: &[String],
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let replay_input = PasteTransformParams {
        content: input.content.clone(),
        format,
        columns: input.columns.clone(),
        controls: input.controls.clone(),
        sample_row_count: input.sample_row_count,
        preview_smart_replacements: Vec::new(),
    };
    let replay = execute_text_candidate_replay(
        &replay_input,
        format,
        snapshot,
        confirmed_candidate_ids,
        provider,
    )?;
    let samples = input
        .content
        .split('\n')
        .zip(replay.output.split('\n'))
        .take(sample_count)
        .map(|(original, anonymized)| SampleTransform {
            original: original.to_string(),
            anonymized: anonymized.to_string(),
        })
        .collect::<Vec<_>>();
    let previews = replay
        .columns
        .iter()
        .filter(|column| column.is_selected)
        .map(|column| ColumnPreview {
            column_index: column.index,
            column_name: column.name.clone(),
            samples: samples.clone(),
        })
        .collect();
    Ok(PreviewData {
        previews,
        warnings: Vec::new(),
        smart_replacements: replay.smart_replacements,
    })
}

struct TextCandidateReplay {
    output: String,
    columns: Vec<ColumnMetadata>,
    report: TransformReport,
    smart_replacements: Vec<SmartReplacementEntry>,
}

fn execute_text_candidate_replay(
    input: &PasteTransformParams,
    format: PasteDataFormat,
    snapshot: &PreparedAnalysisSnapshot,
    confirmed_candidate_ids: &[String],
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<TextCandidateReplay> {
    let validated = snapshot
        .validate(
            PASTE_SOURCE_IDENTITY,
            format_name(format),
            input.content.as_bytes(),
            input.sample_row_count,
            confirmed_candidate_ids,
        )
        .map_err(snapshot_error)?;
    validated
        .columns_for_selection(&input.columns)
        .map_err(snapshot_error)?;
    let mut columns = select_columns(validated.columns(), &input.columns, &input.controls)?;
    for column in &mut columns {
        // Explicitly confirming a candidate and selecting its column is the user's
        // authorization to redact it. Preserve any explicit strategy control.
        if column.is_selected
            && column.strategy == AnonymizationStrategy::PassThrough
            && !input
                .controls
                .iter()
                .any(|control| control.column_index == column.index)
        {
            column.strategy = AnonymizationStrategy::Redact;
        }
    }
    let selected = columns
        .iter()
        .filter(|column| column.is_selected)
        .map(|column| (column.index, column))
        .collect::<HashMap<_, _>>();
    let line_starts = line_start_offsets(&input.content);
    let deterministic = collect_privacy_spans(&input.content);
    let mut spans = Vec::new();

    // The text analysis has one review column. Selecting it authorizes both the
    // deterministic findings the user already saw and the confirmed learned spans.
    if let Some(column) = selected.values().next().copied() {
        spans.extend(
            deterministic
                .iter()
                .map(|item| (item.start, item.end, column, item.data_type, false)),
        );
    }
    for evidence in &snapshot.candidate_evidence {
        if !validated.confirmed_candidate_ids().contains(&evidence.id) {
            continue;
        }
        let Some(column) = selected.get(&evidence.column_index) else {
            continue;
        };
        let (start, end) = evidence_global_byte_span(
            &input.content,
            &line_starts,
            evidence.row_index,
            evidence.start,
            evidence.end,
        )
        .ok_or_else(|| snapshot_error(PreparedSnapshotError::InvalidEvidence))?;
        if input.content.get(start..end) != Some(evidence.match_value.as_str())
            || deterministic
                .iter()
                .any(|item| start < item.end && item.start < end)
        {
            return Err(snapshot_error(PreparedSnapshotError::InvalidEvidence));
        }
        let data_type = match evidence.kind {
            PrivacyFindingKind::Person => DataType::FullName,
            PrivacyFindingKind::PrivateAddress => DataType::Address,
            _ => column.detected_type,
        };
        spans.push((start, end, *column, data_type, true));
    }
    spans.sort_by_key(|(start, end, _, _, _)| (*start, *end));
    if spans.windows(2).any(|pair| pair[1].0 < pair[0].1) {
        return Err(snapshot_error(PreparedSnapshotError::InvalidEvidence));
    }

    let limits = FieldSampleLimits::detection_only(spans.len().max(1));
    let mut replacement_fields = Vec::with_capacity(1);
    for (start, end, _, _, _) in &spans {
        push_identified_field_sample(
            &mut replacement_fields,
            Some("$text"),
            UNSTRUCTURED_TEXT_COLUMN,
            &input.content[*start..*end],
            limits,
        )?;
    }
    let smart_replacements =
        smart_replacements_for_fields(&replacement_fields, &columns, input, provider)?;
    let smart_replacement_entries = smart_replacements.to_entries();
    let mut output = String::with_capacity(input.content.len());
    let mut last_end = 0;
    let mut state = TransformState::with_smart_replacements_if_active(smart_replacements);
    for (row_index, (start, end, column, data_type, _candidate)) in spans.into_iter().enumerate() {
        output.push_str(&input.content[last_end..start]);
        let mut replay_column = column.clone();
        replay_column.detected_type = data_type;
        output.push_str(&transform_value_with_state(
            &input.content[start..end],
            &replay_column,
            &TransformContext::for_column(&replay_column, row_index),
            &mut state,
        ));
        last_end = end;
    }
    output.push_str(&input.content[last_end..]);
    Ok(TextCandidateReplay {
        output,
        columns,
        report: state.report(),
        smart_replacements: smart_replacement_entries,
    })
}

fn evidence_global_byte_span(
    content: &str,
    line_starts: &[usize],
    row_index: usize,
    start_utf16: usize,
    end_utf16: usize,
) -> Option<(usize, usize)> {
    let line_start = *line_starts.get(row_index)?;
    let line_end = content[line_start..]
        .find('\n')
        .map_or(content.len(), |offset| line_start + offset);
    let line = &content[line_start..line_end];
    let start = utf16_to_byte(line, start_utf16)?;
    let end = utf16_to_byte(line, end_utf16)?;
    (start < end).then_some((line_start + start, line_start + end))
}

fn utf16_to_byte(value: &str, target: usize) -> Option<usize> {
    let mut offset = 0;
    for (byte, character) in value.char_indices() {
        if offset == target {
            return Some(byte);
        }
        offset += character.len_utf16();
        if offset > target {
            return None;
        }
    }
    (offset == target).then_some(value.len())
}

fn line_start_offsets(content: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(content.match_indices('\n').map(|(index, _)| index + 1))
        .collect()
}

fn format_name(format: PasteDataFormat) -> &'static str {
    match format {
        PasteDataFormat::PlainText => "plainText",
        PasteDataFormat::Logs => "logs",
        _ => "unsupported",
    }
}

fn snapshot_error(error: PreparedSnapshotError) -> AnonymizerError {
    AnonymizerError::input_parse("prepared text analysis", error.to_string())
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
