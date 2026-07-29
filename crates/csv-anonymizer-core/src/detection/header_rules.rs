use crate::types::{Confidence, DataType, DetectionResult};

use super::header;
use super::locale::LocaleContext;
use super::postal::postal_match_country;
use super::scoring::{calculate_confidence, detection_result, trace_item};
use super::validators::{
    is_dutch_btw_tax_number, is_phone_value, is_tax_id, is_unformatted_tax_id, is_us_ein,
    is_us_ssn, is_vat_id,
};
use super::value::is_unsigned_integer;

pub(in crate::detection) type HeaderDetector =
    fn(&header::HeaderAnalysis, &[&String], usize, &LocaleContext) -> Option<HeaderDetection>;

pub(in crate::detection) struct HeaderDetection {
    result: DetectionResult,
    signal: header::HeaderSignal,
}

#[derive(Clone, Copy)]
pub(in crate::detection) struct HeaderDetectionRule {
    pub detect: HeaderDetector,
    pub selected_reason: &'static str,
    pub trace_reason: &'static str,
}

pub(in crate::detection) fn early_header_detection_rules() -> [HeaderDetectionRule; 4] {
    [
        HeaderDetectionRule {
            detect: detect_header_phone,
            selected_reason: "Header terms and sample shape matched phone detection.",
            trace_reason: "header phone rule",
        },
        HeaderDetectionRule {
            detect: detect_header_postal_code,
            selected_reason: "Header terms and sample shape matched postal code detection.",
            trace_reason: "header postal code rule",
        },
        HeaderDetectionRule {
            detect: detect_header_address,
            selected_reason: "Header terms and sample shape matched address detection.",
            trace_reason: "header address rule",
        },
        HeaderDetectionRule {
            detect: detect_header_tax_id,
            selected_reason: "Header terms and sample shape matched tax ID detection.",
            trace_reason: "header tax ID rule",
        },
    ]
}

pub(in crate::detection) fn first_header_detection(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    rules: &[HeaderDetectionRule],
    locale: &LocaleContext,
) -> Option<DetectionResult> {
    // The first matching rule wins outright: header rules are not scored against
    // each other, so this reports the match directly as a trace item rather than
    // building a candidate for a selection that never happens.
    rules.iter().find_map(|rule| {
        (rule.detect)(header, non_empty_values, total_samples, locale).map(|detection| {
            detection_result(
                detection.result.data_type,
                detection.result.confidence,
                detection.result.sample_matches,
                detection.result.total_samples,
                total_non_empty,
                format!("{} {}", detection.signal.reason, rule.selected_reason),
                vec![trace_item(
                    detection.result.data_type,
                    format!(
                        "{}: {} ({:?}, {:?} confidence)",
                        rule.trace_reason,
                        detection.signal.concept,
                        detection.signal.data_type,
                        detection.signal.confidence
                    ),
                    detection.result.sample_matches,
                    total_non_empty,
                    detection.result.confidence,
                    detection.result.confidence != Confidence::Low,
                )],
            )
        })
    })
}

/// The shape every fixed-type header rule shares.
///
/// A taxonomy signal for `kinds` gates the rule — no signal, no detection. A
/// value predicate then decides how many sampled values corroborate the header,
/// and the column is classified as `data_type` only if that ratio clears Low.
/// Header evidence boosts and disambiguates; it never classifies on its own.
///
/// `count_matches` receives the parsed header terms because some rules widen
/// their predicate based on further header context (Dutch BTW, US tax IDs).
fn header_signal_detection(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    kinds: &[&str],
    data_type: DataType,
    count_matches: impl Fn(&header::HeaderTerms, &[&String]) -> usize,
) -> Option<HeaderDetection> {
    let signal = header.best_for_kinds(kinds)?;

    let match_count = count_matches(header.terms(), non_empty_values);
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(HeaderDetection {
        result: DetectionResult {
            data_type,
            confidence,
            sample_matches: match_count,
            total_samples,
            trace: None,
        },
        signal,
    })
}

