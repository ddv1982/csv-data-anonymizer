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
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let Some(at_index) = value.rfind('@') else {
        return value.to_string();
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
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Timestamp, &source_key, |attempt| {
        transform_timestamp_candidate(value, attempt)
    })
}

fn transform_timestamp_candidate(value: &str, _attempt: usize) -> String {
    if value.len() < 10 {
        return value.to_string();
    }

    let Ok(date) = NaiveDate::parse_from_str(&value[..10], "%Y-%m-%d") else {
        return value.to_string();
    };

    let Some(offset_date) = shifted_date(date) else {
        return value.to_string();
    };

    format!("{}{}", offset_date.format("%Y-%m-%d"), &value[10..])
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
    _context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Phone, &source_key, |attempt| {
        transform_phone_candidate(value, attempt)
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
