use super::{
    apply_column_controls, cardinality_warning_for_column, possible_person_name_warning_for_column,
    preview_warning_for_column, validate_column_indices,
};
use crate::error::Result;
use crate::metadata::apply_column_selection;
use crate::preview::generate_column_preview;
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::TransformState;
use crate::types::{ColumnControl, ColumnMetadata, PreviewData};

/// `population_values` is how many values each column holds in the whole input, not
/// in `rows` — the cardinality warning is judged against the column's real size, and
/// a caller that passes the sample size back gets only the absolute test. See
/// [`crate::types::ColumnValueDistribution::frequency_inversion_risk_in`].
pub(crate) fn preview_rows_with_smart_provider(
    metadata: &[ColumnMetadata],
    rows: &[Vec<String>],
    columns: &[usize],
    controls: &[ColumnControl],
    sample_count: usize,
    population_values: usize,
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
        .flat_map(|column| {
            let mut column_warnings = Vec::new();
            // Every warning about *how* a column will be transformed only makes sense
            // for a column that is being transformed.
            if column.is_selected {
                column_warnings.extend(preview_warning_for_column(column));
                column_warnings.extend(cardinality_warning_for_column(column, population_values));
            }
            // Outside that gate on purpose, and it is the whole point of this one: it
            // reports a column the app did *not* pick up but which may hold people. A
            // warning shown only for selected columns could never say that, because the
            // columns it needs to talk about are exactly the unselected ones.
            column_warnings.extend(possible_person_name_warning_for_column(column));
            column_warnings
        })
        .collect();

    Ok(PreviewData {
        previews,
        warnings,
        smart_replacements: smart_replacement_entries,
    })
}
