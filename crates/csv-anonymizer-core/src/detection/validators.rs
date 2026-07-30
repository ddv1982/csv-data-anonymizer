use card_validate::Validate as CardValidate;
use ein::Ein;
use email_address::{EmailAddress, Options as EmailOptions};
use iban::Iban;
use phonenumber::{country, parse as parse_phone_number};
use regex::Regex;
use ssn::Ssn;
use std::convert::TryFrom;
use std::sync::OnceLock;
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
    // An explicit international prefix declares the value to be a phone number, so
    // one parse settles it — including vanity spellings like `+1800FLOWERS`.
    if trimmed.starts_with('+') {
        return parse_phone_number(None, trimmed).is_ok_and(|number| number.is_valid());
    }

    if !can_be_swept_for_phone_region(trimmed) {
        return false;
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

/// Whether `value` is worth trying against every candidate region.
///
/// Without a `+` prefix there is no country to parse against, so the only way to
/// know is to try each of the 54 regions in `world_phone_regions` until one
/// validates. A value that is *not* a phone number pays the full sweep before
/// concluding so, and libphonenumber maps letters to digits on the way — meaning
/// free text is scored as a candidate vanity number, 54 times over. Measured on 20
/// phone-labeled columns of ordinary prose: **157 seconds**, against 33ms for the
/// same data under a header the phone rule ignores.
///
/// Requiring the value to be free of letters cuts that to the values a sweep could
/// actually confirm. The cost is vanity numbers written without a country prefix:
/// `1800FLOWERS` is no longer recognized, while `+1800FLOWERS` and every all-digit
/// form still are. That is an acceptable trade twice over — such numbers are a US
/// marketing convention rather than a formatting people use for their own numbers,
/// and a published vanity line is a business contact, not the personal data this
/// tool exists to protect.
///
/// An *extension* is not part of that trade, which a plain letter test quietly made
/// it: `020 1234567 ext 45` and `(415) 234-0100 x89` both contain letters, and both
/// are ordinary ways to write a business number. Testing the value with the extension
/// removed keeps them, and keeps prose out — prose carries letters throughout, so
/// dropping one trailing suffix cannot rescue it.
fn can_be_swept_for_phone_region(value: &str) -> bool {
    !without_phone_extension(value)
        .chars()
        .any(|character| character.is_ascii_alphabetic())
}

/// `value` without a trailing extension suffix: `ext 45`, `ext. 45`, `x89`.
///
/// Only for deciding whether the sweep is worth attempting. libphonenumber parses
/// extensions itself, so the *original* value is what gets parsed — stripping here
/// and passing the remainder on would throw away a digit run the parser wants.
fn without_phone_extension(value: &str) -> &str {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        // Longest alternative first so `extension` is not shadowed by `ext`.
        Regex::new(r"(?i)[\s,;.\-]*(?:extension|ext|x)\.?\s*\d{1,6}\s*$")
            .expect("phone extension pattern should compile")
    });
    match pattern.find(value) {
        Some(found) => &value[..found.start()],
        None => value,
    }
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

/// Whether a whole column value should count as a phone number.
///
/// The libphonenumber check alone rejects real-world entries it has no region
/// rule for — notably the reserved US 555 range that fixtures and test data are
/// full of — so a formatted-shape fallback backs it up.
///
/// This is the *column* bar, and it is deliberately looser than the free-text bar
/// in [`is_formatted_phone_span`]: a column is only asked this once its header has
/// already said "phone", so an unformatted digit run in it is very likely a phone
/// number, and the region sweep is affordable on a bounded sample. Free text has
/// neither the header evidence nor the budget. The two bars differ on exactly one
/// case — a bare digit run — and each path documents why it draws the line where
/// it does.
pub(super) fn is_phone_value(value: &str) -> bool {
    let trimmed = value.trim();
    // Cheap shape check first. The two are OR'd, so the order cannot change the
    // answer, but `is_valid_phone_number` sweeps dozens of regions through
    // libphonenumber, and a column rule evaluates this once per sampled value.
    is_formatted_phone_fallback(trimmed, 7, true) || is_valid_phone_number(trimmed)
}

/// Whether free text holds a *formatted* phone number at this position.
///
/// Inline span scanning runs on every cell of every column, which rules out the
/// region sweep in [`is_phone_value`]: bare digit runs fail the cheap shape check
/// and would fall through to it, and a ten-digit numeric ID is exactly the common
/// case. Requiring the formatting — a `+` prefix or internal separators — is both
/// cheap and the more precise bar for free text, where a bare run of ten digits is
/// far more often an identifier than a phone number.
///
/// Failing this does not mean the span is discarded: an unformatted run is still
/// reported, at Low confidence. That is enough for the free-text workflow to offer
/// it for redaction, and `analyze_column_privacy` ignores Low findings when it
/// computes risk, so the run cannot escalate a column on its own. See
/// `spans::pattern_span_specs`.
pub(super) fn is_formatted_phone_span(value: &str) -> bool {
    is_formatted_phone_fallback(value.trim(), 7, true)
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
