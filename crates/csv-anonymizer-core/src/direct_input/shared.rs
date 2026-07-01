use crate::detection::{classify_pii_risk, max_pii_risk};
use crate::error::{AnonymizerError, Result};
use crate::metadata::{
    apply_column_selection, build_column_metadata, default_strategy_for_pii_risk,
};
use crate::preview::generate_column_preview;
use crate::service::{apply_column_controls, preview_warning_for_column, validate_column_indices};
use crate::smart::{
    SmartReplacementMap, SmartReplacementProvider, has_smart_replacement_columns,
    prepare_smart_replacements_from_rows,
};
use crate::strategies::TransformState;
use crate::types::{ColumnControl, ColumnMetadata, DataType, PreviewData};
use std::collections::HashMap;

pub(super) const PASTE_MAX_CONTENT_BYTES: usize = 5 * 1024 * 1024;
pub(super) const PASTE_MAX_FIELDS: usize = 512;
pub(super) const PASTE_MAX_SAMPLE_ROWS: usize = 1_000;
pub(super) const PASTE_MAX_PREVIEW_SAMPLES: usize = 100;
pub(super) const PASTE_MAX_TEXT_MATCHES: usize = 10_000;

pub(super) struct PreviewSelection<'a, 'provider> {
    pub(super) columns: &'a [usize],
    pub(super) controls: &'a [ColumnControl],
    pub(super) sample_count: usize,
    pub(super) provider: Option<&'provider mut dyn SmartReplacementProvider>,
}

pub(super) fn preview_rows_with_smart_provider(
    rows: &[Vec<String>],
    metadata: &[ColumnMetadata],
    selection: PreviewSelection<'_, '_>,
) -> Result<PreviewData> {
    preview_from_rows_with_smart_provider(metadata, rows, selection)
}

pub(super) fn preview_from_fields_with_smart_provider(
    fields: &[FieldSamples],
    selection: PreviewSelection<'_, '_>,
) -> Result<PreviewData> {
    let (headers, rows) = fields_to_rows(fields, selection.sample_count.saturating_mul(2).max(1));
    let metadata = metadata_from_fields(fields, &headers, &rows);
    preview_from_rows_with_smart_provider(&metadata, &rows, selection)
}

pub(super) fn preview_from_rows_with_smart_provider(
    metadata: &[ColumnMetadata],
    rows: &[Vec<String>],
    selection: PreviewSelection<'_, '_>,
) -> Result<PreviewData> {
    let PreviewSelection {
        columns,
        controls,
        sample_count,
        provider,
    } = selection;

    validate_column_indices(metadata, columns)?;
    let controlled = apply_column_controls(metadata, controls)?;
    let selected_metadata = apply_column_selection(&controlled, columns);
    let smart_replacements =
        prepare_smart_replacements_from_rows(rows, &selected_metadata, None, provider)?;
    let smart_replacement_entries = smart_replacements.to_entries();
    let mut state = transform_state_for_smart_replacements(smart_replacements);
    let mut previews = Vec::new();

    for column in selected_metadata.iter().filter(|column| column.is_selected) {
        previews.push(generate_column_preview(
            column,
            rows,
            sample_count,
            &mut state,
        ));
    }

    let warnings = selected_metadata
        .iter()
        .filter(|column| column.is_selected)
        .filter_map(preview_warning_for_column)
        .collect();

    Ok(PreviewData {
        previews,
        warnings,
        smart_replacements: smart_replacement_entries,
    })
}

pub(super) fn transform_state_for_smart_replacements(
    smart_replacements: SmartReplacementMap,
) -> TransformState {
    if smart_replacements.has_activity() {
        TransformState::with_smart_replacements(smart_replacements)
    } else {
        TransformState::new()
    }
}

pub(super) fn preview_smart_replacements_for_transform(
    input: &crate::types::PasteTransformParams,
    metadata: &[ColumnMetadata],
) -> Option<SmartReplacementMap> {
    let preview_smart_replacements =
        SmartReplacementMap::from_entries(&input.preview_smart_replacements);
    (preview_smart_replacements.has_activity() && has_smart_replacement_columns(metadata))
        .then_some(preview_smart_replacements)
}

pub(super) fn prepare_selected_metadata(
    metadata: &[ColumnMetadata],
    columns: &[usize],
    controls: &[ColumnControl],
) -> Result<Vec<ColumnMetadata>> {
    validate_column_indices(metadata, columns)?;
    let controlled = apply_column_controls(metadata, controls)?;
    Ok(apply_column_selection(&controlled, columns))
}

pub(super) fn selected_columns_by_source(
    metadata: &[ColumnMetadata],
) -> HashMap<String, ColumnMetadata> {
    metadata
        .iter()
        .filter(|column| column.is_selected)
        .map(|column| {
            (
                column
                    .source_path
                    .clone()
                    .unwrap_or_else(|| column.name.clone()),
                column.clone(),
            )
        })
        .collect()
}

pub(super) fn validate_paste_content(content: &str) -> Result<()> {
    if content.trim().is_empty() {
        return Err(AnonymizerError::input_parse(
            "pasted data",
            "Paste data before analyzing or anonymizing.",
        ));
    }

    if content.len() > PASTE_MAX_CONTENT_BYTES {
        return Err(AnonymizerError::input_parse(
            "pasted data",
            format!(
                "Paste at most {} of data at a time. Use the CSV file workflow for larger inputs.",
                format_byte_limit(PASTE_MAX_CONTENT_BYTES)
            ),
        ));
    }

    Ok(())
}

