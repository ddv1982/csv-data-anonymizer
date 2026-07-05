use std::collections::HashSet;
use std::sync::OnceLock;

use regex::Regex;

use crate::types::{Confidence, DataType, DetectionResult, DetectionTraceItem};

use super::candidate::{DetectorCandidate, DetectorCandidateSpec, DetectorEvidence};
use super::locale::LocaleContext;
use super::national_id::is_national_id;
use super::scoring::{DetectorDecision, calculate_confidence, detection_result, trace_item};
use super::validators::{is_email, is_iban, is_phone_in_context, is_tax_id, is_url, is_vat_id};

type DetectionPredicate = fn(&str, &LocaleContext) -> bool;

pub(in crate::detection) fn detect_priority_pattern(
    values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    locale: &LocaleContext,
) -> std::result::Result<DetectionResult, Vec<DetectionTraceItem>> {
    let candidates = detection_priority()
        .into_iter()
        .enumerate()
        .map(|(order, (data_type, matches, reason))| {
            let match_count = values.iter().filter(|value| matches(value, locale)).count();
            let confidence = calculate_confidence(match_count, total_non_empty);

            DetectorCandidate::from_spec(DetectorCandidateSpec {
                data_type,
                reason: reason.to_string(),
                match_count,
                total_considered: total_non_empty,
                confidence,
                evidence: evidence_for(data_type),
                specificity: specificity_for(data_type),
                order,
            })
        })
        .collect();
    let decision = DetectorDecision::select(candidates);
    let trace_items = decision.trace_items();

    if let Some(selected) = decision.selected {
        return Ok(detection_result(
            selected.data_type,
            selected.confidence,
            selected.match_count,
            total_samples,
            total_non_empty,
            "Sample values matched a built-in pattern rule.",
            trace_items,
        ));
    }

    Err(trace_items)
}

pub(in crate::detection) fn detect_numeric_value_type(
    values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
) -> Option<DetectionResult> {
    let match_count = values
        .iter()
        .filter(|value| is_numeric_value(value))
        .count();
    let confidence = calculate_confidence(match_count, total_non_empty);

    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::NumericValue,
        confidence,
        sample_matches: match_count,
        total_samples,
        trace: None,
    })
}

pub(in crate::detection) fn detect_vat_value_type(
    values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
) -> Option<DetectionResult> {
    let match_count = values.iter().filter(|value| is_vat_id(value)).count();
    let confidence = calculate_confidence(match_count, total_non_empty);

    if confidence == Confidence::Low {
        return None;
    }

    Some(detection_result(
        DataType::TaxId,
        confidence,
        match_count,
        total_samples,
        total_non_empty,
        "Sample values matched the VAT country-specific validator.",
        vec![trace_item(
            DataType::TaxId,
            "validator:vat",
            match_count,
            total_non_empty,
            confidence,
            true,
        )],
    ))
}

pub(in crate::detection) fn detect_iban_value_type(
    values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
) -> Option<DetectionResult> {
    let match_count = values.iter().filter(|value| is_iban(value)).count();
    let confidence = calculate_confidence(match_count, total_non_empty);

    if confidence == Confidence::Low {
        return None;
    }

    Some(detection_result(
        DataType::String,
        confidence,
        match_count,
        total_samples,
        total_non_empty,
        "Sample values matched the IBAN checksum validator.",
        vec![trace_item(
            DataType::String,
            "validator:iban",
            match_count,
            total_non_empty,
            confidence,
            true,
        )],
    ))
}

pub(in crate::detection) fn detect_enum_type(non_empty_values: &[&String]) -> bool {
    if non_empty_values.len() <= 10 {
        return false;
    }
    let unique_values: HashSet<&str> = non_empty_values
        .iter()
        .map(|value| value.as_str())
        .collect();
    unique_values.len() <= 20
}

fn detection_priority() -> [(DataType, DetectionPredicate, &'static str); 14] {
    [
        (DataType::Email, email_predicate, "pattern rule"),
        (DataType::Uuid, uuid_predicate, "pattern rule"),
        (DataType::Timestamp, timestamp_predicate, "pattern rule"),
        (DataType::Phone, phone_predicate, "pattern rule"),
        (DataType::IpAddress, ip_address_predicate, "pattern rule"),
        (DataType::MacAddress, mac_address_predicate, "pattern rule"),
        (DataType::Url, url_predicate, "pattern rule"),
        (DataType::TaxId, tax_id_predicate, "pattern rule"),
        (DataType::TaxId, is_national_id_value, "validator:idsmith"),
        (DataType::Boolean, boolean_predicate, "pattern rule"),
        (DataType::Currency, currency_predicate, "pattern rule"),
        (DataType::Percentage, percentage_predicate, "pattern rule"),
        (DataType::NumericId, numeric_id_predicate, "pattern rule"),
        (
            DataType::CountryCode,
            country_code_predicate,
            "pattern rule",
        ),
    ]
}

fn email_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_email(value)
}

