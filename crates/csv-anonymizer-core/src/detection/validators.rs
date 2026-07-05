use card_validate::Validate as CardValidate;
use ein::Ein;
use email_address::{EmailAddress, Options as EmailOptions};
use iban::Iban;
use phonenumber::{country, parse as parse_phone_number};
use ssn::Ssn;
use std::convert::TryFrom;
use url::Url;
use vat_id_validator::check_vat_by_country;

use super::locale::LocaleContext;

pub(super) fn is_payment_card_number(digits: &str) -> bool {
    (13..=19).contains(&digits.len()) && CardValidate::from(digits).is_ok()
}

pub(super) fn is_email(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && !trimmed.chars().any(char::is_whitespace)
        && EmailAddress::parse_with_options(
            trimmed,
            EmailOptions::default()
                .with_required_tld()
                .without_domain_literal()
                .without_display_text(),
        )
        .is_ok()
}

pub(super) fn is_phone_in_context(value: &str, locale: &LocaleContext) -> bool {
    let trimmed = value.trim();
    is_phone_like_shape(trimmed, 10, false)
        && !has_code_like_leading_group(trimmed)
        && is_valid_phone_number_in_context(trimmed, locale)
}

fn is_phone_separator(character: char) -> bool {
    matches!(character, ' ' | '-' | '(' | ')' | '.')
}

fn is_phone_like_shape(value: &str, min_digits: usize, allow_slash: bool) -> bool {
    if !value.chars().all(|character| {
        character.is_ascii_digit()
            || matches!(character, '+' | ' ' | '-' | '(' | ')' | '.')
            || (allow_slash && character == '/')
    }) {
        return false;
    }

    let digit_count = phone_digit_count(value);
    if !(min_digits..=15).contains(&digit_count) {
        return false;
    }

    let mut chars = value.chars();
    let plus_count = chars.by_ref().filter(|character| *character == '+').count();
    if plus_count > 1 || (plus_count == 1 && !value.trim_start().starts_with('+')) {
        return false;
    }

    value.trim_start().starts_with('+')
        || value
            .chars()
            .any(|character| is_phone_separator(character) || (allow_slash && character == '/'))
}

fn phone_digit_count(value: &str) -> usize {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .count()
}

fn has_code_like_leading_group(value: &str) -> bool {
    if value.trim_start().starts_with('+') || value.contains('(') {
        return false;
    }

    value.find([' ', '-', '.']).is_some_and(|index| {
        index == 4
            && value[..index]
                .chars()
                .all(|character| character.is_ascii_digit())
    })
}

/// Broad world coverage ordered by rough likelihood; locale-context regions
/// are tried first. 200-sample cap (Task 6) bounds worst-case parse cost.
fn world_phone_regions() -> &'static [country::Id] {
    use country::Id::*;
    &[
        US, CA, GB, NL, DE, FR, ES, PT, IT, JP, BE, LU, IE, AT, CH, DK, SE, NO, FI, PL, CZ, SK, HU,
        RO, BG, GR, TR, UA, IN, CN, KR, AU, NZ, SG, HK, ID, TH, VN, PH, MY, BR, AR, CL, CO, MX, PE,
        ZA, NG, EG, KE, IL, SA, AE, RU,
    ]
}

pub(super) fn is_valid_phone_number_in_context(value: &str, locale: &LocaleContext) -> bool {
    let trimmed = value.trim();
    if trimmed.starts_with('+') {
        return parse_phone_number(None, trimmed).is_ok_and(|number| number.is_valid());
    }

    let context_regions = locale
        .countries()
        .iter()
        .filter_map(|code| code.parse::<country::Id>().ok());

    context_regions
        .chain(world_phone_regions().iter().copied())
        .any(|region| {
            parse_phone_number(Some(region), trimmed).is_ok_and(|number| number.is_valid())
        })
}

pub(super) fn is_valid_phone_number(value: &str) -> bool {
    is_valid_phone_number_in_context(value, &LocaleContext::default())
}

pub(super) fn is_formatted_phone_fallback(
    value: &str,
    min_digits: usize,
    allow_slash: bool,
) -> bool {
    is_phone_like_shape(value, min_digits, allow_slash)
        && phone_digit_count(value) >= min_digits
        && (value.trim_start().starts_with('+') || value.chars().any(is_phone_separator))
}

pub(super) fn is_url(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_whitespace) {
        return false;
    }

    let owned_candidate;
    let candidate = if trimmed.starts_with("www.") {
        owned_candidate = format!("https://{trimmed}");
        owned_candidate.as_str()
    } else {
        trimmed
    };

    Url::parse(candidate)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

pub(super) fn is_tax_id(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.contains('-') && (is_us_ssn(trimmed) || is_us_ein(trimmed))
}

pub(super) fn is_unformatted_tax_id(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() == 9
        && trimmed.chars().all(|character| character.is_ascii_digit())
        && (is_us_ssn(trimmed) || is_us_ein(trimmed))
}

pub(super) fn is_us_ssn(value: &str) -> bool {
    value.trim().parse::<Ssn>().is_ok()
}

pub(super) fn is_us_ein(value: &str) -> bool {
    value.trim().parse::<Ein>().is_ok()
}

pub(super) fn is_iban(value: &str) -> bool {
    let normalized = value
        .chars()
        .filter(|character| !character.is_whitespace())
        .map(|character| character.to_ascii_uppercase())
        .collect::<String>();

    Iban::try_from(normalized.as_str()).is_ok()
}

pub(super) fn is_vat_id(value: &str) -> bool {
    let normalized = normalized_ascii_identifier(value);
    let Some(country) = prefixed_vat_country_code(&normalized) else {
        return false;
    };

    let result = check_vat_by_country(&normalized, country);
    result.is_supported_country && result.is_valid
}

fn prefixed_vat_country_code(value: &str) -> Option<&str> {
    if value.len() < 4 {
        return None;
    }
    let country = &value[..2];
    country
        .chars()
        .all(|character| character.is_ascii_uppercase())
        .then_some(country)
}

pub(super) fn is_dutch_btw_tax_number(value: &str) -> bool {
    let normalized = normalized_ascii_identifier(value);
    if normalized.len() != 12
        || !normalized[..9]
            .chars()
            .all(|character| character.is_ascii_digit())
        || &normalized[9..10] != "B"
        || !normalized[10..]
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return false;
    }

    normalized[10..]
        .parse::<u8>()
        .is_ok_and(|suffix| (1..=99).contains(&suffix))
}

fn normalized_ascii_identifier(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_uppercase())
        .collect()
}
