use super::state::{PseudonymDomain, TransformState};
use crate::hash::deterministic_string;
use crate::types::TransformContext;
use rand::Rng;

pub(super) fn transform_numeric_id(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), value);
    state.assign_generated(PseudonymDomain::NumericId, &source_key, |attempt| {
        transform_numeric_id_candidate(value, context, attempt)
    })
}

fn transform_numeric_id_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let digit_count = value.len();
    if digit_count == 0 {
        return value.to_string();
    }

    let leading_zero_count = value
        .chars()
        .take_while(|character| *character == '0')
        .count();
    if leading_zero_count > 0 && leading_zero_count < digit_count {
        let generated =
            generate_numeric_id(digit_count - leading_zero_count, value, context, attempt);
        return format!("{}{}", "0".repeat(leading_zero_count), generated);
    }

    if leading_zero_count == digit_count {
        return generate_zero_width_numeric_id(digit_count, value, context, attempt);
    }

    generate_numeric_id(digit_count, value, context, attempt)
}

fn generate_zero_width_numeric_id(
    length: usize,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:zero:{attempt}", context.seed),
            length,
            "0123456789",
        )
    } else {
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect()
    }
}

fn generate_numeric_id(
    length: usize,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if context.deterministic {
        let first_digit = deterministic_string(
            value,
            &format!("{}:first:{attempt}", context.seed),
            1,
            "123456789",
        );
        if length == 1 {
            return first_digit;
        }
        let rest_digits = deterministic_string(
            value,
            &format!("{}:rest:{attempt}", context.seed),
            length - 1,
            "0123456789",
        );
        format!("{first_digit}{rest_digits}")
    } else {
        let mut rng = rand::thread_rng();
        let first_digit = rng.gen_range(1..=9).to_string();
        let rest: String = (1..length)
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect();
        format!("{first_digit}{rest}")
    }
}

pub(super) fn transform_numeric_value(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), value);
    state.assign_generated(PseudonymDomain::NumericValue, &source_key, |attempt| {
        transform_numeric_value_candidate(value, context, attempt)
    })
}

fn transform_numeric_value_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let (sign, unsigned) = match value.as_bytes().first() {
        Some(b'+') | Some(b'-') => (&value[..1], &value[1..]),
        _ => ("", value),
    };

    let Some((integer_part, fractional_part)) = unsigned.split_once('.') else {
        return format!(
            "{sign}{}",
            generate_numeric_component(unsigned, value, context, attempt)
        );
    };

    let integer = generate_numeric_component(integer_part, value, context, attempt);
    let fraction = generate_fractional_component(fractional_part, value, context, attempt);

    format!("{sign}{integer}.{fraction}")
}

fn generate_numeric_component(
    component: &str,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
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

    let generated = generate_numeric_id(
        component.len() - leading_zero_count,
        value,
        context,
        attempt,
    );
    format!("{}{}", "0".repeat(leading_zero_count), generated)
}

fn generate_fractional_component(
    component: &str,
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if component.is_empty() {
        return String::new();
    }

    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:fraction:{attempt}", context.seed),
            component.len(),
            "0123456789",
        )
    } else {
        let mut rng = rand::thread_rng();
        (0..component.len())
            .map(|_| rng.gen_range(0..=9).to_string())
            .collect()
    }
}
