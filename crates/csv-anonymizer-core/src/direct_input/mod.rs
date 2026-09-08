mod csv_text;
mod documents;
mod format_detection;
mod quick;
mod shared;
mod text;
mod xml;

#[cfg(test)]
mod tests;

use crate::detection::CandidateDetector;
use crate::error::Result;
use crate::execution::TransformRuntime;
use crate::metadata::should_auto_select_column;
use crate::types::{
    PasteAnalyzeData, PasteAnalyzeParams, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreviewData,
};

pub fn analyze_paste_data(
    input: PasteAnalyzeParams,
    detector: Option<&mut dyn CandidateDetector>,
) -> Result<PasteAnalyzeData> {
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    let mut analysis = match format {
        PasteDataFormat::Csv => {
            csv_text::analyze_csv_text(&input.content, input.sample_row_count, detector)
                .map(|(analysis, _)| analysis)
        }
        PasteDataFormat::Json | PasteDataFormat::Yaml => {
            let value = documents::parse_value_document(format, &input.content)?;
            documents::analyze_value_document(format, &value, input.sample_row_count, detector)
                .map(|(analysis, _)| analysis)
        }
        PasteDataFormat::Xml => xml::analyze_xml(&input.content, input.sample_row_count, detector)
            .map(|(analysis, _)| analysis),
        PasteDataFormat::PlainText | PasteDataFormat::Logs => {
            text::analyze_text_content(&input.content, format, input.sample_row_count, detector)
        }
        PasteDataFormat::Auto => unreachable!("auto format must resolve before analysis"),
    }?;

    for column in &mut analysis.columns {
        column.is_selected = should_auto_select_column(column);
    }

    Ok(analysis)
}

pub fn preview_paste_data(
    input: PastePreviewParams,
    runtime: TransformRuntime<'_, '_>,
) -> Result<PreviewData> {
    let TransformRuntime {
        provider,
        tokenization_key,
    } = runtime;
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    match format {
        PasteDataFormat::Csv => {
            csv_text::preview_csv_text_with_smart_provider(input, provider, tokenization_key)
        }
        PasteDataFormat::Json | PasteDataFormat::Yaml => {
            let value = documents::parse_value_document(format, &input.content)?;
            documents::preview_value_document_with_smart_provider(
                input,
                value,
                format,
                provider,
                tokenization_key,
            )
        }
        PasteDataFormat::Xml => {
            xml::preview_xml_with_smart_provider(input, provider, tokenization_key)
        }
        PasteDataFormat::PlainText | PasteDataFormat::Logs => {
            text::preview_text_content_with_smart_provider(
                input,
                format,
                provider,
                tokenization_key,
            )
        }
        PasteDataFormat::Auto => unreachable!("auto format must resolve before preview"),
    }
}

pub fn transform_paste_data(
    input: PasteTransformParams,
    runtime: TransformRuntime<'_, '_>,
) -> Result<PasteTransformData> {
    let TransformRuntime {
        provider,
        tokenization_key,
    } = runtime;
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);

    match format {
        PasteDataFormat::Csv => {
            csv_text::transform_csv_text_with_smart_provider(input, provider, tokenization_key)
        }
        PasteDataFormat::Json | PasteDataFormat::Yaml => {
            documents::transform_value_document_with_smart_provider(
                input,
                format,
                provider,
                tokenization_key,
            )
        }
        PasteDataFormat::Xml => {
            xml::transform_xml_with_smart_provider(input, provider, tokenization_key)
        }
        PasteDataFormat::PlainText | PasteDataFormat::Logs => {
            text::transform_text_with_smart_provider(input, format, provider, tokenization_key)
        }
        PasteDataFormat::Auto => unreachable!("auto format must resolve before transform"),
    }
}

pub use quick::generate_quick_values;

pub fn replay_paste_text_candidate_evidence(
    input: &PasteTransformParams,
    snapshot: &crate::PreparedAnalysisSnapshot,
    confirmed_candidate_ids: &[String],
    runtime: TransformRuntime<'_, '_>,
) -> Result<PasteTransformData> {
    let TransformRuntime {
        provider,
        tokenization_key,
    } = runtime;
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);
    if !matches!(format, PasteDataFormat::PlainText | PasteDataFormat::Logs) {
        return Err(crate::error::AnonymizerError::input_parse(
            "prepared text analysis",
            "Candidate span replay is only available for plain text and logs.",
        ));
    }
    text::replay_text_candidate_evidence(
        input,
        format,
        snapshot,
        confirmed_candidate_ids,
        provider,
        tokenization_key,
    )
}

pub fn preview_paste_text_candidate_evidence(
    input: &PastePreviewParams,
    snapshot: &crate::PreparedAnalysisSnapshot,
    confirmed_candidate_ids: &[String],
    runtime: TransformRuntime<'_, '_>,
) -> Result<PreviewData> {
    let TransformRuntime {
        provider,
        tokenization_key,
    } = runtime;
    shared::validate_paste_content(&input.content)?;
    let format = format_detection::resolve_format(input.format, &input.content);
    if !matches!(format, PasteDataFormat::PlainText | PasteDataFormat::Logs) {
        return Err(crate::error::AnonymizerError::input_parse(
            "prepared text analysis",
            "Candidate span preview is only available for plain text and logs.",
        ));
    }
    text::preview_text_candidate_evidence(
        input,
        format,
        snapshot,
        confirmed_candidate_ids,
        provider,
        tokenization_key,
    )
}
