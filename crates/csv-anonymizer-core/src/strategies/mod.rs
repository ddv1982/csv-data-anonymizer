use crate::detection::is_empty_value;
use crate::types::{AnonymizationStrategy, ColumnMetadata, DataType, TransformContext};

mod names;
mod numeric;
mod redaction;
mod state;
mod structured;

pub(crate) use redaction::{
    STRUCTURED_SCALAR_REDACTION_WARNING, base_column_label, build_evidence_profile,
    refresh_evidence_profile,
};
pub use state::{TokenizationKey, TransformState};
pub(crate) use structured::is_phone_shaped;

/// What survives [`mask_value`], stated in the words the report and the preview both
/// use.
///
/// Masking looks like the most destructive strategy on screen and is one of the least
/// so on paper: `Jan de Vries` becomes `*** ** *****`, which publishes the word count
/// and every word's letter count exactly. Against a known roster — a company
/// directory, a customer list, a village — that pattern is frequently unique, so the
/// masked column still singles a person out. Nothing in the report said so: the column
/// table read "Masked, Verified — Selected values are replaced with mask characters",
/// which is true and reads as a guarantee.
///
/// A constant rather than two sentences, because the release report and the preview
/// warning are the same claim made at two different times, and the moment they drift
/// the pre-run screen and the post-run report disagree about what the output contains.
pub(crate) const MASK_STRUCTURE_DISCLOSURE: &str = "Masking replaces every non-whitespace character with a star, so each value's length, word count and per-word letter counts are published unchanged — a pattern that can single a record out against a known set of people. Use Redact or Label when the shape itself is identifying.";

/// [`transform_value_with_state`] over a fresh state, for tests that do not care about one.
///
/// Test-only: every production path threads a state across rows, because that is what keeps
/// a repeated source value mapping to the same pseudonym for the whole run.
#[cfg(test)]
pub(crate) fn transform_value(
    value: &str,
    column: &ColumnMetadata,
    context: &TransformContext<'_>,
) -> String {
    let mut state = TransformState::new();
    transform_value_with_state(value, column, context, &mut state)
}

pub fn transform_value_with_state(
    value: &str,
    column: &ColumnMetadata,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    if is_empty_value(value) {
        return value.to_string();
    }

    // Set only by the Local AI arm below, and read only by the pass-through gate that
    // follows it. See the gate for what it is for.
    let mut is_local_ai_fallback = false;

    match column.strategy {
        AnonymizationStrategy::PassThrough => return value.to_string(),
        AnonymizationStrategy::Mask => return mask_value(value),
        AnonymizationStrategy::Redact => {
            return redaction::placeholder_for_column(column);
        }
        AnonymizationStrategy::Label => {
            let ordinal = state.record_pseudonymized_value(column.index, value);
            return redaction::labelled_placeholder(column, ordinal);
        }
        AnonymizationStrategy::Tokenize => {
            state.record_pseudonymized_value(column.index, value);
            return structured::transform_opaque_token(value, context, state);
        }
        AnonymizationStrategy::LocalAi => {
            if let Some(replacement) = state.smart_replacement(column.index, value) {
                state.record_pseudonymized_value(column.index, value);
                return replacement;
            }
            state.record_smart_fallback();
            is_local_ai_fallback = true;
        }
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {}
    }

    // Single source of truth for pass-through behavior: the same predicate the
    // report, preview warnings, and readiness builders consult.
    //
    // Skipped for a Local AI fallback, and that exemption is the whole point of the
    // flag. `uses_default_pass_through` types — `Enum`, `CountryCode`, `Boolean`,
    // `Currency`, `Percentage` — are closed value domains, and a closed domain is
    // exactly what makes the smart-replacement leak guard reject nearly everything: a
    // realistic replacement for one row's `Netherlands` is another row's real
    // `Netherlands`, so the guard refuses it, correctly. Without the exemption the
    // refused value falls into this gate and is written out *verbatim*: the user asked
    // for Smart replacement, the guard did its job, and the source value is published
    // anyway, at close to a 100% rate on precisely the columns where the guard is most
    // active.
    //
    // Only the Local AI path is exempted. Auto and Pseudonymize still pass these types
    // through untouched, which is what pass-through is for: replacing `NL` with another
    // country code buys no privacy and destroys the column, so a user who did not ask
    // for Smart replacement sees no change in behaviour here. What a rejected Local AI
    // value gets instead is the column's rule-based transformer — generic-string
    // pseudonymization for all five of these types — which is what the column report
    // claims happens.
    if !is_local_ai_fallback && column.detected_type.uses_default_pass_through() {
        return value.to_string();
    }

    // Past every early return, so what remains is consistently pseudonymized —
    // including the Local AI fallback, which lands on the same transformers.
    state.record_pseudonymized_value(column.index, value);

    match column.detected_type {
        DataType::Email => structured::transform_email(value, context, state),
        DataType::Uuid => structured::transform_uuid(value, context, state),
        DataType::Timestamp => structured::transform_timestamp(value, context, state),
        DataType::NumericId => numeric::transform_numeric_id(value, context, state),
        DataType::NumericValue => numeric::transform_numeric_value(value, context, state),
        DataType::Phone => structured::transform_phone(value, context, state),
        DataType::FirstName => names::transform_first_name(value, state),
        DataType::LastName => names::transform_last_name(value, state),
        DataType::FullName => names::transform_full_name(value, state),
        _ => structured::transform_generic_string(value, context, state),
    }
}

