use crate::error::{AnonymizerError, Result};
use crate::service::{build_privacy_report, count_transforming_selected_columns};
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::transform_value_with_state;
use crate::types::{
    DataType, PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, TransformContext,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use super::shared::{
    FieldSamples, PASTE_MAX_TEXT_CANDIDATES, PASTE_MAX_TEXT_MATCHES, PreviewSelection,
    bounded_analysis_sample_count, bounded_preview_sample_count, fields_to_rows,
    metadata_from_fields, next_row_index, prepare_selected_metadata,
    preview_from_fields_with_smart_provider, push_typed_field_sample, selected_columns_by_source,
    transform_state_for_smart_replacements,
};

pub(super) fn analyze_text_content(
    content: &str,
    format: PasteDataFormat,
    sample_row_count: usize,
) -> Result<PasteAnalyzeData> {
    let sample_row_count = bounded_analysis_sample_count(sample_row_count)?;
    let matches = collect_text_matches(content)?;
    let fields = text_fields_from_matches(&matches, sample_row_count)?;
    let (headers, rows) = fields_to_rows(&fields, sample_row_count);
    let columns = metadata_from_fields(&fields, &headers, &rows);

    Ok(PasteAnalyzeData {
        format,
        row_count: matches.len(),
        row_count_is_complete: true,
        columns,
    })
}

pub(super) fn preview_text_content_with_smart_provider(
    input: PastePreviewParams,
    _format: PasteDataFormat,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let matches = collect_text_matches(&input.content)?;
    let fields = text_fields_from_matches(&matches, sample_count.saturating_mul(2).max(1))?;
    preview_from_fields_with_smart_provider(
        &fields,
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

pub(super) fn transform_text_with_smart_provider(
    input: PasteTransformParams,
    format: PasteDataFormat,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let matches = collect_text_matches(&input.content)?;
    let fields = text_fields_from_matches(&matches, 100)?;
    let (headers, rows) = fields_to_rows(&fields, 100);
    let analysis = PasteAnalyzeData {
        format,
        row_count: matches.len(),
        row_count_is_complete: true,
        columns: metadata_from_fields(&fields, &headers, &rows),
    };
    let metadata = prepare_selected_metadata(&analysis.columns, &input.columns, &input.controls)?;
    let selected_by_name = selected_columns_by_source(&metadata);
    let smart_fields = text_fields_from_matches(&matches, matches.len().max(1))?;
    let (_headers, smart_rows) = fields_to_rows(&smart_fields, matches.len().max(1));
    let smart_replacements = prepare_smart_replacements_from_rows(
        &smart_rows,
        &metadata,
        input.deterministic,
        &input.seed,
        provider,
    )?;
    let start_time = Instant::now();
    let mut state = transform_state_for_smart_replacements(
        input.deterministic,
        &input.seed,
        smart_replacements,
    );
    let mut row_indices = HashMap::new();
    let mut output = String::with_capacity(input.content.len());
    let mut last_end = 0;

    for token_match in matches {
        output.push_str(&input.content[last_end..token_match.start]);
        if let Some(column) = selected_by_name.get(token_match.name) {
            let row_index = next_row_index(&mut row_indices, token_match.name);
            let context = TransformContext {
                column_name: &column.name,
                column_index: column.index,
                row_index,
                seed: &input.seed,
                deterministic: input.deterministic,
                empty_format: column.empty_format,
            };
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

    Ok(PasteTransformData {
        output,
        row_count: analysis.row_count,
        columns_anonymized: count_transforming_selected_columns(&metadata),
        duration_ms: start_time.elapsed().as_millis(),
        privacy_report: build_privacy_report(&metadata, state.report(), input.deterministic),
    })
}
struct TextMatch<'a> {
    name: &'static str,
    data_type: DataType,
    start: usize,
    end: usize,
    value: &'a str,
    priority: usize,
}

struct TextTokenSpec {
    name: &'static str,
    data_type: DataType,
    regex: &'static Regex,
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
    sample_count: usize,
) -> Result<Vec<FieldSamples>> {
    let mut fields = Vec::new();
    for token_match in matches {
        push_typed_field_sample(
            &mut fields,
            token_match.name,
            token_match.data_type,
            token_match.value,
            sample_count,
        )?;
    }
    Ok(fields)
}

fn collect_text_matches(content: &str) -> Result<Vec<TextMatch<'_>>> {
    let mut candidates = Vec::new();
    for (priority, spec) in text_token_specs().iter().enumerate() {
        for regex_match in spec.regex.find_iter(content) {
            if candidates.len() >= PASTE_MAX_TEXT_CANDIDATES {
                return Err(AnonymizerError::input_parse(
                    "pasted data",
                    format!(
                        "Detected more than {PASTE_MAX_TEXT_CANDIDATES} text token candidates. Use a smaller paste or the CSV file workflow."
                    ),
                ));
            }
            candidates.push(TextMatch {
                name: spec.name,
                data_type: spec.data_type,
                start: regex_match.start(),
                end: regex_match.end(),
                value: regex_match.as_str(),
                priority,
            });
        }
    }

    candidates.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then(left.priority.cmp(&right.priority))
            .then((right.end - right.start).cmp(&(left.end - left.start)))
    });

    let mut selected = Vec::new();
    let mut last_end = 0;
    for candidate in candidates {
        if candidate.start < last_end {
            continue;
        }
        if selected.len() >= PASTE_MAX_TEXT_MATCHES {
            return Err(AnonymizerError::input_parse(
                "pasted data",
                format!(
                    "Detected more than {PASTE_MAX_TEXT_MATCHES} text values. Use a smaller paste or the CSV file workflow."
                ),
            ));
        }
        last_end = candidate.end;
        selected.push(candidate);
    }
    Ok(selected)
}

fn text_token_specs() -> [TextTokenSpec; 8] {
    [
        TextTokenSpec {
            name: "email",
            data_type: DataType::Email,
            regex: email_regex(),
        },
        TextTokenSpec {
            name: "url",
            data_type: DataType::Url,
            regex: url_regex(),
        },
        TextTokenSpec {
            name: "uuid",
            data_type: DataType::Uuid,
            regex: uuid_regex(),
        },
        TextTokenSpec {
            name: "timestamp",
            data_type: DataType::Timestamp,
            regex: timestamp_regex(),
        },
        TextTokenSpec {
            name: "ipAddress",
            data_type: DataType::IpAddress,
            regex: ip_address_regex(),
        },
        TextTokenSpec {
            name: "macAddress",
            data_type: DataType::MacAddress,
            regex: mac_address_regex(),
        },
        TextTokenSpec {
            name: "taxId",
            data_type: DataType::TaxId,
            regex: tax_id_regex(),
        },
        TextTokenSpec {
            name: "phone",
            data_type: DataType::Phone,
            regex: phone_regex(),
        },
    ]
}

fn email_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b").unwrap())
}

fn url_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r#"\b(?:https?://|www\.)[^\s<>'"]+"#).unwrap())
}

fn uuid_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
        )
        .unwrap()
    })
}

fn timestamp_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\b\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?)?\b").unwrap()
    })
}

fn ip_address_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)\b")
            .unwrap()
    })
}

fn mac_address_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}\b").unwrap())
}

fn tax_id_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b(?:\d{3}-\d{2}-\d{4}|\d{2}-\d{7})\b").unwrap())
}

fn phone_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\b(?:\+\d{1,3}[\s.-]?)?(?:\(?\d{3}\)?[\s.-]?)\d{3}[\s.-]?\d{4}\b").unwrap()
    })
}

fn log_level_regex() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"\b(?:TRACE|DEBUG|INFO|WARN|WARNING|ERROR|FATAL)\b").unwrap())
}
