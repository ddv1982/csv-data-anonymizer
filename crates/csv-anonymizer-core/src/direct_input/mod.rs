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
use crate::metadata::should_auto_select_column;
use crate::smart::SmartReplacementProvider;
use crate::types::{
    AnonymizationStrategy, ColumnControl, ColumnMetadata, DataType, PasteAnalyzeData,
    PasteAnalyzeParams, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData, PrivacyReport, QuickGenerateParams, QuickTransformData,
    QuickTransformParams,
};

pub fn analyze_paste_data(input: PasteAnalyzeParams) -> Result<PasteAnalyzeData> {
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    let mut analysis = match format {
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
    }?;

    for column in &mut analysis.columns {
        column.is_selected = should_auto_select_column(column);
    }

    Ok(analysis)
}

pub fn preview_paste_data(input: PastePreviewParams) -> Result<PreviewData> {
    preview_paste_data_with_smart_provider(input, None)
}

pub fn preview_paste_data_with_smart_provider(
    input: PastePreviewParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    match format {
        PasteDataFormat::Csv => csv_text::preview_csv_text_with_smart_provider(input, provider),
        PasteDataFormat::Json => {
            let value = documents::parse_json(&input.content)?;
            documents::preview_value_document_with_smart_provider(
                input,
                value,
                PasteDataFormat::Json,
                provider,
            )
        }
        PasteDataFormat::Yaml => {
            let value = documents::parse_yaml(&input.content)?;
            documents::preview_value_document_with_smart_provider(
                input,
                value,
                PasteDataFormat::Yaml,
                provider,
            )
        }
        PasteDataFormat::Xml => xml::preview_xml_with_smart_provider(input, provider),
        PasteDataFormat::PlainText | PasteDataFormat::Logs => {
            text::preview_text_content_with_smart_provider(input, format, provider)
        }
        PasteDataFormat::Auto => unreachable!("auto format must resolve before preview"),
    }
}

pub fn transform_paste_data(input: PasteTransformParams) -> Result<PasteTransformData> {
    transform_paste_data_with_smart_provider(input, None)
}

pub fn transform_paste_data_with_smart_provider(
    input: PasteTransformParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PasteTransformData> {
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    match format {
        PasteDataFormat::Csv => csv_text::transform_csv_text_with_smart_provider(input, provider),
        PasteDataFormat::Json => documents::transform_json_with_smart_provider(input, provider),
        PasteDataFormat::Yaml => documents::transform_yaml_with_smart_provider(input, provider),
        PasteDataFormat::Xml => xml::transform_xml_with_smart_provider(input, provider),
        PasteDataFormat::PlainText | PasteDataFormat::Logs => {
            text::transform_text_with_smart_provider(input, format, provider)
        }
        PasteDataFormat::Auto => unreachable!("auto format must resolve before transform"),
    }
}

pub fn transform_quick_values(input: QuickTransformParams) -> Result<QuickTransformData> {
    quick::transform_quick_values(input)
}

pub fn generate_quick_values(input: QuickGenerateParams) -> Result<QuickTransformData> {
    quick::generate_quick_values(input)
}

pub fn generate_quick_values_with_smart_provider(
    input: QuickGenerateParams,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<QuickTransformData> {
    quick::generate_quick_values_with_smart_provider(input, provider)
}

pub fn preview_rows(
    rows: &[Vec<String>],
    metadata: &[ColumnMetadata],
    columns: &[usize],
    controls: &[ColumnControl],
    sample_count: usize,
) -> Result<PreviewData> {
    shared::preview_rows(rows, metadata, columns, controls, sample_count)
}

pub fn anonymize_rows(
    rows: &[Vec<String>],
    metadata: &[ColumnMetadata],
    columns: &[usize],
    controls: &[ColumnControl],
) -> Result<(Vec<Vec<String>>, PrivacyReport)> {
    shared::anonymize_rows(rows, metadata, columns, controls)
}

pub fn anonymize_rows_with_smart_provider(
    rows: &[Vec<String>],
    metadata: &[ColumnMetadata],
    columns: &[usize],
    controls: &[ColumnControl],
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<(Vec<Vec<String>>, PrivacyReport)> {
    shared::anonymize_rows_with_smart_provider(rows, metadata, columns, controls, provider)
}

pub fn quick_anonymize_values(
    values: &[String],
    data_type: DataType,
    strategy: AnonymizationStrategy,
) -> Result<QuickTransformData> {
    quick::quick_anonymize_values(values, data_type, strategy)
}