fn uuid_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_uuid(value)
}

fn timestamp_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_timestamp(value)
}

fn phone_predicate(value: &str, locale: &LocaleContext) -> bool {
    is_phone_in_context(value, locale)
}

fn ip_address_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_ip_address(value)
}

fn mac_address_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_mac_address(value)
}

fn url_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_url(value)
}

fn tax_id_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_tax_id(value)
}

fn is_national_id_value(value: &str, _locale: &LocaleContext) -> bool {
    is_national_id(value)
}

fn boolean_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_boolean(value)
}

fn currency_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_currency(value)
}

fn percentage_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_percentage(value)
}

fn numeric_id_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_numeric_id(value)
}

fn country_code_predicate(value: &str, _locale: &LocaleContext) -> bool {
    is_country_code(value)
}

fn evidence_for(data_type: DataType) -> DetectorEvidence {
    match data_type {
        DataType::Email | DataType::Phone | DataType::Url | DataType::TaxId => {
            DetectorEvidence::Validator
        }
        DataType::NumericId
        | DataType::NumericValue
        | DataType::Boolean
        | DataType::Currency
        | DataType::Percentage
        | DataType::CountryCode
        | DataType::Timestamp => DetectorEvidence::Shape,
        _ => DetectorEvidence::Pattern,
    }
}

fn specificity_for(data_type: DataType) -> u8 {
    match data_type {
        DataType::TaxId => 100,
        DataType::Uuid | DataType::MacAddress => 90,
        DataType::Email | DataType::Url | DataType::IpAddress => 80,
        DataType::Phone => 70,
        DataType::NumericId => 60,
        DataType::Timestamp => 50,
        DataType::Boolean | DataType::Currency | DataType::Percentage | DataType::CountryCode => 40,
        _ => 20,
    }
}

fn is_uuid(value: &str) -> bool {
    uuid_pattern().is_match(value)
}

pub(in crate::detection) fn is_timestamp(value: &str) -> bool {
    timestamp_pattern().is_match(value)
}

fn is_numeric_id(value: &str) -> bool {
    numeric_id_pattern().is_match(value)
}

pub(in crate::detection) fn is_unsigned_integer(value: &str) -> bool {
    unsigned_integer_pattern().is_match(value)
}

fn is_numeric_value(value: &str) -> bool {
    numeric_value_pattern().is_match(value)
}

fn is_ip_address(value: &str) -> bool {
    let parts: Vec<&str> = value.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 3
                && part.chars().all(|character| character.is_ascii_digit())
                && part.parse::<u8>().is_ok()
        })
}

fn is_mac_address(value: &str) -> bool {
    mac_address_pattern().is_match(value)
}

fn is_boolean(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "true" | "false" | "yes" | "no"
    )
}

fn is_currency(value: &str) -> bool {
    currency_pattern().is_match(value)
}

fn is_percentage(value: &str) -> bool {
    percentage_pattern().is_match(value)
}

fn is_country_code(value: &str) -> bool {
    country_code_pattern().is_match(value)
}

fn uuid_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
            .unwrap()
    })
}

fn timestamp_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN
        .get_or_init(|| Regex::new(r"^\d{4}-\d{2}-\d{2}([T ]\d{2}:\d{2}:\d{2}(\.\d+)?)?$").unwrap())
}

fn numeric_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\d{4,}$").unwrap())
}

fn unsigned_integer_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\d+$").unwrap())
}

fn numeric_value_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[+-]?(?:\d+|\d+\.\d+|\.\d+)$").unwrap())
}

fn mac_address_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}$").unwrap())
}

fn currency_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\$\s?(?:\d+|\d{1,3}(?:,\d{3})+)(?:\.\d{2})?$").unwrap())
}

fn percentage_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[+-]?\d+(?:\.\d+)?%$").unwrap())
}

fn country_code_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Z]{2}$").unwrap())
}
