use super::state::{PseudonymDomain, TOKEN_CHARSET, TransformState};
use crate::random::{random_digit, random_string, random_uuid_v4};
use crate::smart::value_identity_key;
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
        value_identity_key(value)
    );
    state.assign_generated(PseudonymDomain::OpaqueToken, &source_key, || {
        format!("tok_{}", random_string(16, TOKEN_CHARSET))
    })
}

pub(super) fn transform_email(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    // The domain is read from the folded value, not the raw one. Taking it raw carried
    // the source's own padding into the output — `"  a@b.com  "` produced
    // `"userNNN@b.com  "` — so one source value written twice with different padding
    // produced two different cells while the ledger counted it once, and the retained
    // whitespace disclosed a detail of the original that nothing else in the output does.
    let identity = value_identity_key(value);
    let Some(at_index) = identity.rfind('@') else {
        return shape_fallback(value, context, state);
    };
    let domain = &identity[at_index..];
    let local_part = state.assign_generated(PseudonymDomain::EmailLocal, &identity, || {
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
    let source_key = value_identity_key(value);
    let uuid = state.assign_generated(PseudonymDomain::Uuid, &source_key, random_uuid_v4);
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
    let source_key = value_identity_key(value);
    state.assign_generated(PseudonymDomain::Timestamp, &source_key, || {
        transform_timestamp_candidate(date, suffix)
    })
}

fn iso_date_prefix(value: &str) -> Option<(NaiveDate, &str)> {
    let prefix = value.get(..10)?;
    let date = NaiveDate::parse_from_str(prefix, "%Y-%m-%d").ok()?;
    Some((date, &value[10..]))
}

fn transform_timestamp_candidate(date: NaiveDate, suffix: &str) -> String {
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
    let source_key = value_identity_key(value);
    state.assign_generated(PseudonymDomain::Phone, &source_key, || {
        transform_phone_candidate(value)
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

fn transform_phone_candidate(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if !character.is_ascii_digit() {
                return character.to_string();
            }

            random_digit()
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
    let identity = value_identity_key(value);
    state.assign_generated(PseudonymDomain::GenericString, &identity, || {
        transform_generic_string_candidate(&identity)
    })
}

const GENERIC_STRING_CHARSET: &str =
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";

fn transform_generic_string_candidate(value: &str) -> String {
    let target_length = value.len();
    if target_length == 0 {
        return value.to_string();
    }

    let min_length = (target_length as f64 * 0.8).floor().max(1.0) as usize;
    let max_length = (target_length as f64 * 1.2).ceil() as usize;
    let output_length = rand::thread_rng().gen_range(min_length..=max_length);

    let candidate = random_string(output_length, GENERIC_STRING_CHARSET);
    if candidate.eq_ignore_ascii_case(value) {
        // A short value can be redrawn exactly — a single character repeats
        // roughly once in 64 draws. An "anonymized" cell must never be the
        // original, so extend the draw instead of returning it. The result is
        // longer than the input, so it cannot match it.
        //
        // Case-insensitively, because `value` here is the case-folded identity while
        // the charset below is mixed-case: an exact comparison would let the draw
        // `A` through for the source value `A`, whose identity is `a`. Folding the
        // comparison covers the original and every case variant of it, so this is
        // stricter than comparing against the raw source rather than a substitute
        // for it.
        return format!("{candidate}{}", random_string(1, GENERIC_STRING_CHARSET));
    }
    candidate
}