/// Keeping whitespace is a deliberate readability trade — a masked address stays
/// legible as an address — and it is also what makes the output structure-preserving.
/// See [`MASK_STRUCTURE_DISCLOSURE`] for what that publishes and where it is said.
fn mask_value(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_whitespace() {
                character
            } else {
                '*'
            }
        })
        .collect()
}

pub fn transform_row_with_state(
    row: &[String],
    columns: &[ColumnMetadata],
    row_index: usize,
    state: &mut TransformState,
) -> Vec<String> {
    let released = transform_row_values(row, columns, row_index, state);
    state.record_unchanged_sensitive_values(row, &released, columns);
    state.record_residual_audit(row, &released, columns);
    // After the row is complete, because the joint measure is over the released row and
    // there is no such thing as a partial one. Fed here rather than from the CSV loop so
    // that every caller driving rows through this function is measured, not only the
    // one loop in `crate::csv_io`.
    state.record_released_row(&released, columns);
    released
}

fn transform_row_values(
    row: &[String],
    columns: &[ColumnMetadata],
    row_index: usize,
    state: &mut TransformState,
) -> Vec<String> {
    row.iter()
        .enumerate()
        .map(|(column_index, value)| {
            let Some(column) = columns.get(column_index) else {
                // A cell with no metadata behind it has no strategy, no detected type and
                // no entry in the privacy report: nothing decided what may happen to it and
                // nothing counts what was released. `crate::csv_io` refuses such a row
                // outright — "non-empty data beyond the header cannot be safely modeled or
                // written" — but this function is `pub` and returns no `Result`, so it
                // cannot refuse. Releasing the original was the other option, and it means a
                // caller driving its own rows publishes verbatim values that every figure in
                // the report is silently blind to. Blank is the only cell that leaks
                // nothing, and it keeps the row's length, so a caller writing the result
                // still gets the record arity it handed in.
                return String::new();
            };

            if !column.is_selected {
                return value.clone();
            }

            // Detection sampled trimmed values; transform the trimmed value too so
            // padded duplicates stay consistent and cells detection treated as
            // empty are preserved rather than transformed.
            let trimmed = value.trim();
            if is_empty_value(trimmed) {
                return value.clone();
            }

            let context = TransformContext::for_column(column, row_index);
            transform_value_with_state(trimmed, column, &context, state)
        })
        .collect()
}

#[cfg(test)]
mod tests;
