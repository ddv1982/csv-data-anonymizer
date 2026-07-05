use crate::detection::{
    LocaleContext, analyze_column_privacy, classify_pii_risk, detect_column_type_in_context,
    detect_empty_format, infer_locale_context, max_pii_risk,
};
use crate::types::{AnonymizationStrategy, ColumnMetadata, PiiRisk};
use std::collections::HashSet;

const DEFAULT_SAMPLE_COUNT: usize = 5;

pub fn build_column_metadata(headers: &[String], samples: &[Vec<String>]) -> Vec<ColumnMetadata> {
    let column_values: Vec<Vec<String>> = (0..headers.len())
        .map(|index| extract_column_values(samples, index))
        .collect();
    let locale = infer_locale_context(&column_values);
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            build_single_column_metadata(
                header,
                index,
                &column_values[index],
                DEFAULT_SAMPLE_COUNT,
                &locale,
            )
        })
        .collect()
}

pub fn apply_column_selection(
    metadata: &[ColumnMetadata],
    selected_indices: &[usize],
) -> Vec<ColumnMetadata> {
    let selected: HashSet<usize> = selected_indices.iter().copied().collect();
    metadata
        .iter()
        .map(|column| {
            let mut column = column.clone();
            column.is_selected = selected.contains(&column.index);
            column
        })
        .collect()
}

pub fn auto_select_pii_columns(metadata: &[ColumnMetadata]) -> Vec<ColumnMetadata> {
    metadata
        .iter()
        .map(|column| {
            let mut column = column.clone();
            column.is_selected = should_auto_select_column(&column);
            column
        })
        .collect()
}

pub fn should_auto_select_column(column: &ColumnMetadata) -> bool {
    !column.sample_values.is_empty() && matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium)
}

pub fn default_strategy_for_pii_risk(pii_risk: PiiRisk) -> AnonymizationStrategy {
    if matches!(pii_risk, PiiRisk::High | PiiRisk::Medium) {
        AnonymizationStrategy::Redact
    } else {
        AnonymizationStrategy::Auto
    }
}

fn extract_column_values(rows: &[Vec<String>], column_index: usize) -> Vec<String> {
    rows.iter()
        .map(|row| row.get(column_index).cloned().unwrap_or_default())
        .collect()
}

fn build_single_column_metadata(
    name: &str,
    index: usize,
    values: &[String],
    sample_count: usize,
    locale: &LocaleContext,
) -> ColumnMetadata {
    let detection = detect_column_type_in_context(name, values, locale);
    let privacy = analyze_column_privacy(
        name,
        index,
        values,
        detection.data_type,
        detection.confidence,
    );
    let detected_type = detection.data_type;
    let pii_risk = max_pii_risk(classify_pii_risk(detected_type), privacy.pii_risk);
    let empty_format = detect_empty_format(values);
    let sample_values = values
        .iter()
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .take(sample_count)
        .cloned()
        .collect();

    ColumnMetadata {
        name: name.to_string(),
        source_path: None,
        index,
        detected_type,
        confidence: detection.confidence,
        detection_trace: detection.trace,
        privacy_findings: privacy.findings,
        privacy_evidence: privacy.evidence,
        pii_risk,
        sample_values,
        empty_format,
        is_selected: false,
        strategy: default_strategy_for_pii_risk(pii_risk),
    }
}

#[cfg(test)]
mod tests;
