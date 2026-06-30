use crate::detection::is_empty_value;
use crate::strategies::{TransformState, transform_value_with_state};
use crate::types::{ColumnMetadata, ColumnPreview, SampleTransform, TransformContext};

pub(crate) fn generate_column_preview(
    column: &ColumnMetadata,
    rows: &[Vec<String>],
    sample_count: usize,
    transform_state: &mut TransformState,
) -> ColumnPreview {
    let mut samples = Vec::new();

    for (row_index, row) in rows.iter().enumerate() {
        if samples.len() >= sample_count {
            break;
        }

        let Some(value) = row.get(column.index) else {
            continue;
        };
        if is_empty_value(value) {
            continue;
        }

        let context = TransformContext {
            column_name: &column.name,
            column_index: column.index,
            row_index,
            empty_format: column.empty_format,
        };
        samples.push(SampleTransform {
            original: value.clone(),
            anonymized: transform_value_with_state(value, column, &context, transform_state),
        });
    }

    ColumnPreview {
        column_index: column.index,
        column_name: column.name.clone(),
        samples,
    }
}