fn count_matching<'a>(values: &[&'a String], predicate: impl Fn(&'a str) -> bool) -> usize {
    values.iter().filter(|value| predicate(value)).count()
}

pub(in crate::detection) fn detect_header_numeric_id(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["numeric_id", "account_number"],
        DataType::NumericId,
        |_, values| count_matching(values, is_unsigned_integer),
    )
}

fn detect_header_phone(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["phone"],
        DataType::Phone,
        |_, values| count_matching(values, is_phone_value),
    )
}

fn detect_header_postal_code(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["postal_code"],
        DataType::PostalCode,
        |_, values| {
            // With a known file locale, header-labeled postal columns are held to
            // the per-country formats: when any sample matches a country format,
            // only those matches count. Without locale context (the default), the
            // loose shape check stands unchanged, preserving columns that mix
            // unambiguous formats with unlabeled bare-digit zips (e.g. GB + US).
            let context_match_count = if locale.countries().is_empty() {
                0
            } else {
                count_matching(values, |value| {
                    postal_match_country(value, locale).is_some()
                })
            };
            if context_match_count > 0 {
                context_match_count
            } else {
                count_matching(values, is_postal_code)
            }
        },
    )
}

fn detect_header_address(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["address"],
        DataType::Address,
        |_, values| count_matching(values, is_plausible_address),
    )
}

fn detect_header_tax_id(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    header_signal_detection(
        header,
        non_empty_values,
        total_samples,
        &["tax_id"],
        DataType::TaxId,
        |header_terms, values| {
            let allow_dutch_btw_number = has_dutch_btw_context(header_terms);
            let tax_id_context = tax_id_header_context(header_terms);
            count_matching(values, |value| {
                is_tax_id(value)
                    || is_contextual_unformatted_us_tax_id(value, tax_id_context)
                    || is_vat_id(value)
                    || (allow_dutch_btw_number && is_dutch_btw_tax_number(value))
            })
        },
    )
}

pub(in crate::detection) fn detect_name_type(
    header: &header::HeaderAnalysis,
    non_empty_values: &[&String],
    total_samples: usize,
    _locale: &LocaleContext,
) -> Option<HeaderDetection> {
    let signal =
        header.best_for_kinds(&["first_name", "last_name", "full_name", "generic_name"])?;
    let data_type = signal.data_type;
    let mut match_count = non_empty_values
        .iter()
        .filter(|value| match data_type {
            DataType::FirstName => is_plausible_name_part(value, 2),
            DataType::LastName => is_plausible_name_part(value, 4),
            DataType::FullName => is_plausible_full_name(value),
            _ => false,
        })
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());

    if confidence == Confidence::Low {
        if data_type == DataType::FullName && header.matches_kind("generic_name") {
            match_count = non_empty_values
                .iter()
                .filter(|value| is_plausible_generic_single_name(value))
                .count();
            let confidence = calculate_confidence(match_count, non_empty_values.len());

            if confidence != Confidence::Low {
                return Some(HeaderDetection {
                    result: DetectionResult {
                        data_type: DataType::FirstName,
                        confidence,
                        sample_matches: match_count,
                        total_samples,
                        trace: None,
                    },
                    signal,
                });
            }
        }

        return None;
    }

    Some(HeaderDetection {
        result: DetectionResult {
            data_type,
            confidence,
            sample_matches: match_count,
            total_samples,
            trace: None,
        },
        signal,
    })
}

fn is_postal_code(value: &str) -> bool {
    let trimmed = value.trim();
    (3..=12).contains(&trimmed.len())
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, ' ' | '-'))
}

