use crate::error::{AnonymizerError, Result};
use crate::service::{build_privacy_report, count_transforming_selected_columns};
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{
    ColumnMetadata, PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, TransformContext,
};
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;
use std::time::Instant;

use super::shared::{
    FieldSamples, PreviewSelection, bounded_analysis_sample_count, bounded_preview_sample_count,
    fields_to_rows, format_path, metadata_from_fields, next_row_index, prepare_selected_metadata,
    preview_from_fields_with_smart_provider, push_identified_field_sample,
    selected_columns_by_source, transform_state_for_smart_replacements,
};

pub(super) fn analyze_xml(content: &str, sample_row_count: usize) -> Result<PasteAnalyzeData> {
    let sample_row_count = bounded_analysis_sample_count(sample_row_count)?;
    let fields = collect_xml_fields(content, sample_row_count)?;
    let (headers, rows) = fields_to_rows(&fields, sample_row_count);
    let columns = metadata_from_fields(&fields, &headers, &rows);

    Ok(PasteAnalyzeData {
        format: PasteDataFormat::Xml,
        row_count: infer_xml_row_count(&fields),
        row_count_is_complete: true,
        columns,
    })
}

pub(super) fn preview_xml_with_smart_provider(
    input: PastePreviewParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let fields = collect_xml_fields(&input.content, sample_count.saturating_mul(2).max(1))?;
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

pub(super) fn transform_xml_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let analysis = analyze_xml(&input.content, 100)?;
    let metadata = prepare_selected_metadata(&analysis.columns, &input.columns, &input.controls)?;
    let selected_by_path = selected_columns_by_source(&metadata);
    let smart_replacements = prepare_xml_smart_replacements(&input, &metadata, provider)?;
    let start_time = Instant::now();
    let mut state = transform_state_for_smart_replacements(
        input.deterministic,
        &input.seed,
        smart_replacements,
    );
    let output = transform_xml_content(
        &input.content,
        &selected_by_path,
        &mut state,
        &input.seed,
        input.deterministic,
    )?;

    Ok(PasteTransformData {
        output,
        row_count: analysis.row_count,
        columns_anonymized: count_transforming_selected_columns(&metadata),
        duration_ms: start_time.elapsed().as_millis(),
        privacy_report: build_privacy_report(&metadata, state.report(), input.deterministic),
    })
}

