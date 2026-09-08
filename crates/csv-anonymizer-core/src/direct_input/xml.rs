use crate::detection::CandidateDetector;
use crate::error::{AnonymizerError, Result};
use crate::service::select_columns;
use crate::smart::SmartReplacementProvider;
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{
    ColumnMetadata, DetectionCoverage, PasteAnalyzeData, PasteDataFormat, PastePreviewParams,
    PasteTransformData, PasteTransformParams, PreviewData, TransformContext,
};
use quick_xml::events::{BytesCData, BytesText, Event};
use quick_xml::{Reader, Writer, XmlVersion};
use std::collections::HashMap;
use std::time::Instant;

use super::shared::{
    FieldSampleLimits, FieldSamples, PreviewSelection, analysis_from_fields,
    bounded_preview_sample_count, escape_path_key, format_path, next_row_index,
    paste_detection_sample_rows, paste_transform_data, preview_field_sample_limits,
    preview_from_fields_with_smart_provider, push_identified_field_sample,
    selected_columns_by_source, smart_replacements_for_fields,
};

/// XML analysis plus the coverage used by transforms for the privacy report.
pub(super) fn analyze_xml(
    content: &str,
    sample_row_count: usize,
    detector: Option<&mut dyn CandidateDetector>,
) -> Result<(PasteAnalyzeData, DetectionCoverage)> {
    let sample_row_count = paste_detection_sample_rows(sample_row_count)?;
    let fields = collect_xml_fields(content, FieldSampleLimits::detection_only(sample_row_count))?;
    let row_count = infer_xml_row_count(&fields);
    Ok(analysis_from_fields(
        PasteDataFormat::Xml,
        &fields,
        row_count,
        detector,
    ))
}

pub(super) fn preview_xml_with_smart_provider(
    input: PastePreviewParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
    tokenization_key: Option<&crate::TokenizationKey>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let detection_sample_rows = paste_detection_sample_rows(input.sample_row_count)?;
    let fields = collect_xml_fields(
        &input.content,
        preview_field_sample_limits(sample_count, detection_sample_rows),
    )?;
    preview_from_fields_with_smart_provider(
        &fields,
        PreviewSelection::from_params(&input, sample_count, provider, tokenization_key),
    )
}

pub(super) fn transform_xml_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
    tokenization_key: Option<&crate::TokenizationKey>,
) -> Result<PasteTransformData> {
    let (analysis, coverage) = analyze_xml(&input.content, input.sample_row_count, None)?;
    let metadata = select_columns(&analysis.columns, &input.columns, &input.controls)?;
    let selected_by_path = selected_columns_by_source(&metadata);
    // Collected over every value rather than over the detection window, so each value
    // the run rewrites has a replacement of its own.
    let smart_fields = collect_xml_fields(
        &input.content,
        FieldSampleLimits::detection_only(usize::MAX),
    )?;
    let smart_replacements =
        smart_replacements_for_fields(&smart_fields, &metadata, &input, provider)?;
    let start_time = Instant::now();
    let mut state = TransformState::with_smart_replacements_if_active(smart_replacements)
        .with_tokenization_key(tokenization_key.cloned());
    let output = transform_xml_content(&input.content, &selected_by_path, &mut state)?;

    let mut report = state.report();
    // XML scalar content is transformed node-by-node, not through the row-level audit.
    report.residual_audit_incomplete = true;
    Ok(paste_transform_data(
        output,
        analysis.row_count,
        &metadata,
        report,
        coverage,
        start_time,
    ))
}