pub(in crate::detection) fn is_plausible_address(value: &str) -> bool {
    let trimmed = value.trim();
    if !(5..=200).contains(&trimmed.len())
        || !trimmed.chars().any(|character| character.is_ascii_digit())
        || !trimmed.chars().any(|character| character.is_alphabetic())
    {
        return false;
    }

    let normalized = trimmed.to_lowercase();
    if contains_address_keyword(&normalized) {
        return true;
    }

    if trimmed
        .chars()
        .any(|character| character.is_alphabetic() && !character.is_ascii())
        && trimmed.contains('-')
    {
        return true;
    }

    trimmed.contains(',') || trimmed.matches(char::is_whitespace).count() >= 2
}

pub(in crate::detection) fn has_dutch_btw_context(terms: &header::HeaderTerms) -> bool {
    matches!(
        terms.compact.as_str(),
        "btw" | "btwnr" | "btwnummer" | "btwid" | "btwidentificatienummer" | "omzetbelastingnummer"
    ) || terms.compact.ends_with("btwnummer")
        || terms.compact.ends_with("omzetbelastingnummer")
}

#[derive(Clone, Copy)]
pub(in crate::detection) enum TaxIdHeaderContext {
    Generic,
    Ssn,
    Ein,
}

pub(in crate::detection) fn tax_id_header_context(
    terms: &header::HeaderTerms,
) -> TaxIdHeaderContext {
    match terms.compact.as_str() {
        "ssn" | "socialsecuritynumber" => TaxIdHeaderContext::Ssn,
        "ein" | "employeridentificationnumber" => TaxIdHeaderContext::Ein,
        _ => TaxIdHeaderContext::Generic,
    }
}

pub(in crate::detection) fn is_contextual_unformatted_us_tax_id(
    value: &str,
    context: TaxIdHeaderContext,
) -> bool {
    match context {
        TaxIdHeaderContext::Ssn => is_us_ssn(value),
        TaxIdHeaderContext::Ein => is_us_ein(value),
        TaxIdHeaderContext::Generic => is_unformatted_tax_id(value),
    }
}

pub(in crate::detection) fn address_keywords() -> &'static [&'static str] {
    &[
        " st",
        " street",
        " ave",
        " avenue",
        " rd",
        " road",
        " blvd",
        " boulevard",
        " dr",
        " drive",
        " ln",
        " lane",
        " way",
        " court",
        " ct",
        "straat",
        "weg",
        "laan",
        "plein",
        "strasse",
        "straße",
        "platz",
        "allee",
        "rue",
        "avenue",
        "boulevard",
        "calle",
        "avenida",
        "carrera",
        "rua",
        "travessa",
        "via",
        "viale",
        "piazza",
    ]
}

pub(in crate::detection) fn contains_address_keyword(normalized: &str) -> bool {
    normalized
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .any(|token| {
            address_keywords().iter().any(|keyword| {
                let keyword = keyword.trim();
                token == keyword
                    || compound_address_suffixes().contains(&keyword)
                        && token.len() > keyword.len()
                        && token.ends_with(keyword)
            })
        })
}

fn compound_address_suffixes() -> &'static [&'static str] {
    &[
        "straat", "weg", "laan", "plein", "strasse", "straße", "platz", "allee",
    ]
}

fn is_plausible_name_part(value: &str, max_tokens: usize) -> bool {
    let trimmed = value.trim();
    if !(2..=80).contains(&trimmed.len()) {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    !tokens.is_empty()
        && tokens.len() <= max_tokens
        && tokens.iter().all(|token| is_plausible_name_token(token))
}

fn is_plausible_full_name(value: &str) -> bool {
    let trimmed = value.trim();
    if !(5..=120).contains(&trimmed.len()) {
        return false;
    }

    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    (2..=6).contains(&tokens.len()) && tokens.iter().all(|token| is_plausible_name_token(token))
}

fn is_plausible_generic_single_name(value: &str) -> bool {
    let trimmed = value.trim();
    is_plausible_name_part(trimmed, 1)
        && trimmed
            .chars()
            .next()
            .is_some_and(|character| character.is_uppercase())
}

fn is_plausible_name_token(token: &str) -> bool {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_alphabetic()
        && chars.all(|character| character.is_alphabetic() || matches!(character, '\'' | '-' | '.'))
}