fn prepare_xml_smart_replacements(
    input: &PasteTransformParams,
    metadata: &[ColumnMetadata],
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<crate::smart::SmartReplacementMap> {
    let fields = collect_xml_fields(&input.content, usize::MAX)?;
    let (_headers, rows) = fields_to_rows(&fields, usize::MAX);
    prepare_smart_replacements_from_rows(
        &rows,
        metadata,
        input.deterministic,
        &input.seed,
        provider,
    )
}

pub(super) fn collect_xml_fields(content: &str, sample_count: usize) -> Result<Vec<FieldSamples>> {
    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(false);
    let mut path = Vec::new();
    let mut fields = Vec::new();

    loop {
        match reader
            .read_event()
            .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?
        {
            Event::Start(event) => {
                path.push(xml_name(event.name().as_ref()));
                collect_xml_attributes(&reader, &event, &path, &mut fields, sample_count)?;
            }
            Event::Empty(event) => {
                path.push(xml_name(event.name().as_ref()));
                collect_xml_attributes(&reader, &event, &path, &mut fields, sample_count)?;
                path.pop();
            }
            Event::Text(event) => {
                let value = event
                    .xml_content()
                    .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
                if !value.trim().is_empty() {
                    let source_path = xml_text_source_path(&path);
                    let label = xml_text_label(&path);
                    push_identified_field_sample(
                        &mut fields,
                        Some(&source_path),
                        &label,
                        value.trim(),
                        sample_count,
                    )?;
                }
            }
            Event::CData(event) => {
                let value = event
                    .decode()
                    .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
                if !value.trim().is_empty() {
                    let source_path = xml_text_source_path(&path);
                    let label = xml_text_label(&path);
                    push_identified_field_sample(
                        &mut fields,
                        Some(&source_path),
                        &label,
                        value.trim(),
                        sample_count,
                    )?;
                }
            }
            Event::End(_) => {
                path.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(fields)
}

fn collect_xml_attributes(
    reader: &Reader<&[u8]>,
    event: &quick_xml::events::BytesStart<'_>,
    path: &[String],
    fields: &mut Vec<FieldSamples>,
    sample_count: usize,
) -> Result<()> {
    for attribute in event.attributes().with_checks(false) {
        let attribute =
            attribute.map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
        let value = attribute
            .decode_and_unescape_value(reader.decoder())
            .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
        if value.trim().is_empty() {
            continue;
        }
        let key = xml_name(attribute.key.as_ref());
        let source_path = xml_attribute_source_path(path, &key);
        let label = xml_attribute_label(path, &key);
        push_identified_field_sample(
            fields,
            Some(&source_path),
            &label,
            value.trim(),
            sample_count,
        )?;
    }

    Ok(())
}

fn transform_xml_content(
    content: &str,
    selected_by_path: &HashMap<String, ColumnMetadata>,
    state: &mut TransformState,
    seed: &str,
    deterministic: bool,
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
        seed,
        deterministic,
    };

    loop {
        let event = reader
            .read_event()
            .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
        match event {
            Event::Start(event) => {
                path.push(xml_name(event.name().as_ref()));
                let event =
                    transform_xml_attributes(&reader, event, &path, &mut transform_context)?;
                writer
                    .write_event(Event::Start(event))
                    .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
            }
            Event::Empty(event) => {
                path.push(xml_name(event.name().as_ref()));
                let event =
                    transform_xml_attributes(&reader, event, &path, &mut transform_context)?;
                writer
                    .write_event(Event::Empty(event))
                    .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
                path.pop();
            }
            Event::Text(event) => {
                let path_name = xml_text_source_path(&path);
                if let Some(column) = transform_context.selected_by_path.get(&path_name) {
                    let value = event
                        .xml_content()
                        .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
                    if value.trim().is_empty() {
                        writer.write_event(Event::Text(event)).map_err(|error| {
                            AnonymizerError::input_parse("XML", error.to_string())
                        })?;
                    } else {
                        let row_index = next_row_index(transform_context.row_indices, &path_name);
                        let context = TransformContext {
                            column_name: &column.name,
                            column_index: column.index,
                            row_index,
                            seed: transform_context.seed,
                            deterministic: transform_context.deterministic,
                            empty_format: column.empty_format,
                        };
                        let anonymized = transform_value_with_state(
                            value.trim(),
                            column,
                            &context,
                            transform_context.state,
                        );
                        writer
                            .write_event(Event::Text(BytesText::new(&anonymized)))
                            .map_err(|error| {
                                AnonymizerError::input_parse("XML", error.to_string())
                            })?;
                    }
                } else {
                    writer
                        .write_event(Event::Text(event))
                        .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
                }
            }
            Event::End(event) => {
                writer
                    .write_event(Event::End(event))
                    .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
                path.pop();
            }
            Event::Eof => break,
            other => {
                writer
                    .write_event(other)
                    .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
            }
        }
    }

    String::from_utf8(writer.into_inner())
        .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))
}

struct XmlTransformContext<'a> {
    selected_by_path: &'a HashMap<String, ColumnMetadata>,
    row_indices: &'a mut HashMap<String, usize>,
    state: &'a mut TransformState,
    seed: &'a str,
    deterministic: bool,
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
            attribute
                .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))
                .and_then(|attribute| {
                    let key = xml_name(attribute.key.as_ref());
                    let value = attribute
                        .decode_and_unescape_value(reader.decoder())
                        .map_err(|error| AnonymizerError::input_parse("XML", error.to_string()))?;
                    let path_name = xml_attribute_source_path(path, &key);
                    let next_value = if let Some(column) = context.selected_by_path.get(&path_name)
                    {
                        let row_index = next_row_index(context.row_indices, &path_name);
                        let value_context = TransformContext {
                            column_name: &column.name,
                            column_index: column.index,
                            row_index,
                            seed: context.seed,
                            deterministic: context.deterministic,
                            empty_format: column.empty_format,
                        };
                        transform_value_with_state(
                            value.trim(),
                            column,
                            &value_context,
                            context.state,
                        )
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

fn infer_xml_row_count(fields: &[FieldSamples]) -> usize {
    fields
        .iter()
        .map(|field| field.values.len())
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

fn escape_path_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
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