pub(super) fn collect_xml_fields(
    content: &str,
    limits: FieldSampleLimits,
) -> Result<Vec<FieldSamples>> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(false);
    let mut path = Vec::new();
    let mut fields = Vec::new();
    let mut pending_scalar = None;

    loop {
        match reader.read_event().map_err(xml_error)? {
            Event::Start(event) => {
                flush_xml_field_sample(&mut pending_scalar, &mut fields, limits)?;
                path.push(xml_name(event.name().as_ref()));
                collect_xml_attributes(&reader, &event, &path, &mut fields, limits)?;
            }
            Event::Empty(event) => {
                flush_xml_field_sample(&mut pending_scalar, &mut fields, limits)?;
                path.push(xml_name(event.name().as_ref()));
                collect_xml_attributes(&reader, &event, &path, &mut fields, limits)?;
                path.pop();
            }
            Event::Text(event) => {
                let value = event
                    .xml_content(XmlVersion::Implicit1_0)
                    .map_err(xml_error)?;
                append_pending_xml_scalar(&mut pending_scalar, &path, value.into_owned(), false);
            }
            Event::CData(event) => {
                let value = event.decode().map_err(xml_error)?;
                append_pending_xml_scalar(&mut pending_scalar, &path, value.into_owned(), true);
            }
            Event::GeneralRef(event) => {
                let value = decode_xml_general_ref(event.as_ref())
                    .ok_or_else(|| xml_error("unsupported XML general reference"))?;
                append_pending_xml_scalar(&mut pending_scalar, &path, value, false);
            }
            Event::End(_) => {
                flush_xml_field_sample(&mut pending_scalar, &mut fields, limits)?;
                path.pop();
            }
            Event::Eof => {
                flush_xml_field_sample(&mut pending_scalar, &mut fields, limits)?;
                break;
            }
            _ => {
                flush_xml_field_sample(&mut pending_scalar, &mut fields, limits)?;
            }
        }
    }

    Ok(fields)
}

fn flush_xml_field_sample(
    pending: &mut Option<PendingXmlScalar>,
    fields: &mut Vec<FieldSamples>,
    limits: FieldSampleLimits,
) -> Result<()> {
    let Some(pending_scalar) = pending.take() else {
        return Ok(());
    };
    push_xml_text_sample(
        fields,
        &pending_scalar.path,
        pending_scalar.raw.trim(),
        limits,
    )
}

fn push_xml_text_sample(
    fields: &mut Vec<FieldSamples>,
    path: &[String],
    value: &str,
    limits: FieldSampleLimits,
) -> Result<()> {
    if value.is_empty() {
        return Ok(());
    }

    let source_path = xml_text_source_path(path);
    let label = xml_text_label(path);
    push_identified_field_sample(fields, Some(&source_path), &label, value, limits)
}

fn collect_xml_attributes(
    reader: &Reader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    path: &[String],
    fields: &mut Vec<FieldSamples>,
    limits: FieldSampleLimits,
) -> Result<()> {
    for attribute in event.attributes().with_checks(false) {
        let attribute = attribute.map_err(xml_error)?;
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
            .map_err(xml_error)?;
        if value.trim().is_empty() {
            continue;
        }
        let key = xml_name(attribute.key.as_ref());
        let source_path = xml_attribute_source_path(path, &key);
        let label = xml_attribute_label(path, &key);
        push_identified_field_sample(fields, Some(&source_path), &label, value.trim(), limits)?;
    }

    Ok(())
}

fn transform_xml_content(
    content: &str,
    selected_by_path: &HashMap<String, ColumnMetadata>,
    state: &mut TransformState,
) -> Result<String> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(false);
    let mut writer = Writer::new(Vec::new());
    let mut path = Vec::new();
    let mut row_indices = HashMap::new();
    let mut transform_context = XmlTransformContext {
        selected_by_path,
        row_indices: &mut row_indices,
        state,
    };

    let mut pending_scalar: Option<PendingXmlScalar> = None;
    loop {
        let event = reader.read_event().map_err(xml_error)?;
        match event {
            Event::Start(event) => {
                flush_pending_xml_scalar(&mut pending_scalar, &mut writer, &mut transform_context)?;
                path.push(xml_name(event.name().as_ref()));
                let event =
                    transform_xml_attributes(&reader, event, &path, &mut transform_context)?;
                writer.write_event(Event::Start(event)).map_err(xml_error)?;
            }
            Event::Empty(event) => {
                flush_pending_xml_scalar(&mut pending_scalar, &mut writer, &mut transform_context)?;
                path.push(xml_name(event.name().as_ref()));
                let event =
                    transform_xml_attributes(&reader, event, &path, &mut transform_context)?;
                writer.write_event(Event::Empty(event)).map_err(xml_error)?;
                path.pop();
            }
            // Text, CDATA, and general references are one logical scalar in XML. Buffer adjacent
            // content so a value split as `Ada&#32;Lovelace` is transformed once, rather than
            // leaving the reference untouched or assigning separate replacements to each piece.
            Event::Text(event) => {
                let raw = event
                    .xml_content(XmlVersion::Implicit1_0)
                    .map_err(xml_error)?;
                if selected_xml_text_path(&path, selected_by_path) {
                    append_pending_xml_scalar(&mut pending_scalar, &path, raw.into_owned(), false);
                } else {
                    flush_pending_xml_scalar(
                        &mut pending_scalar,
                        &mut writer,
                        &mut transform_context,
                    )?;
                    writer.write_event(Event::Text(event)).map_err(xml_error)?;
                }
            }
            Event::CData(event) => {
                let raw = event.decode().map_err(xml_error)?;
                if selected_xml_text_path(&path, selected_by_path) {
                    append_pending_xml_scalar(&mut pending_scalar, &path, raw.into_owned(), true);
                } else {
                    flush_pending_xml_scalar(
                        &mut pending_scalar,
                        &mut writer,
                        &mut transform_context,
                    )?;
                    writer.write_event(Event::CData(event)).map_err(xml_error)?;
                }
            }
            Event::GeneralRef(event) => {
                if selected_xml_text_path(&path, selected_by_path) {
                    let raw = decode_xml_general_ref(event.as_ref())
                        .ok_or_else(|| xml_error("unsupported XML general reference"))?;
                    append_pending_xml_scalar(&mut pending_scalar, &path, raw, false);
                } else {
                    flush_pending_xml_scalar(
                        &mut pending_scalar,
                        &mut writer,
                        &mut transform_context,
                    )?;
                    writer
                        .write_event(Event::GeneralRef(event))
                        .map_err(xml_error)?;
                }
            }
            Event::End(event) => {
                flush_pending_xml_scalar(&mut pending_scalar, &mut writer, &mut transform_context)?;
                writer.write_event(Event::End(event)).map_err(xml_error)?;
                path.pop();
            }
            Event::Eof => {
                flush_pending_xml_scalar(&mut pending_scalar, &mut writer, &mut transform_context)?;
                break;
            }
            other => {
                flush_pending_xml_scalar(&mut pending_scalar, &mut writer, &mut transform_context)?;
                writer.write_event(other).map_err(xml_error)?;
            }
        }
    }

    String::from_utf8(writer.into_inner()).map_err(xml_error)
}

