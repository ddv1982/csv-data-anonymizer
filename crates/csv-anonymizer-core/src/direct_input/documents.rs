use crate::error::{AnonymizerError, Result};
use crate::service::{build_privacy_report, count_transforming_selected_columns};
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{
    ColumnMetadata, PasteAnalyzeData, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, TransformContext,
};
use serde_json::{Number, Value};
use std::collections::HashMap;
use std::time::Instant;

use super::shared::{
    FieldSamples, PreviewSelection, bounded_analysis_sample_count, bounded_preview_sample_count,
    fields_to_rows, metadata_from_fields, next_row_index, prepare_selected_metadata,
    preview_from_fields_with_smart_provider, preview_smart_replacements_for_transform,
    push_identified_field_sample, selected_columns_by_source,
    transform_state_for_smart_replacements,
};

pub(super) fn preview_value_document_with_smart_provider(
    input: PastePreviewParams,
    value: Value,
    format: PasteDataFormat,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    let sample_count = bounded_preview_sample_count(input.sample_count)?;
    let mut fields = Vec::new();
    collect_json_fields(
        &value,
        format,
        &mut Vec::new(),
        &mut fields,
        sample_count.saturating_mul(2).max(1),
    )?;
    preview_from_fields_with_smart_provider(
        &fields,
        PreviewSelection {
            columns: &input.columns,
            controls: &input.controls,
            sample_count,
            provider,
        },
    )
}

pub(super) fn transform_json_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let value = parse_json(&input.content)?;
    let (output, result) = transform_value_document(input, value, PasteDataFormat::Json, provider)?;
    let output = serde_json::to_string_pretty(&output)
        .map_err(|error| AnonymizerError::input_parse("JSON", error.to_string()))?;
    Ok(PasteTransformData { output, ..result })
}

pub(super) fn transform_yaml_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    let value = parse_yaml(&input.content)?;
    let (output, result) = transform_value_document(input, value, PasteDataFormat::Yaml, provider)?;
    let output = yaml_serde::to_string(&output)
        .map_err(|error| AnonymizerError::input_parse("YAML", error.to_string()))?;
    Ok(PasteTransformData { output, ..result })
}

fn transform_value_document(
    input: PasteTransformParams,
    mut value: Value,
    format: PasteDataFormat,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<(Value, PasteTransformData)> {
    let analysis = analyze_value_document(format, &value, 100)?;
    let metadata = prepare_selected_metadata(&analysis.columns, &input.columns, &input.controls)?;
    let selected_by_path = selected_columns_by_source(&metadata);
    let smart_replacements =
        prepare_value_smart_replacements(&value, format, &metadata, &input, provider)?;
    let start_time = Instant::now();
    let mut state = transform_state_for_smart_replacements(smart_replacements);
    let mut row_indices = HashMap::new();

    let mut context = ValueTransformContext {
        format,
        selected_by_path: &selected_by_path,
        row_indices: &mut row_indices,
        state: &mut state,
    };

    transform_json_value(&mut value, &mut Vec::new(), &mut context);

    let row_count = infer_value_row_count(&value);
    let result = PasteTransformData {
        output: String::new(),
        row_count,
        columns_anonymized: count_transforming_selected_columns(&metadata),
        duration_ms: start_time.elapsed().as_millis(),
        privacy_report: build_privacy_report(&metadata, state.report()),
    };

    Ok((value, result))
}

fn prepare_value_smart_replacements(
    value: &Value,
    format: PasteDataFormat,
    metadata: &[ColumnMetadata],
    input: &PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<crate::smart::SmartReplacementMap> {
    let mut fields = Vec::new();
    collect_json_fields(value, format, &mut Vec::new(), &mut fields, usize::MAX)?;
    let (_headers, rows) = fields_to_rows(&fields, usize::MAX);
    let existing_smart_replacements = preview_smart_replacements_for_transform(input, metadata);
    prepare_smart_replacements_from_rows(
        &rows,
        metadata,
        existing_smart_replacements.as_ref(),
        provider,
    )
}

pub(super) fn analyze_value_document(
    format: PasteDataFormat,
    value: &Value,
    sample_row_count: usize,
) -> Result<PasteAnalyzeData> {
    let sample_row_count = bounded_analysis_sample_count(sample_row_count)?;
    let mut fields = Vec::new();
    collect_json_fields(
        value,
        format,
        &mut Vec::new(),
        &mut fields,
        sample_row_count,
    )?;
    let (headers, rows) = fields_to_rows(&fields, sample_row_count);
    let columns = metadata_from_fields(&fields, &headers, &rows);

    Ok(PasteAnalyzeData {
        format,
        row_count: infer_value_row_count(value),
        row_count_is_complete: true,
        columns,
    })
}

fn transform_json_value(
    value: &mut Value,
    path: &mut Vec<ValuePathSegment>,
    context: &mut ValueTransformContext<'_>,
) {
    match value {
        Value::Array(items) => {
            path.push(ValuePathSegment::Array);
            for item in items {
                transform_json_value(item, path, context);
            }
            path.pop();
        }
        Value::Object(map) => {
            for (key, child) in map {
                path.push(ValuePathSegment::Key(key.clone()));
                transform_json_value(child, path, context);
                path.pop();
            }
        }
        Value::Null => {}
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            let path_name = value_path_id(context.format, path);
            let Some(column) = context.selected_by_path.get(&path_name) else {
                return;
            };
            let Some(original) = json_scalar_to_string(value) else {
                return;
            };
            let row_index = next_row_index(context.row_indices, &path_name);
            let value_context = TransformContext {
                column_name: &column.name,
                column_index: column.index,
                row_index,
                empty_format: column.empty_format,
            };
            let anonymized =
                transform_value_with_state(&original, column, &value_context, context.state);
            *value = json_replacement_value(value, &anonymized);
        }
    }
}

struct ValueTransformContext<'a> {
    format: PasteDataFormat,
    selected_by_path: &'a HashMap<String, ColumnMetadata>,
    row_indices: &'a mut HashMap<String, usize>,
    state: &'a mut TransformState,
}

