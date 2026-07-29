use crate::detection::{classify_pii_risk, max_pii_risk};
use crate::error::{AnonymizerError, Result};
use crate::metadata::{
    apply_column_selection, build_column_metadata, default_strategy_for_pii_risk,
};
use crate::sampling::SpreadSampler;
use crate::service::{
    apply_column_controls, preview_rows_with_smart_provider as build_preview_from_rows,
    validate_column_indices,
};
use crate::smart::{SmartReplacementMap, SmartReplacementProvider, has_smart_replacement_columns};
use crate::strategies::TransformState;
use crate::types::{
    ColumnControl, ColumnMetadata, DataType, MAX_PREVIEW_SAMPLE_COUNT, MAX_SAMPLE_ROW_COUNT,
    PreviewData,
};
use std::collections::HashMap;

pub(super) const PASTE_MAX_CONTENT_BYTES: usize = 5 * 1024 * 1024;
pub(super) const PASTE_MAX_FIELDS: usize = 512;
pub(super) const PASTE_MAX_TEXT_MATCHES: usize = 10_000;

/// Floor on the rows the paste workflow classifies on, drawn from the whole paste.
///
/// Analyze, preview and transform must all detect on the same basis, or the column
/// table and the preview promise a type the run will not apply. All three reach this
/// figure through [`paste_detection_sample_rows`] with the caller's own "Sample
/// rows", which may only raise the sample, never lower it.
pub(super) const PASTE_DETECTION_SAMPLE_ROWS: usize = 100;

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

/// Previews a field-shaped paste (XML, JSON/YAML, free text).
///
/// Classification and display read different windows of the same fields, for the
/// same reason the CSV paste path splits them: the display window is however many
/// rows the user asked to see, which is no basis for deciding a column's type. A
/// preview that classified on it would show a strategy the transform — which detects
/// on its own basis — then declines to apply.
pub(super) fn preview_from_fields_with_smart_provider(
    fields: &[FieldSamples],
    selection: PreviewSelection<'_, '_>,
) -> Result<PreviewData> {
    let (headers, detection_rows) = fields_to_rows(fields, FieldWindow::Detection);
    let metadata = metadata_from_fields(fields, &headers, &detection_rows);
    let (_, display_rows) = fields_to_rows(fields, FieldWindow::Display);
    preview_from_rows_with_smart_provider(&metadata, &display_rows, selection)
}

/// The two windows a field-shaped preview collects.
pub(super) fn preview_field_sample_limits(
    sample_count: usize,
    detection_sample_rows: usize,
) -> FieldSampleLimits {
    FieldSampleLimits {
        detection: detection_sample_rows,
        display: display_row_count(sample_count),
    }
}

/// Rows a preview displays for a requested sample count.
///
/// Twice the request, because a preview drops rows that a column's strategy leaves
/// unchanged and would otherwise run short of samples to show.
pub(super) fn display_row_count(sample_count: usize) -> usize {
    sample_count.saturating_mul(2).max(1)
}

