use super::state::{
    LETTER_CHARSET, PseudonymDomain, TOKEN_CHARSET, TransformState, normalized_identity,
    random_string,
};
use crate::hash::{deterministic_number, deterministic_string, deterministic_uuid};
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
        if context.deterministic {
            format!(
                "tok_{}",
                deterministic_string(
                    &source_key,
                    &format!("{}:opaque:{attempt}", context.seed),
                    16,
                    TOKEN_CHARSET,
                )
            )
        } else {
            format!("tok_{}", random_string(16, TOKEN_CHARSET))
        }
    })
}

pub(super) fn transform_email(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let Some(at_index) = value.rfind('@') else {
        return value.to_string();
    };
    let domain = &value[at_index..];
    let source_key = normalized_identity(value);
    let local_part = state.assign_generated(PseudonymDomain::EmailLocal, &source_key, |attempt| {
        if context.deterministic {
            let prefix = deterministic_string(
                value,
                &format!("{}:email-prefix:{attempt}", context.seed),
                6,
                LETTER_CHARSET,
            );
            let suffix = deterministic_string(
                value,
                &format!("{}:email-suffix:{attempt}", context.seed),
                3,
                "0123456789",
            );
            format!("{prefix}{suffix}")
        } else {
            let mut rng = rand::thread_rng();
            format!("user{}", rng.gen_range(1..=999_999))
        }
    });
    format!("{local_part}{domain}")
}

pub(super) fn transform_uuid(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    let uuid = state.assign_generated(PseudonymDomain::Uuid, &source_key, |attempt| {
        if context.deterministic {
            deterministic_uuid(value, &format!("{}:uuid:{attempt}", context.seed))
        } else {
            random_uuid_v4()
        }
    });
    if value == value.to_uppercase() {
        uuid.to_uppercase()
    } else {
        uuid
    }
}

fn random_uuid_v4() -> String {
    let mut bytes = [0_u8; 16];
    rand::thread_rng().fill(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

pub(super) fn transform_timestamp(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Timestamp, &source_key, |attempt| {
        transform_timestamp_candidate(value, context, attempt)
    })
}

fn transform_timestamp_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    if value.len() < 10 {
        return value.to_string();
    }

    let Ok(date) = NaiveDate::parse_from_str(&value[..10], "%Y-%m-%d") else {
        return value.to_string();
    };

    let offset_days = if context.deterministic {
        deterministic_number(
            value,
            &format!("{}:timestamp:{attempt}", context.seed),
            -365,
            365,
        )
    } else {
        rand::thread_rng().gen_range(-365..=365)
    };

    let Some(offset_date) = date.checked_add_signed(Duration::days(offset_days)) else {
        return value.to_string();
    };

    format!("{}{}", offset_date.format("%Y-%m-%d"), &value[10..])
}

pub(super) fn transform_phone(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = normalized_identity(value);
    state.assign_generated(PseudonymDomain::Phone, &source_key, |attempt| {
        transform_phone_candidate(value, context, attempt)
    })
}

fn transform_phone_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let mut digit_index = 0;
    value
        .chars()
        .map(|character| {
            if !character.is_ascii_digit() {
                return character.to_string();
            }

            let seed = format!("{}:phone:{attempt}:{digit_index}", context.seed);
            digit_index += 1;
            if context.deterministic {
                deterministic_string(value, &seed, 1, "0123456789")
            } else {
                rand::thread_rng().gen_range(0..=9).to_string()
            }
        })
        .collect()
}

pub(super) fn transform_generic_string(
    value: &str,
    context: &TransformContext<'_>,
    state: &mut TransformState,
) -> String {
    let source_key = format!("{}:{}", value.len(), normalized_identity(value));
    state.assign_generated(PseudonymDomain::GenericString, &source_key, |attempt| {
        transform_generic_string_candidate(value, context, attempt)
    })
}

fn transform_generic_string_candidate(
    value: &str,
    context: &TransformContext<'_>,
    attempt: usize,
) -> String {
    let target_length = value.len();
    if target_length == 0 {
        return value.to_string();
    }

    let min_length = (target_length as f64 * 0.8).floor().max(1.0) as usize;
    let max_length = (target_length as f64 * 1.2).ceil() as usize;
    let charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";

    let output_length = if context.deterministic {
        deterministic_number(
            value,
            &format!("{}:length:{attempt}", context.seed),
            min_length as i64,
            max_length as i64,
        ) as usize
    } else {
        rand::thread_rng().gen_range(min_length..=max_length)
    };

    if context.deterministic {
        deterministic_string(
            value,
            &format!("{}:content:{attempt}", context.seed),
            output_length,
            charset,
        )
    } else {
        let chars: Vec<char> = charset.chars().collect();
        let mut rng = rand::thread_rng();
        (0..output_length)
            .map(|_| chars[rng.gen_range(0..chars.len())])
            .collect()
    }
}
