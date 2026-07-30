use crate::detection::{classify_pii_risk, max_pii_risk};
use crate::error::{AnonymizerError, Result};
use crate::metadata::{build_column_metadata, default_strategy_for_pii_risk};
use crate::sampling::SpreadSampler;
use crate::service::{
    build_privacy_report, count_transforming_selected_columns, display_row_count,
    preview_rows_with_smart_provider as build_preview_from_rows,
};
use crate::smart::{
    SmartReplacementMap, SmartReplacementProvider, prepare_smart_replacements_from_rows,
    reusable_preview_smart_replacements,
};
use crate::types::{
    ColumnControl, ColumnMetadata, DETECTION_SAMPLE_ROW_FLOOR, DataType, DetectionCoverage,
    MAX_PREVIEW_SAMPLE_COUNT, MAX_SAMPLE_ROW_COUNT, PasteAnalyzeData, PasteDataFormat,
    PastePreviewParams, PasteTransformData, PasteTransformParams, PreviewData, TransformReport,
};
use std::collections::HashMap;
use std::time::Instant;

pub(super) const PASTE_MAX_CONTENT_BYTES: usize = 5 * 1024 * 1024;
pub(super) const PASTE_MAX_FIELDS: usize = 512;
pub(super) const PASTE_MAX_TEXT_MATCHES: usize = 10_000;

/// What a paste preview transforms, and for which columns.
///
/// One shape for all five formats, built one way. Each format previews from a
/// different reader but makes the same promise about the request it was given, and
/// assembling the four fields per format is how a preview comes to be shown for a
/// different column set, or a different number of rows, than the caller asked for.
pub(super) struct PreviewSelection<'a, 'provider> {
    pub(super) columns: &'a [usize],
    pub(super) controls: &'a [ColumnControl],
    pub(super) sample_count: usize,
    pub(super) provider: Option<&'provider mut dyn SmartReplacementProvider>,
}

impl<'a, 'provider> PreviewSelection<'a, 'provider> {
    /// The selection a paste preview request describes.
    ///
    /// `sample_count` is passed separately rather than read from `input`, because
    /// every caller has already put it through [`bounded_preview_sample_count`] and
    /// the bounded figure is the one the preview must use.
    pub(super) fn from_params(
        input: &'a PastePreviewParams,
        sample_count: usize,
        provider: Option<&'provider mut dyn SmartReplacementProvider>,
    ) -> Self {
        Self {
            columns: &input.columns,
            controls: &input.controls,
            sample_count,
            provider,
        }
    }
}