pub(super) fn preview_from_rows_with_smart_provider(
    metadata: &[ColumnMetadata],
    rows: &[Vec<String>],
    selection: PreviewSelection<'_, '_>,
) -> Result<PreviewData> {
    build_preview_from_rows(
        metadata,
        rows,
        selection.columns,
        selection.controls,
        selection.sample_count,
        selection.provider,
    )
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

/// Validates a requested sample size and applies the detection floor.
///
/// Every paste entry point that classifies — analyze, preview and transform, in all
/// five formats — routes its own "Sample rows" figure through here, so all of them
/// classify on the same number of rows. Both halves of that matter. Without the
/// floor, a low setting let analyze build a column table on less evidence than the
/// run that followed it; without the shared figure, preview and transform stayed
/// pinned to the floor while analyze rose above it, so a raised setting made the
/// table promise a classification the run did not apply.
///
/// The bound is checked against the caller's own figure, so an oversized request is
/// an error rather than something the floor quietly absorbs. It is
/// [`MAX_SAMPLE_ROW_COUNT`] — the same ceiling the CSV workflow and the settings
/// store use — because "Sample rows" is one setting for both workflows; a paste-only
/// ceiling below it turned a valid setting into an error on pasted input. Paste
/// content is capped at [`PASTE_MAX_CONTENT_BYTES`], which bounds the sample far
/// more tightly than the row count does anyway.
pub(super) fn paste_detection_sample_rows(sample_count: usize) -> Result<usize> {
    let bounded = bounded_sample_count(sample_count, MAX_SAMPLE_ROW_COUNT, "sample row count")?;
    Ok(bounded.max(PASTE_DETECTION_SAMPLE_ROWS))
}

pub(super) fn bounded_preview_sample_count(sample_count: usize) -> Result<usize> {
    bounded_sample_count(
        sample_count,
        MAX_PREVIEW_SAMPLE_COUNT,
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

/// How many values a field keeps, per window.
///
/// Two windows rather than one, because the two consumers want different values out
/// of the same field. This used to be a single head window sized to whichever was
/// larger, which quietly made the larger one the *only* one: with the display window
/// under the detection basis, classification read the field's opening values. A field
/// whose PII started past the basis — a log where a column is a placeholder for the
/// first few hundred records — was then classified off the placeholders, came back
/// `String` at Low risk, and was not offered for anonymization, while the transform
/// walked every record and copied the real values out.
#[derive(Debug, Clone, Copy)]
pub(super) struct FieldSampleLimits {
    /// Values kept for classification, drawn from across the whole input.
    pub(super) detection: usize,
    /// Opening values kept to show the user. Zero for entry points that display
    /// nothing.
    pub(super) display: usize,
}

impl FieldSampleLimits {
    /// For an entry point that only classifies.
    pub(super) fn detection_only(detection: usize) -> Self {
        Self {
            detection,
            display: 0,
        }
    }
}

/// Which of a field's windows a caller wants rows from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldWindow {
    Detection,
    Display,
}

pub(super) struct FieldSamples {
    pub(super) source_path: Option<String>,
    pub(super) name: String,
    detection: SpreadSampler<String>,
    display: SpreadSampler<String>,
    pub(super) data_type: Option<DataType>,
}

impl FieldSamples {
    fn new(
        source_path: Option<String>,
        name: String,
        data_type: Option<DataType>,
        limits: FieldSampleLimits,
    ) -> Self {
        Self {
            source_path,
            name,
            detection: SpreadSampler::spread(limits.detection),
            display: SpreadSampler::head(limits.display),
            data_type,
        }
    }

    /// Offers one value to both windows. Each keeps it only if it belongs there, so
    /// a value most of the input's fields will drop costs no allocation.
    fn push(&mut self, value: &str) {
        self.detection.push_with(|| value.to_string());
        self.display.push_with(|| value.to_string());
    }

    /// How many values the field held in total, kept or not.
    pub(super) fn value_count(&self) -> usize {
        self.detection.offered()
    }

    /// Builds a field directly from a known set of values, for tests that exercise
    /// what happens *after* collection.
    #[cfg(test)]
    pub(super) fn from_values(
        source_path: Option<&str>,
        name: &str,
        data_type: Option<DataType>,
        values: &[&str],
    ) -> Self {
        let mut field = Self::new(
            source_path.map(ToString::to_string),
            name.to_string(),
            data_type,
            FieldSampleLimits {
                detection: values.len().max(1),
                display: values.len().max(1),
            },
        );
        for value in values {
            field.push(value);
        }
        field
    }

    fn window(&self, window: FieldWindow) -> &SpreadSampler<String> {
        match window {
            FieldWindow::Detection => &self.detection,
            FieldWindow::Display => &self.display,
        }
    }
}

/// Lays a set of fields out as rows, reading one window of each.
///
/// Fields hold different numbers of values, so the row count is the longest field's
/// and shorter fields pad with empties — the same shape `build_column_metadata`
/// expects from a CSV sample.
pub(super) fn fields_to_rows(
    fields: &[FieldSamples],
    window: FieldWindow,
) -> (Vec<String>, Vec<Vec<String>>) {
    let headers = fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();
    let row_count = fields
        .iter()
        .map(|field| field.window(window).len())
        .max()
        .unwrap_or(0);
    let rows = (0..row_count)
        .map(|row_index| {
            fields
                .iter()
                .map(|field| {
                    field
                        .window(window)
                        .get(row_index)
                        .cloned()
                        .unwrap_or_default()
                })
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
            // The free-text path names a field after the span shape it found, and
            // that name then reaches `build_column_metadata` as a header — so a
            // `phone` field is header evidence for Phone regardless of how sure the
            // span was. Span confidence therefore cannot gate risk here; it governs
            // the column path, where the header is the file's own. See
            // `spans::pattern_span_specs`.
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
    limits: FieldSampleLimits,
) -> Result<()> {
    if name.is_empty() {
        return Ok(());
    }
    if let Some(field) = fields
        .iter_mut()
        .find(|field| field.name == name && field.source_path.as_deref() == source_path)
    {
        field.push(value);
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
    let mut field = FieldSamples::new(
        source_path.map(ToString::to_string),
        name.to_string(),
        None,
        limits,
    );
    field.push(value);
    fields.push(field);
    Ok(())
}

pub(super) fn push_typed_field_sample(
    fields: &mut Vec<FieldSamples>,
    name: &'static str,
    data_type: DataType,
    value: &str,
    limits: FieldSampleLimits,
) -> Result<()> {
    if let Some(field) = fields.iter_mut().find(|field| field.name == name) {
        field.push(value);
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
    let mut field = FieldSamples::new(None, name.to_string(), Some(data_type), limits);
    field.push(value);
    fields.push(field);
    Ok(())
}

pub(super) fn next_row_index(row_indices: &mut HashMap<String, usize>, path_name: &str) -> usize {
    let row_index = row_indices.entry(path_name.to_string()).or_insert(0);
    let current = *row_index;
    *row_index += 1;
    current
}

/// Escapes a path segment for a JSON-Pointer-style source path.
///
/// `~` and `/` are the pointer syntax, so they have to be encoded before a
/// segment can be embedded in one. Both the JSON/YAML and XML readers build the
/// same path identifiers, and those identifiers are how the transform matches a
/// selected column back to its source field — so the two must escape
/// identically or a selected field silently fails to match.
pub(super) fn escape_path_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
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
