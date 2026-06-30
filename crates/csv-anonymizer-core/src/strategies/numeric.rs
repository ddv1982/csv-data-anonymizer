use super::state::{PseudonymDomain, TransformState};
use crate::types::TransformContext;
use rand::Rng;

pub(super) fn transform_numeric_id(
    value: &str,
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), value);
    state.assign_generated(PseudonymDomain::NumericId, &source_key, |attempt| {
        transform_numeric_id_candidate(value, attempt)
    })
}

fn transform_numeric_id_candidate(value: &str, attempt: usize) -> String {
    let digit_count = value.len();
    if digit_count == 0 {
        return value.to_string();
    }

    let leading_zero_count = value
        .chars()
        .take_while(|character| *character == '0')
        .count();
    let candidate = if leading_zero_count > 0 && leading_zero_count < digit_count {
        let generated = generate_numeric_id(digit_count - leading_zero_count, attempt);
        format!("{}{}", "0".repeat(leading_zero_count), generated)
    } else if leading_zero_count == digit_count {
        generate_zero_width_numeric_id(digit_count, attempt)
    } else {
        generate_numeric_id(digit_count, attempt)
    };

    ensure_numeric_replacement_diff(candidate, value)
}

fn generate_zero_width_numeric_id(length: usize, _attempt: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| rng.gen_range(0..=9).to_string())
        .collect()
}

fn generate_numeric_id(length: usize, _attempt: usize) -> String {
    let mut rng = rand::thread_rng();
    let first_digit = rng.gen_range(1..=9).to_string();
    let rest: String = (1..length)
        .map(|_| rng.gen_range(0..=9).to_string())
        .collect();
    format!("{first_digit}{rest}")
}

pub(super) fn transform_numeric_value(
    value: &str,
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), value);
    state.assign_generated(PseudonymDomain::NumericValue, &source_key, |attempt| {
        transform_numeric_value_candidate(value, attempt)
    })
}

fn transform_numeric_value_candidate(value: &str, attempt: usize) -> String {
    let (sign, unsigned) = match value.as_bytes().first() {
        Some(b'+') | Some(b'-') => (&value[..1], &value[1..]),
        _ => ("", value),
    };

    let candidate = if let Some((integer_part, fractional_part)) = unsigned.split_once('.') {
        let integer = generate_numeric_component(integer_part, attempt);
        let fraction = generate_fractional_component(fractional_part, attempt);
        format!("{sign}{integer}.{fraction}")
    } else {
        format!("{sign}{}", generate_numeric_component(unsigned, attempt))
    };

    ensure_numeric_replacement_diff(candidate, value)
}

fn generate_numeric_component(component: &str, attempt: usize) -> String {
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

    let generated = generate_numeric_id(component.len() - leading_zero_count, attempt);
    format!("{}{}", "0".repeat(leading_zero_count), generated)
}

fn generate_fractional_component(component: &str, _attempt: usize) -> String {
    if component.is_empty() {
        return String::new();
    }

    let mut rng = rand::thread_rng();
    (0..component.len())
        .map(|_| rng.gen_range(0..=9).to_string())
        .collect()
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