struct PendingXmlScalar {
    path: Vec<String>,
    raw: String,
    all_cdata: bool,
}

fn selected_xml_text_path(path: &[String], selected: &HashMap<String, ColumnMetadata>) -> bool {
    selected.contains_key(&xml_text_source_path(path))
}

fn append_pending_xml_scalar(
    pending: &mut Option<PendingXmlScalar>,
    path: &[String],
    raw: String,
    is_cdata: bool,
) {
    match pending {
        Some(existing) if existing.path == path => {
            existing.raw.push_str(&raw);
            existing.all_cdata &= is_cdata;
        }
        _ => {
            *pending = Some(PendingXmlScalar {
                path: path.to_vec(),
                raw,
                all_cdata: is_cdata,
            });
        }
    }
}

fn flush_pending_xml_scalar(
    pending: &mut Option<PendingXmlScalar>,
    writer: &mut Writer<Vec<u8>>,
    context: &mut XmlTransformContext<'_>,
) -> Result<()> {
    let Some(pending_scalar) = pending.take() else {
        return Ok(());
    };
    let replacement = xml_text_replacement(&pending_scalar.path, &pending_scalar.raw, context);
    match replacement.as_deref() {
        Some(anonymized) if pending_scalar.all_cdata => writer
            .write_event(Event::CData(BytesCData::new(anonymized)))
            .map_err(xml_error),
        Some(anonymized) => writer
            .write_event(Event::Text(BytesText::new(anonymized)))
            .map_err(xml_error),
        None if pending_scalar.all_cdata => writer
            .write_event(Event::CData(BytesCData::new(&pending_scalar.raw)))
            .map_err(xml_error),
        None => writer
            .write_event(Event::Text(BytesText::new(&pending_scalar.raw)))
            .map_err(xml_error),
    }
}

fn decode_xml_general_ref(raw: &[u8]) -> Option<String> {
    let name = std::str::from_utf8(raw).ok()?;
    match name {
        "amp" => Some("&".to_string()),
        "lt" => Some("<".to_string()),
        "gt" => Some(">".to_string()),
        "apos" => Some("'".to_string()),
        "quot" => Some("\"".to_string()),
        value if let Some(hex) = value.strip_prefix("#x") => {
            char::from_u32(u32::from_str_radix(hex, 16).ok()?).map(|c| c.to_string())
        }
        value if let Some(decimal) = value.strip_prefix('#') => {
            char::from_u32(decimal.parse().ok()?).map(|c| c.to_string())
        }
        _ => None,
    }
}

