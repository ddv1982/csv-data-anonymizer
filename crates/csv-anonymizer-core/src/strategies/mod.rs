use crate::detection::is_empty_value;
use crate::types::{AnonymizationStrategy, ColumnMetadata, DataType, TransformContext};

mod names;
mod numeric;
mod redaction;
mod state;
mod structured;

pub(crate) use redaction::STRUCTURED_SCALAR_REDACTION_WARNING;
pub use state::TransformState;

pub fn transform_value(
    value: &str,
    column: &ColumnMetadata,
    context: &TransformContext<'_>,
) -> String {
    let mut state = TransformState::new(context.deterministic, context.seed);
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

    match column.strategy {
        AnonymizationStrategy::PassThrough => return value.to_string(),
        AnonymizationStrategy::Mask => return mask_value(value),
        AnonymizationStrategy::Redact => {
            return redaction::placeholder_for_column(column).to_string();
        }
        AnonymizationStrategy::Tokenize => {
            return structured::transform_opaque_token(value, context, state);
        }
        AnonymizationStrategy::LocalAi => {
            if let Some(replacement) = state.smart_replacement(column.index, value) {
                return replacement;
            }
            state.record_smart_fallback();
        }
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {}
    }

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
        DataType::PostalCode
        | DataType::Address
        | DataType::IpAddress
        | DataType::Url
        | DataType::MacAddress
        | DataType::TaxId
        | DataType::String
        | DataType::Unknown => structured::transform_generic_string(value, context, state),
        DataType::Boolean | DataType::Currency | DataType::Percentage => value.to_string(),
        DataType::CountryCode | DataType::Enum => value.to_string(),
    }
}

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

pub fn transform_row(
    row: &[String],
    columns: &[ColumnMetadata],
    row_index: usize,
    seed: &str,
    deterministic: bool,
) -> Vec<String> {
    let mut state = TransformState::new(deterministic, seed);
    transform_row_with_state(row, columns, row_index, seed, deterministic, &mut state)
}

pub fn transform_row_with_state(
    row: &[String],
    columns: &[ColumnMetadata],
    row_index: usize,
    seed: &str,
    deterministic: bool,
    state: &mut TransformState,
) -> Vec<String> {
    row.iter()
        .enumerate()
        .map(|(column_index, value)| {
            let Some(column) = columns.get(column_index) else {
                return value.clone();
            };

            if !column.is_selected {
                return value.clone();
            }

            let context = TransformContext {
                column_name: &column.name,
                column_index: column.index,
                row_index,
                seed,
                deterministic,
                empty_format: column.empty_format,
            };
            transform_value_with_state(value, column, &context, state)
        })
        .collect()
}

#[cfg(test)]
mod tests;
