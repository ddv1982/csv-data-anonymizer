use crate::types::{ColumnMetadata, PiiRisk};

pub(crate) fn push_unselected_column_note(notes: &mut Vec<String>, columns: &[ColumnMetadata]) {
    let unselected_columns = columns.iter().filter(|column| !column.is_selected).count();
    if unselected_columns == 0 {
        return;
    }

    let unselected_detector_risk_columns = columns
        .iter()
        .filter(|column| {
            !column.is_selected && matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium)
        })
        .count();
    if unselected_detector_risk_columns > 0 {
        notes.push(format!(
            "{} unselected high/medium detector-risk {} written unchanged.",
            unselected_detector_risk_columns,
            plural(
                unselected_detector_risk_columns,
                "column was",
                "columns were"
            )
        ));
    } else {
        notes.push(format!(
            "{} unselected {} written unchanged.",
            unselected_columns,
            plural(unselected_columns, "column was", "columns were")
        ));
    }
}

fn plural<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}
