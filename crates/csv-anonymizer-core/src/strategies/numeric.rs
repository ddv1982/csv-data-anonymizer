use super::state::{PseudonymDomain, TransformState};
use crate::random::{LEADING_DIGIT_CHARSET, random_digits, random_string};
use crate::smart::value_identity_key;
use crate::types::TransformContext;

/// Keyed on [`value_identity_key`], and the generated width is taken from the folded
/// value rather than the raw one.
///
/// Both matter, and the earlier key — `format!("{}:{}", value.len(), value)` — got both
/// wrong in the same way. The transform ledger folds padding and case before counting, so
/// a key that does not fold makes the two disagree: `"42"` and `" 42 "` were one value to
/// the ledger and two to the mapper, which handed out two different replacements while
/// the privacy report stated that one distinct value had been consistently replaced. That
/// is the "repeated source values stay consistent within each run" guarantee failing
/// silently, and being *reported* as holding. CSV input is read with `Trim::All` so it
/// cannot reach here padded; JSON, XML and YAML scalars can.
///
/// Taking the width from the folded value also stops the padding inflating the pseudonym:
/// `" 42 "` is a two-digit number, and generating from the raw length produced a
/// four-digit replacement for it.
pub(super) fn transform_numeric_id(
    value: &str,
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let identity = value_identity_key(value);
    state.assign_generated(PseudonymDomain::NumericId, &identity, || {
        transform_numeric_id_candidate(&identity)
    })
}

fn transform_numeric_id_candidate(value: &str) -> String {
    let digit_count = value.len();
    if digit_count == 0 {
        return value.to_string();
    }

    let leading_zero_count = value
        .chars()
        .take_while(|character| *character == '0')
        .count();
    let candidate = if leading_zero_count > 0 && leading_zero_count < digit_count {
        let generated = generate_numeric_id(digit_count - leading_zero_count);
        format!("{}{}", "0".repeat(leading_zero_count), generated)
    } else if leading_zero_count == digit_count {
        // An all-zeros value has no significant digits to preserve, so the
        // replacement may start with any digit including zero.
        random_digits(digit_count)
    } else {
        generate_numeric_id(digit_count)
    };

    ensure_numeric_replacement_diff(candidate, value)
}

/// `length` digits that do not start with zero, so the replacement keeps the
/// original's digit count when read back as a number.
fn generate_numeric_id(length: usize) -> String {
    let first_digit = random_string(1, LEADING_DIGIT_CHARSET);
    format!("{first_digit}{}", random_digits(length.saturating_sub(1)))
}

pub(super) fn transform_numeric_value(
    value: &str,
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let identity = value_identity_key(value);
    state.assign_generated(PseudonymDomain::NumericValue, &identity, || {
        transform_numeric_value_candidate(&identity)
    })
}

fn transform_numeric_value_candidate(value: &str) -> String {
    let (sign, unsigned) = match value.as_bytes().first() {
        Some(b'+') | Some(b'-') => (&value[..1], &value[1..]),
        _ => ("", value),
    };

    let candidate = if let Some((integer_part, fractional_part)) = unsigned.split_once('.') {
        let integer = generate_numeric_component(integer_part);
        // Fractional digits keep their exact width and may lead with zero.
        let fraction = random_digits(fractional_part.len());
        format!("{sign}{integer}.{fraction}")
    } else {
        format!("{sign}{}", generate_numeric_component(unsigned))
    };

    ensure_numeric_replacement_diff(candidate, value)
}

fn generate_numeric_component(component: &str) -> String {
    if component.is_empty() {
        return String::new();
    }

    let leading_zero_count = component
        .chars()
        .take_while(|character| *character == '0')
        .count();
    if leading_zero_count == component.len() {
        return component.to_string();
    }

    let generated = generate_numeric_id(component.len() - leading_zero_count);
    format!("{}{}", "0".repeat(leading_zero_count), generated)
}

fn ensure_numeric_replacement_diff(candidate: String, original: &str) -> String {
    if candidate != original {
        return candidate;
    }

    let mut characters = candidate.chars().collect::<Vec<_>>();
    for character in characters.iter_mut().rev() {
        if !character.is_ascii_digit() {
            continue;
        }

        *character = if *character == '9' {
            '8'
        } else {
            char::from_digit(character.to_digit(10).unwrap_or(0) + 1, 10).unwrap_or('1')
        };
        return characters.into_iter().collect();
    }

    candidate
}
