use super::{
    cardinality_warning_for_column, possible_person_name_warning_for_column,
    preview_warning_for_column, select_columns,
};
use crate::error::Result;
use crate::preview::generate_column_preview;
use crate::smart::{SmartReplacementProvider, prepare_smart_replacements_from_rows};
use crate::strategies::TransformState;
use crate::types::{ColumnControl, ColumnMetadata, PreviewData};

/// Rows a preview reads to display a requested sample count.
///
/// Twice the request, because a preview drops rows that a column's strategy leaves
/// unchanged and would otherwise run short of samples to show.
///
/// Named once for both workflows: the file preview and the paste previews show the
/// same window, and as a bare expression in one of them and a named function in the
/// other the two could be widened apart, leaving the same paste and file showing a
/// different number of rows.
pub(crate) fn display_row_count(sample_count: usize) -> usize {
    sample_count.saturating_mul(2).max(1)
}

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
    let selected_metadata = select_columns(metadata, columns, controls)?;
    let smart_replacements =
        prepare_smart_replacements_from_rows(rows, &selected_metadata, None, provider)?;
    let smart_replacement_entries = smart_replacements.to_entries();
    let mut state = TransformState::with_smart_replacements_if_active(smart_replacements);
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