pub(super) fn bounded_analysis_sample_count(sample_count: usize) -> Result<usize> {
    bounded_sample_count(sample_count, PASTE_MAX_SAMPLE_ROWS, "sample row count")
}

pub(super) fn bounded_preview_sample_count(sample_count: usize) -> Result<usize> {
    bounded_sample_count(
        sample_count,
        PASTE_MAX_PREVIEW_SAMPLES,
        "preview sample count",
    )
}

fn bounded_sample_count(sample_count: usize, max: usize, label: &str) -> Result<usize> {
    if sample_count > max {
        return Err(AnonymizerError::input_parse(
            "pasted data",
            format!("{label} must be no more than {max}."),
        ));
    }
    Ok(sample_count.max(1))
}

fn format_byte_limit(bytes: usize) -> String {
    let mib = bytes as f64 / (1024.0 * 1024.0);
    if mib >= 1.0 {
        format!("{mib:.0} MiB")
    } else {
        format!("{bytes} bytes")
    }
}

#[derive(Debug, Clone)]
pub(super) struct FieldSamples {
    pub(super) source_path: Option<String>,
    pub(super) name: String,
    pub(super) values: Vec<String>,
    pub(super) data_type: Option<DataType>,
}

pub(super) fn fields_to_rows(
    fields: &[FieldSamples],
    sample_count: usize,
) -> (Vec<String>, Vec<Vec<String>>) {
    let headers = fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    let row_count = fields
        .iter()
        .map(|field| field.values.len())
        .max()
        .unwrap_or(0)
        .min(sample_count);
    let rows = (0..row_count)
        .map(|row_index| {
            fields
                .iter()
                .map(|field| field.values.get(row_index).cloned().unwrap_or_default())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    (headers, rows)
}

pub(super) fn metadata_from_fields(
    fields: &[FieldSamples],
    headers: &[String],
    rows: &[Vec<String>],
) -> Vec<ColumnMetadata> {
    let mut metadata = build_column_metadata(headers, rows);
    for (field, column) in fields.iter().zip(metadata.iter_mut()) {
        column.source_path = field.source_path.clone();
        if let Some(data_type) = field.data_type {
            column.detected_type = data_type;
            column.pii_risk = max_pii_risk(column.pii_risk, classify_pii_risk(data_type));
        }
    }
    apply_direct_input_strategy_defaults(&mut metadata);
    metadata
}

fn apply_direct_input_strategy_defaults(metadata: &mut [ColumnMetadata]) {
    for column in metadata {
        column.strategy = default_strategy_for_pii_risk(column.pii_risk);
    }
}

pub(super) fn push_identified_field_sample(
    fields: &mut Vec<FieldSamples>,
    source_path: Option<&str>,
    name: &str,
    value: &str,
    sample_count: usize,
) -> Result<()> {
    if name.is_empty() {
        return Ok(());
    }
    if let Some(field) = fields
        .iter_mut()
        .find(|field| field.name == name && field.source_path.as_deref() == source_path)
    {
        if field.values.len() < sample_count {
            field.values.push(value.to_string());
        }
        return Ok(());
    }
    if fields.len() >= PASTE_MAX_FIELDS {
        return Err(AnonymizerError::input_parse(
            "pasted data",
            format!(
                "Detected more than {PASTE_MAX_FIELDS} fields. Reduce the input or choose fewer nested fields."
            ),
        ));
    }
    fields.push(FieldSamples {
        source_path: source_path.map(ToString::to_string),
        name: name.to_string(),
        values: vec![value.to_string()],
        data_type: None,
    });
    Ok(())
}

pub(super) fn push_typed_field_sample(
    fields: &mut Vec<FieldSamples>,
    name: &'static str,
    data_type: DataType,
    value: &str,
    sample_count: usize,
) -> Result<()> {
    if let Some(field) = fields.iter_mut().find(|field| field.name == name) {
        if field.values.len() < sample_count {
            field.values.push(value.to_string());
        }
        return Ok(());
    }
    if fields.len() >= PASTE_MAX_FIELDS {
        return Err(AnonymizerError::input_parse(
            "pasted data",
            format!(
                "Detected more than {PASTE_MAX_FIELDS} fields. Reduce the input or choose fewer detected value types."
            ),
        ));
    }
    fields.push(FieldSamples {
        source_path: None,
        name: name.to_string(),
        values: vec![value.to_string()],
        data_type: Some(data_type),
    });
    Ok(())
}

pub(super) fn next_row_index(row_indices: &mut HashMap<String, usize>, path_name: &str) -> usize {
    let row_index = row_indices.entry(path_name.to_string()).or_insert(0);
    let current = *row_index;
    *row_index += 1;
    current
}

pub(super) fn format_path(path: &[String]) -> String {
    let mut formatted = String::new();
    for segment in path {
        if segment == "[]" {
            formatted.push_str("[]");
            continue;
        }
        if !formatted.is_empty() {
            formatted.push('.');
        }
        formatted.push_str(segment);
    }
    formatted
}