pub(super) fn parse_json(content: &str) -> Result<Value> {
    serde_json::from_str(content)
        .map_err(|error| AnonymizerError::input_parse("JSON", error.to_string()))
}

pub(super) fn parse_yaml(content: &str) -> Result<Value> {
    yaml_serde::from_str(content)
        .map_err(|error| AnonymizerError::input_parse("YAML", error.to_string()))
}

fn collect_json_fields(
    value: &Value,
    format: PasteDataFormat,
    path: &mut Vec<ValuePathSegment>,
    fields: &mut Vec<FieldSamples>,
    sample_count: usize,
) -> Result<()> {
    match value {
        Value::Array(items) => {
            path.push(ValuePathSegment::Array);
            for item in items {
                collect_json_fields(item, format, path, fields, sample_count)?;
            }
            path.pop();
        }
        Value::Object(map) => {
            for (key, child) in map {
                path.push(ValuePathSegment::Key(key.clone()));
                collect_json_fields(child, format, path, fields, sample_count)?;
                path.pop();
            }
        }
        Value::Null => push_value_field_sample(fields, format, path, "null", sample_count)?,
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            if let Some(value) = json_scalar_to_string(value) {
                push_value_field_sample(fields, format, path, &value, sample_count)?;
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
enum ValuePathSegment {
    Array,
    Key(String),
}

fn push_value_field_sample(
    fields: &mut Vec<FieldSamples>,
    format: PasteDataFormat,
    path: &[ValuePathSegment],
    value: &str,
    sample_count: usize,
) -> Result<()> {
    let source_path = value_path_id(format, path);
    let label = value_path_label(path);
    push_identified_field_sample(fields, Some(&source_path), &label, value, sample_count)
}

fn value_path_id(format: PasteDataFormat, path: &[ValuePathSegment]) -> String {
    let prefix = match format {
        PasteDataFormat::Json => "json",
        PasteDataFormat::Yaml => "yaml",
        _ => "value",
    };
    let mut id = String::from(prefix);
    for segment in path {
        id.push('/');
        match segment {
            ValuePathSegment::Array => id.push('a'),
            ValuePathSegment::Key(key) => {
                id.push_str("k:");
                id.push_str(&escape_path_key(key));
            }
        }
    }
    id
}

fn value_path_label(path: &[ValuePathSegment]) -> String {
    let mut label = String::new();
    for segment in path {
        match segment {
            ValuePathSegment::Array => label.push_str("[]"),
            ValuePathSegment::Key(key) if is_plain_label_segment(key) => {
                if !label.is_empty() {
                    label.push('.');
                }
                label.push_str(key);
            }
            ValuePathSegment::Key(key) => {
                label.push('[');
                label.push_str(&serde_json::to_string(key).unwrap_or_else(|_| "\"?\"".into()));
                label.push(']');
            }
        }
    }
    if label.is_empty() {
        "$".to_string()
    } else {
        label
    }
}

fn escape_path_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn is_plain_label_segment(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

fn json_scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::Null => Some("null".to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.clone()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn json_replacement_value(original: &Value, anonymized: &str) -> Value {
    match original {
        Value::Number(number) => {
            if number.is_i64() {
                anonymized
                    .parse::<i64>()
                    .ok()
                    .map(|value| Value::Number(value.into()))
                    .unwrap_or_else(|| Value::String(anonymized.to_string()))
            } else if number.is_u64() {
                anonymized
                    .parse::<u64>()
                    .ok()
                    .map(|value| Value::Number(value.into()))
                    .unwrap_or_else(|| Value::String(anonymized.to_string()))
            } else {
                anonymized
                    .parse::<f64>()
                    .ok()
                    .and_then(Number::from_f64)
                    .map(Value::Number)
                    .unwrap_or_else(|| Value::String(anonymized.to_string()))
            }
        }
        Value::Bool(_) => match anonymized {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::String(anonymized.to_string()),
        },
        Value::String(_) => Value::String(anonymized.to_string()),
        Value::Null => Value::Null,
        Value::Array(_) | Value::Object(_) => Value::String(anonymized.to_string()),
    }
}

fn infer_value_row_count(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len(),
        Value::Object(_) => 1,
        Value::Null => 0,
        Value::Bool(_) | Value::Number(_) | Value::String(_) => 1,
    }
}
