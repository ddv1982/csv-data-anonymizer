use crate::detection::{classify_pii_risk, detect_column_type, detect_empty_format};
use crate::types::{ColumnMetadata, PiiRisk};
use std::collections::HashSet;

const DEFAULT_SAMPLE_COUNT: usize = 5;

pub fn build_column_metadata(headers: &[String], samples: &[Vec<String>]) -> Vec<ColumnMetadata> {
    headers
        .iter()
        .enumerate()
        .map(|(index, header)| {
            let values = extract_column_values(samples, index);
            build_single_column_metadata(header, index, &values, DEFAULT_SAMPLE_COUNT)
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
            column.is_selected = matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium);
            column
        })
        .collect()
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
) -> ColumnMetadata {
    let detection = detect_column_type(values);
    let pii_risk = classify_pii_risk(detection.data_type);
    let empty_format = detect_empty_format(values);
    let sample_values = values
        .iter()
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .take(sample_count)
        .cloned()
        .collect();

    ColumnMetadata {
        name: name.to_string(),
        index,
        detected_type: detection.data_type,
        confidence: detection.confidence,
        pii_risk,
        sample_values,
        empty_format,
        is_selected: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DataType;

    #[test]
    fn builds_metadata_for_all_columns() {
        let headers = vec!["email".to_string(), "id".to_string(), "country".to_string()];
        let samples = vec![
            vec![
                "john@example.com".to_string(),
                "1001".to_string(),
                "US".to_string(),
            ],
            vec![
                "jane@test.org".to_string(),
                "1002".to_string(),
                "GB".to_string(),
            ],
        ];

        let metadata = build_column_metadata(&headers, &samples);

        assert_eq!(metadata.len(), 3);
        assert_eq!(metadata[0].detected_type, DataType::Email);
        assert_eq!(metadata[1].detected_type, DataType::NumericId);
        assert_eq!(metadata[2].detected_type, DataType::CountryCode);
    }

    #[test]
    fn applies_column_selection_without_mutating_source() {
        let metadata = vec![ColumnMetadata {
            name: "email".to_string(),
            index: 0,
            detected_type: DataType::Email,
            confidence: crate::types::Confidence::High,
            pii_risk: PiiRisk::High,
            sample_values: vec![],
            empty_format: crate::types::EmptyFormat::EmptyString,
            is_selected: false,
        }];

        let selected = apply_column_selection(&metadata, &[0]);

        assert!(selected[0].is_selected);
        assert!(!metadata[0].is_selected);
    }
}
