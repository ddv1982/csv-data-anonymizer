mod csv_text;
mod documents;
mod format_detection;
mod quick;
mod shared;
mod text;
mod xml;

#[cfg(test)]
mod tests;

use crate::error::Result;
use crate::types::{
    AnonymizationStrategy, ColumnControl, ColumnMetadata, DataType, PasteAnalyzeData,
    PasteAnalyzeParams, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, PrivacyReport, QuickGenerateParams, QuickTransformData,
    QuickTransformParams,
};

pub fn analyze_paste_data(input: PasteAnalyzeParams) -> Result<PasteAnalyzeData> {
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    match format {
        PasteDataFormat::Csv => csv_text::analyze_csv_text(&input.content, input.sample_row_count),
        PasteDataFormat::Json => {
            let value = documents::parse_json(&input.content)?;
            documents::analyze_value_document(format, &value, input.sample_row_count)
        }
        PasteDataFormat::Yaml => {
            let value = documents::parse_yaml(&input.content)?;
            documents::analyze_value_document(format, &value, input.sample_row_count)
        }
        PasteDataFormat::Xml => xml::analyze_xml(&input.content, input.sample_row_count),
        PasteDataFormat::PlainText | PasteDataFormat::Logs => {
            text::analyze_text_content(&input.content, format, input.sample_row_count)
        }
        PasteDataFormat::Auto => unreachable!("auto format must resolve before analysis"),
    }
}

pub fn preview_paste_data(input: PastePreviewParams) -> Result<PreviewData> {
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    match format {
        PasteDataFormat::Csv => csv_text::preview_csv_text(input),
        PasteDataFormat::Json => {
            let value = documents::parse_json(&input.content)?;
            documents::preview_value_document(input, value, PasteDataFormat::Json)
        }
        PasteDataFormat::Yaml => {
            let value = documents::parse_yaml(&input.content)?;
            documents::preview_value_document(input, value, PasteDataFormat::Yaml)
        }
        PasteDataFormat::Xml => xml::preview_xml(input),
        PasteDataFormat::PlainText | PasteDataFormat::Logs => {
            text::preview_text_content(input, format)
        }
        PasteDataFormat::Auto => unreachable!("auto format must resolve before preview"),
    }
}

pub fn transform_paste_data(input: PasteTransformParams) -> Result<PasteTransformData> {
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    match format {
        PasteDataFormat::Csv => csv_text::transform_csv_text(input),
        PasteDataFormat::Json => documents::transform_json(input),
        PasteDataFormat::Yaml => documents::transform_yaml(input),
        PasteDataFormat::Xml => xml::transform_xml(input),
        PasteDataFormat::PlainText | PasteDataFormat::Logs => text::transform_text(input, format),
        PasteDataFormat::Auto => unreachable!("auto format must resolve before transform"),
    }
}

pub fn transform_quick_values(input: QuickTransformParams) -> Result<QuickTransformData> {
    quick::transform_quick_values(input)
}

pub fn generate_quick_values(input: QuickGenerateParams) -> Result<QuickTransformData> {
    quick::generate_quick_values(input)
}

pub fn preview_rows(
    rows: &[Vec<String>],
    metadata: &[ColumnMetadata],
    columns: &[usize],
    controls: &[ColumnControl],
    deterministic: bool,
    seed: &str,
    sample_count: usize,
) -> Result<PreviewData> {
    shared::preview_rows(
        rows,
        metadata,
        columns,
        controls,
        deterministic,
        seed,
        sample_count,
    )
}

pub fn anonymize_rows(
    rows: &[Vec<String>],
    metadata: &[ColumnMetadata],
    columns: &[usize],
    controls: &[ColumnControl],
    deterministic: bool,
    seed: &str,
) -> Result<(Vec<Vec<String>>, PrivacyReport)> {
    shared::anonymize_rows(rows, metadata, columns, controls, deterministic, seed)
}

pub fn quick_anonymize_values(
    values: &[String],
    data_type: DataType,
    strategy: AnonymizationStrategy,
    deterministic: bool,
    seed: &str,
) -> Result<QuickTransformData> {
    quick::quick_anonymize_values(values, data_type, strategy, deterministic, seed)
}