/// Wraps any XML reader/writer failure as an input-parse error.
///
/// Every fallible XML call reports the same way, so the message stays uniform
/// and the call sites stay readable.
fn xml_error(error: impl std::fmt::Display) -> AnonymizerError {
    AnonymizerError::input_parse("XML", error.to_string())
}

/// The anonymized replacement for a text or CDATA node, or `None` when the node
/// must be written through untouched — either its path is not selected or it
/// holds only whitespace.
///
/// Text and CDATA differ only in how they decode and how they are written back;
/// the decision and the transform are identical, so they live here once.
fn xml_text_replacement(
    path: &[String],
    raw: &str,
    context: &mut XmlTransformContext<'_>,
) -> Option<String> {
    let path_name = xml_text_source_path(path);
    let column = context.selected_by_path.get(&path_name)?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let row_index = next_row_index(context.row_indices, &path_name);
    let value_context = TransformContext::for_column(column, row_index);
    Some(transform_value_with_state(
        trimmed,
        column,
        &value_context,
        context.state,
    ))
}

struct XmlTransformContext<'a> {
    selected_by_path: &'a HashMap<String, ColumnMetadata>,
    row_indices: &'a mut HashMap<String, usize>,
    state: &'a mut TransformState,
}

fn transform_xml_attributes(
    reader: &Reader<&[u8]>,
    event: quick_xml::events::BytesStart<'_>,
    path: &[String],
    context: &mut XmlTransformContext<'_>,
) -> Result<quick_xml::events::BytesStart<'static>> {
    let mut owned = event.to_owned();
    let attributes = event
        .attributes()
        .with_checks(false)
        .map(|attribute| {
            attribute.map_err(xml_error).and_then(|attribute| {
                let key = xml_name(attribute.key.as_ref());
                let value = attribute
                    .decoded_and_normalized_value(XmlVersion::Implicit1_0, reader.decoder())
                    .map_err(xml_error)?;
                let path_name = xml_attribute_source_path(path, &key);
                let next_value = if let Some(column) = context.selected_by_path.get(&path_name) {
                    let row_index = next_row_index(context.row_indices, &path_name);
                    let value_context = TransformContext::for_column(column, row_index);
                    transform_value_with_state(value.trim(), column, &value_context, context.state)
                } else {
                    value.into_owned()
                };
                Ok((key, next_value))
            })
        })
        .collect::<Result<Vec<_>>>()?;

    owned.clear_attributes();
    for (key, value) in attributes {
        owned.push_attribute((key.as_str(), value.as_str()));
    }

    Ok(owned)
}

/// The record count is the longest field's, counted over the whole document rather
/// than over what the sample kept — the sample is bounded, the count reported to the
/// user is not.
fn infer_xml_row_count(fields: &[FieldSamples]) -> usize {
    fields
        .iter()
        .map(|field| field.value_count())
        .max()
        .unwrap_or(0)
}

fn xml_text_source_path(path: &[String]) -> String {
    let mut source_path = String::from("xml");
    for segment in path {
        source_path.push('/');
        source_path.push_str("e:");
        source_path.push_str(&escape_path_key(segment));
    }
    source_path.push_str("/text");
    source_path
}

fn xml_attribute_source_path(path: &[String], attribute: &str) -> String {
    let mut source_path = String::from("xml");
    for segment in path {
        source_path.push('/');
        source_path.push_str("e:");
        source_path.push_str(&escape_path_key(segment));
    }
    source_path.push('/');
    source_path.push_str("@:");
    source_path.push_str(&escape_path_key(attribute));
    source_path
}

fn xml_text_label(path: &[String]) -> String {
    if path
        .iter()
        .all(|segment| is_plain_xml_label_segment(segment))
    {
        let label = format_path(path);
        if label.is_empty() {
            "$".to_string()
        } else {
            label
        }
    } else {
        path.iter()
            .map(|segment| xml_label_segment(segment))
            .collect::<Vec<_>>()
            .join(".")
    }
}

fn xml_attribute_label(path: &[String], attribute: &str) -> String {
    format!("{}.@{}", xml_text_label(path), xml_label_segment(attribute))
}

fn xml_label_segment(segment: &str) -> String {
    if is_plain_xml_label_segment(segment) {
        segment.to_string()
    } else {
        format!(
            "[{}]",
            serde_json::to_string(segment).unwrap_or_else(|_| "\"?\"".to_string())
        )
    }
}

fn is_plain_xml_label_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | ':')
        })
}

fn xml_name(name: &[u8]) -> String {
    String::from_utf8_lossy(name).into_owned()
}
