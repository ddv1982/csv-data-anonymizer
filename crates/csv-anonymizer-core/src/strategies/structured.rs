use super::state::{
    PseudonymDomain, TOKEN_CHARSET, TransformState, normalized_identity, random_string,
};
use crate::hash::random_uuid_v4;
use crate::types::TransformContext;
use chrono::{Duration, NaiveDate};
use rand::Rng;

pub(super) fn transform_opaque_token(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!(
        "{}:{}:{}",
        context.column_name,
        context.column_index,
        normalized_identity(value)
    );
    state.assign_generated(PseudonymDomain::OpaqueToken, &source_key, |attempt| {
        let _ = attempt;
        format!("tok_{}", random_string(16, TOKEN_CHARSET))
    })
}

pub(super) fn transform_email(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let Some(at_index) = value.rfind('@') else {
        return shape_fallback(value, context, state);
    };
    let domain = &value[at_index..];
    let source_key = normalized_identity(value);
    let local_part = state.assign_generated(PseudonymDomain::EmailLocal, &source_key, |attempt| {
        let _ = attempt;
        let mut rng = rand::thread_rng();
        format!("user{}", rng.gen_range(1..=999_999))
    });
    format!("{local_part}{domain}")
}

pub(super) fn transform_uuid(
    value: &str,
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    let uuid = state.assign_generated(PseudonymDomain::Uuid, &source_key, |attempt| {
        let _ = attempt;
        random_uuid_v4()
    });
    if value == value.to_uppercase() {
        uuid.to_uppercase()
    } else {
        uuid
    }
}

pub(super) fn transform_timestamp(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let Some((date, suffix)) = iso_date_prefix(value) else {
        return shape_fallback(value, context, state);
    };
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Timestamp, &source_key, |attempt| {
        transform_timestamp_candidate(date, suffix, attempt)
    })
}

fn iso_date_prefix(value: &str) -> Option<(NaiveDate, &str)> {
    let prefix = value.get(..10)?;
    let date = NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()?;
    Some((date, &value[10..]))
}

fn transform_timestamp_candidate(date: NaiveDate, suffix: &str, _attempt: usize) -> String {
    let Some(offset_date) = shifted_date(date) else {
        return format!("{}{}", date.format("%Y-%m-%d"), suffix);
    };

    format!("{}{}", offset_date.format("%Y-%m-%d"), suffix)
}

fn shifted_date(date: NaiveDate) -> Option<NaiveDate> {
    for _ in 0..16 {
        let offset_days = random_nonzero_day_offset();
        if let Some(offset_date) = date.checked_add_signed(Duration::days(offset_days)) {
            return Some(offset_date);
        }
    }

    date.checked_add_signed(Duration::days(1))
        .or_else(|| date.checked_add_signed(Duration::days(-1)))
}

fn random_nonzero_day_offset() -> i64 {
    let offset_days = rand::thread_rng().gen_range(-365..=365);
    if offset_days == 0 { 1 } else { offset_days }
}

pub(super) fn transform_phone(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    if !is_phone_shaped(value) {
        return shape_fallback(value, context, state);
    }
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Phone, &source_key, |attempt| {
        transform_phone_candidate(value, attempt)
    })
}

// Digit randomization only anonymizes the digits; any other text in the value
// (names, notes) would survive verbatim. Restrict format preservation to values
// made of digits plus common phone separators and extension markers ("x"/"ext").
fn is_phone_shaped(value: &str) -> bool {
    let digit_count = value.chars().filter(char::is_ascii_digit).count();
    if digit_count < 7 {
        return false;
    }
    value.chars().all(|character| {
        character.is_ascii_digit()
            || character.is_whitespace()
            || matches!(
                character,
                '(' | ')' | '+' | '-' | '.' | '/' | '#' | '*' | ',' | ';'
            )
            || matches!(character.to_ascii_lowercase(), 'x' | 'e' | 't')
    })
}

fn transform_phone_candidate(value: &str, _attempt: usize) -> String {
    value
        .chars()
        .map(|character| {
            if !character.is_ascii_digit() {
                return character.to_string();
            }

            rand::thread_rng().gen_range(0..=9).to_string()
        })
        .collect()
}

// A value that does not match the detected column shape must never survive
// unchanged: replace it with a generic pseudonym and count the fallback so the
// privacy report can disclose it.
fn shape_fallback(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    state.record_shape_fallback();
    transform_generic_string(value, context, state)
}

pub(super) fn transform_generic_string(
    value: &str,
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), normalized_identity(value));
    state.assign_generated(PseudonymDomain::GenericString, &source_key, |attempt| {
        transform_generic_string_candidate(value, attempt)
    })
}

fn transform_generic_string_candidate(value: &str, _attempt: usize) -> String {
    let target_length = value.len();
    if target_length == 0 {
        return value.to_string();
    }

    let min_length = (target_length as f64 * 0.8).floor().max(1.0) as usize;
    let max_length = (target_length as f64 * 1.2).ceil() as usize;
    let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";

    let output_length = rand::thread_rng().gen_range(min_length..=max_length);
    let chars: Vec<char> = charset.chars().collect();
    let mut rng = rand::thread_rng();
    (0..output_length)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}
