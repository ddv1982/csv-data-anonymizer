use super::{apply_column_controls, preview_warning_for_column, validate_column_indices};
use crate::error::Result;
use crate::metadata::apply_column_selection;
use crate::preview::generate_column_preview;
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::TransformState;
use crate::types::{ColumnControl, ColumnMetadata, PreviewData};

pub(crate) fn preview_rows_with_smart_provider(
    metadata: &[ColumnMetadata],
    rows: &[Vec<String>],
    columns: &[usize],
    controls: &[ColumnControl],
    sample_count: usize,
    provider: Option<&mut dyn SmartReplacementProvider>,
) -> Result<PreviewData> {
    validate_column_indices(metadata, columns)?;
    let controlled = apply_column_controls(metadata, controls)?;
    let selected_metadata = apply_column_selection(&controlled, columns);
    let smart_replacements =
        prepare_smart_replacements_from_rows(rows, &selected_metadata, None, provider)?;
    let smart_replacement_entries = smart_replacements.to_entries();
    let mut state = if smart_replacements.has_activity() {
        TransformState::with_smart_replacements(smart_replacements)
    } else {
        TransformState::new()
    };
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