pub(super) fn preview_rows_with_smart_provider(
    rows: &[Vec<String>],
    metadata: &[ColumnMetadata],
    population_values: usize,
    selection: PreviewSelection<'_, '_>,
) -> Result<PreviewData> {
    preview_from_rows_with_smart_provider(metadata, rows, population_values, selection)
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
    // A field-shaped paste knows how many values it sampled, not how many the source
    // document holds, so the cardinality warning here rests on the absolute test
    // alone — the behaviour every path had before the row count was available.
    let population_values = detection_rows.len();
    preview_from_rows_with_smart_provider(&metadata, &display_rows, population_values, selection)
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

pub(super) fn preview_from_rows_with_smart_provider(
    metadata: &[ColumnMetadata],
    rows: &[Vec<String>],
    population_values: usize,
    selection: PreviewSelection<'_, '_>,
) -> Result<PreviewData> {
    build_preview_from_rows(
        metadata,
        rows,
        selection.columns,
        selection.controls,
        selection.sample_count,
        population_values,
        selection.provider,
    )
}

/// Smart replacements for a run over a field-shaped paste.
///
/// `fields` must be the run's own, collected over every value rather than over a
/// detection window: replacements are looked up per value, so a field sampled for
/// classification would leave the values it dropped without one, and those values
/// would fall back to rule-based output in the middle of a Local AI column.
///
/// The three field-shaped formats — JSON/YAML, XML and free text — each collect their
/// fields differently and then do exactly this with them, including reusing what the
/// preview already produced rather than asking the model again.
pub(super) fn smart_replacements_for_fields(
    fields: &[FieldSamples],
    metadata: &[ColumnMetadata],
    input: &PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<SmartReplacementMap> {
    let (_headers, rows) = fields_to_rows(fields, FieldWindow::Detection);
    let existing = reusable_preview_smart_replacements(&input.preview_smart_replacements, metadata);
    prepare_smart_replacements_from_rows(&rows, metadata, existing.as_ref(), provider)
}

/// The reply for a completed paste run, in every format.
///
/// Assembled once because three of these five fields are claims about privacy — how
/// many columns were actually transformed, and the whole privacy report with the
/// detection coverage it rests on — and the five formats have to make them the same
/// way. Built per format, a format could report the columns *selected* rather than
/// the columns transforming, or omit the coverage its types were read from, and say
/// so only on that one format's output.
pub(super) fn paste_transform_data(
    output: String,
    row_count: usize,
    metadata: &[ColumnMetadata],
    report: TransformReport,
    coverage: DetectionCoverage,
    start_time: Instant,
) -> PasteTransformData {
    PasteTransformData {
        output,
        row_count,
        columns_anonymized: count_transforming_selected_columns(metadata),
        duration_ms: start_time.elapsed().as_millis(),
        privacy_report: build_privacy_report(metadata, report, coverage),
    }
}

/// Classifies collected fields, and returns the analysis together with the coverage
/// it rests on.
///
/// The coverage is returned rather than only summarized into the DTO because the
/// transform path feeds it to `build_privacy_report`, and rebuilding one from the wire
/// summary would re-open the invariant [`DetectionCoverage`]'s constructor holds.
///
/// `row_count` is the caller's, since each format counts records its own way — array
/// items, the longest XML field, matched spans — but everything downstream of the
/// count is the same for all of them, including the promise that a field-shaped paste
/// is read whole.
pub(super) fn analysis_from_fields(
    format: PasteDataFormat,
    fields: &[FieldSamples],
    row_count: usize,
) -> (PasteAnalyzeData, DetectionCoverage) {
    let (headers, rows) = fields_to_rows(fields, FieldWindow::Detection);
    let columns = metadata_from_fields(fields, &headers, &rows);
    let coverage = detection_coverage(fields, rows.len());

    (
        PasteAnalyzeData {
            format,
            row_count,
            row_count_is_complete: true,
            detection_coverage: coverage.summary(),
            columns,
        },
        coverage,
    )
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
/// The floor is [`DETECTION_SAMPLE_ROW_FLOOR`], the same constant `service::detection_sample_rows`
/// applies to a file, because the two workflows promise to classify the same input on
/// the same basis.
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
    Ok(bounded.max(DETECTION_SAMPLE_ROW_FLOOR))
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
/// of the same field, and collapsing them into a single head window sized to the
/// larger makes classification read the field's opening values only. A field whose
/// PII starts past that opening — a log where a column is a placeholder for the first
/// few hundred records — is then classified off the placeholders, comes back `String`
/// at Low risk, and is not offered for anonymization, while the transform walks every
/// record and copies the real values out.
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

/// How much of a field-based input detection classified.
///
/// Aggregated the way [`fields_to_rows`] already lays fields out: the pseudo-row
/// count is the longest field's, so the totals compared here are the longest
/// field's too. Taking the maximum rather than a sum keeps the two figures in one
/// unit, and the longest field is the one most likely to have been thinned.
///
/// Counted in *values*, not rows, and the returned coverage says so. These inputs
/// have no rows: for a top-level JSON object `infer_value_row_count` reports one row
/// while this counts the busiest field's 500 values, and for free text the busiest
/// span type's match count matches neither the paste's line count nor the match
/// total shown as `row_count`. Reporting either figure as "rows" states a number the
/// user cannot find on screen, which reads as a bug in the tool and gets the whole
/// disclosure discounted. See [`crate::types::DetectionCoverageUnit`].
pub(super) fn detection_coverage(
    fields: &[FieldSamples],
    examined_values: usize,
) -> DetectionCoverage {
    let total = fields
        .iter()
        .map(FieldSamples::value_count)
        .max()
        .unwrap_or(0);
    DetectionCoverage::values(examined_values, total)
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
