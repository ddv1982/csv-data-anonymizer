use crate::types::{
    Confidence, DataType, DetectionResult, DetectionTrace, DetectionTraceItem, EmptyFormat, PiiRisk,
};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;
use validators::{
    is_dutch_btw_tax_number, is_email, is_formatted_phone_fallback, is_iban, is_phone, is_tax_id,
    is_unformatted_tax_id, is_url, is_us_ein, is_us_ssn, is_valid_phone_number, is_vat_id,
};

mod header;
mod privacy;
mod spans;
mod validators;
pub use privacy::{ColumnPrivacyAnalysis, analyze_column_privacy, max_pii_risk};
pub use spans::{PrivacySpan, collect_privacy_spans};
#[cfg(test)]
use validators::is_payment_card_number;

pub fn is_empty_value(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("null")
}

pub(super) fn utf16_index_for_byte(value: &str, byte_index: usize) -> usize {
    match value.get(..byte_index) {
        Some(prefix) => utf16_len(prefix),
        None => value
            .char_indices()
            .take_while(|(index, _)| *index < byte_index)
            .map(|(_, character)| character.len_utf16())
            .sum(),
    }
}

pub(super) fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

pub fn detect_column_type(values: &[String]) -> DetectionResult {
    detect_column_type_with_name("", values)
}

pub fn detect_column_type_with_name(column_name: &str, values: &[String]) -> DetectionResult {
    let non_empty_values: Vec<&String> = values
        .iter()
        .filter(|value| !is_empty_value(value))
        .collect();
    let total_non_empty = non_empty_values.len();

    if total_non_empty == 0 {
        return detection_result(
            DataType::Unknown,
            Confidence::Low,
            0,
            values.len(),
            total_non_empty,
            "No non-empty sample values were available for detection.",
            Vec::new(),
        );
    }

    if let Some(result) = first_header_detection(
        column_name,
        &non_empty_values,
        values.len(),
        total_non_empty,
        &early_header_detection_rules(),
    ) {
        return result;
    }

    if let Some(result) = detect_vat_value_type(values, total_non_empty) {
        return result;
    }

    if let Some(result) = detect_iban_value_type(values, total_non_empty) {
        return result;
    }

    let candidates = match detect_priority_pattern(values, total_non_empty) {
        Ok(result) => return result,
        Err(candidates) => candidates,
    };

    if let Some(result) = first_header_detection(
        column_name,
        &non_empty_values,
        values.len(),
        total_non_empty,
        &[HeaderDetectionRule {
            detect: detect_header_numeric_id,
            selected_reason: "Header terms and integer sample shape matched numeric ID detection.",
            trace_reason: "header numeric ID rule",
        }],
    ) {
        return result;
    }

    if let Some(result) = detect_numeric_value_type(values, total_non_empty) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Sample values matched numeric value detection after identifier rules were rejected.",
            "numeric value rule",
        );
    }

    if let Some(result) = first_header_detection(
        column_name,
        &non_empty_values,
        values.len(),
        total_non_empty,
        &[HeaderDetectionRule {
            detect: detect_name_type,
            selected_reason: "Header terms and sample shape matched name detection.",
            trace_reason: "header name rule",
        }],
    ) {
        return result;
    }

    if detect_enum_type(&non_empty_values) {
        return detection_result(
            DataType::Enum,
            Confidence::High,
            non_empty_values.len(),
            values.len(),
            total_non_empty,
            "Sample values formed a repeated finite set.",
            vec![trace_item(
                DataType::Enum,
                "finite repeated values",
                non_empty_values.len(),
                total_non_empty,
                Confidence::High,
                true,
            )],
        );
    }

    detection_result(
        DataType::String,
        Confidence::Low,
        non_empty_values.len(),
        values.len(),
        total_non_empty,
        "No sensitive pattern, header, numeric, name, or enum rule passed the threshold.",
        candidates,
    )
}

type HeaderDetector = fn(&str, &[&String], usize) -> Option<HeaderDetection>;

struct HeaderDetection {
    result: DetectionResult,
    signal: header::HeaderSignal,
}

#[derive(Clone, Copy)]
struct HeaderDetectionRule {
    detect: HeaderDetector,
    selected_reason: &'static str,
    trace_reason: &'static str,
}

fn early_header_detection_rules() -> [HeaderDetectionRule; 4] {
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

fn first_header_detection(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    rules: &[HeaderDetectionRule],
) -> Option<DetectionResult> {
    rules.iter().find_map(|rule| {
        (rule.detect)(column_name, non_empty_values, total_samples).map(|detection| {
            attach_single_trace(
                detection.result,
                total_non_empty,
                format!("{} {}", detection.signal.reason, rule.selected_reason),
                format!(
                    "{}: {} ({:?}, {:?} confidence)",
                    rule.trace_reason,
                    detection.signal.concept,
                    detection.signal.data_type,
                    detection.signal.confidence
                ),
            )
        })
    })
}

fn detect_priority_pattern(
    values: &[String],
    total_non_empty: usize,
) -> std::result::Result<DetectionResult, Vec<DetectionTraceItem>> {
    let mut candidates = Vec::new();
    for (data_type, matches) in detection_priority() {
        let match_count = values
            .iter()
            .filter(|value| !is_empty_value(value) && matches(value))
            .count();
        let confidence = calculate_confidence(match_count, total_non_empty);
        let accepted = confidence != Confidence::Low;
        candidates.push(trace_item(
            data_type,
            "pattern rule",
            match_count,
            total_non_empty,
            confidence,
            accepted,
        ));

        if accepted {
            return Ok(detection_result(
                data_type,
                confidence,
                match_count,
                values.len(),
                total_non_empty,
                "Sample values matched a built-in pattern rule.",
                candidates,
            ));
        }
    }
    Err(candidates)
}

fn detection_result(
    data_type: DataType,
    confidence: Confidence,
    sample_matches: usize,
    total_samples: usize,
    total_non_empty: usize,
    selected_reason: impl Into<String>,
    candidates: Vec<DetectionTraceItem>,
) -> DetectionResult {
    DetectionResult {
        data_type,
        confidence,
        sample_matches,
        total_samples,
        trace: Some(DetectionTrace {
            summary: detection_summary(data_type, confidence, sample_matches, total_non_empty),
            selected_reason: selected_reason.into(),
            total_non_empty,
            candidates,
        }),
    }
}

fn attach_single_trace(
    mut result: DetectionResult,
    total_non_empty: usize,
    selected_reason: impl Into<String>,
    reason: impl Into<String>,
) -> DetectionResult {
    let reason = reason.into();
    result.trace = Some(DetectionTrace {
        summary: detection_summary(
            result.data_type,
            result.confidence,
            result.sample_matches,
            total_non_empty,
        ),
        selected_reason: selected_reason.into(),
        total_non_empty,
        candidates: vec![trace_item(
            result.data_type,
            reason,
            result.sample_matches,
            total_non_empty,
            result.confidence,
            true,
        )],
    });
    result
}

fn trace_item(
    data_type: DataType,
    reason: impl Into<String>,
    match_count: usize,
    total_considered: usize,
    confidence: Confidence,
    accepted: bool,
) -> DetectionTraceItem {
    DetectionTraceItem {
        data_type,
        reason: reason.into(),
        match_count,
        total_considered,
        confidence,
        accepted,
    }
}

fn detection_summary(
    data_type: DataType,
    confidence: Confidence,
    sample_matches: usize,
    total_non_empty: usize,
) -> String {
    format!(
        "{data_type:?} selected with {confidence:?} confidence from {sample_matches}/{total_non_empty} non-empty sample value(s)."
    )
}

pub fn classify_pii_risk(data_type: DataType) -> PiiRisk {
    match data_type {
        DataType::Email
        | DataType::Phone
        | DataType::FullName
        | DataType::Address
        | DataType::TaxId => PiiRisk::High,
        DataType::FirstName
        | DataType::LastName
        | DataType::Uuid
        | DataType::NumericId
        | DataType::PostalCode
        | DataType::IpAddress
        | DataType::Url
        | DataType::MacAddress => PiiRisk::Medium,
        DataType::Timestamp
        | DataType::NumericValue
        | DataType::Boolean
        | DataType::Currency
        | DataType::Percentage
        | DataType::CountryCode
        | DataType::Enum
        | DataType::String
        | DataType::Unknown => PiiRisk::Low,
    }
}

pub fn detect_empty_format(values: &[String]) -> EmptyFormat {
    let mut has_empty_string = false;
    let mut has_null_string = false;

    for value in values {
        if value.is_empty() {
            has_empty_string = true;
        } else if value.eq_ignore_ascii_case("null") {
            has_null_string = true;
        }

        if has_empty_string && has_null_string {
            return EmptyFormat::Mixed;
        }
    }

    if has_null_string {
        EmptyFormat::Null
    } else {
        EmptyFormat::EmptyString
    }
}

fn calculate_confidence(match_count: usize, total_non_empty: usize) -> Confidence {
    if total_non_empty == 0 {
        return Confidence::Low;
    }

    let percentage = match_count as f64 / total_non_empty as f64;
    if percentage >= 0.8 {
        Confidence::High
    } else if percentage >= 0.5 {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

fn detect_enum_type(non_empty_values: &[&String]) -> bool {
    if non_empty_values.len() <= 10 {
        return false;
    }
    let unique_values: HashSet<&str> = non_empty_values
        .iter()
        .map(|value| value.as_str())
        .collect();
    unique_values.len() <= 20
}

type DetectionPredicate = fn(&str) -> bool;

fn detection_priority() -> [(DataType, DetectionPredicate); 13] {
    [
        (DataType::Email, is_email),
        (DataType::Uuid, is_uuid),
        (DataType::Timestamp, is_timestamp),
        (DataType::Phone, is_phone),
        (DataType::IpAddress, is_ip_address),
        (DataType::MacAddress, is_mac_address),
        (DataType::Url, is_url),
        (DataType::TaxId, is_tax_id),
        (DataType::Boolean, is_boolean),
        (DataType::Currency, is_currency),
        (DataType::Percentage, is_percentage),
        (DataType::NumericId, is_numeric_id),
        (DataType::CountryCode, is_country_code),
    ]
}

fn is_uuid(value: &str) -> bool {
    uuid_pattern().is_match(value)
}

fn is_timestamp(value: &str) -> bool {
    timestamp_pattern().is_match(value)
}

fn is_numeric_id(value: &str) -> bool {
    numeric_id_pattern().is_match(value)
}

fn is_unsigned_integer(value: &str) -> bool {
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

fn detect_header_numeric_id(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<HeaderDetection> {
    let header_terms = header::terms(column_name);
    let signal = header::best_signal_for_kinds(&header_terms, &["numeric_id", "account_number"])?;

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_unsigned_integer(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());

    if confidence == Confidence::Low {
        return None;
    }

    Some(HeaderDetection {
        result: DetectionResult {
            data_type: DataType::NumericId,
            confidence,
            sample_matches: match_count,
            total_samples,
            trace: None,
        },
        signal,
    })
}

fn detect_header_phone(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<HeaderDetection> {
    let header_terms = header::terms(column_name);
    let signal = header::best_signal_for_kinds(&header_terms, &["phone"])?;

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_header_phone_value(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(HeaderDetection {
        result: DetectionResult {
            data_type: DataType::Phone,
            confidence,
            sample_matches: match_count,
            total_samples,
            trace: None,
        },
        signal,
    })
}

fn detect_numeric_value_type(values: &[String], total_non_empty: usize) -> Option<DetectionResult> {
    let match_count = values
        .iter()
        .filter(|value| !is_empty_value(value) && is_numeric_value(value))
        .count();
    let confidence = calculate_confidence(match_count, total_non_empty);

    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::NumericValue,
        confidence,
        sample_matches: match_count,
        total_samples: values.len(),
        trace: None,
    })
}

fn detect_vat_value_type(values: &[String], total_non_empty: usize) -> Option<DetectionResult> {
    let match_count = values
        .iter()
        .filter(|value| !is_empty_value(value) && is_vat_id(value))
        .count();
    let confidence = calculate_confidence(match_count, total_non_empty);

    if confidence == Confidence::Low {
        return None;
    }

    Some(detection_result(
        DataType::TaxId,
        confidence,
        match_count,
        values.len(),
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

fn detect_iban_value_type(values: &[String], total_non_empty: usize) -> Option<DetectionResult> {
    let match_count = values
        .iter()
        .filter(|value| !is_empty_value(value) && is_iban(value))
        .count();
    let confidence = calculate_confidence(match_count, total_non_empty);

    if confidence == Confidence::Low {
        return None;
    }

    Some(detection_result(
        DataType::String,
        confidence,
        match_count,
        values.len(),
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

fn detect_header_postal_code(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<HeaderDetection> {
    let header_terms = header::terms(column_name);
    let signal = header::best_signal_for_kinds(&header_terms, &["postal_code"])?;

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_postal_code(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(HeaderDetection {
        result: DetectionResult {
            data_type: DataType::PostalCode,
            confidence,
            sample_matches: match_count,
            total_samples,
            trace: None,
        },
        signal,
    })
}

fn detect_header_address(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<HeaderDetection> {
    let header_terms = header::terms(column_name);
    let signal = header::best_signal_for_kinds(&header_terms, &["address"])?;

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_plausible_address(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(HeaderDetection {
        result: DetectionResult {
            data_type: DataType::Address,
            confidence,
            sample_matches: match_count,
            total_samples,
            trace: None,
        },
        signal,
    })
}

fn detect_header_tax_id(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<HeaderDetection> {
    let header_terms = header::terms(column_name);
    let signal = header::best_signal_for_kinds(&header_terms, &["tax_id"])?;
    let allow_dutch_btw_number = has_dutch_btw_context(&header_terms);
    let tax_id_context = tax_id_header_context(&header_terms);

    let match_count = non_empty_values
        .iter()
        .filter(|value| {
            is_tax_id(value)
                || is_contextual_unformatted_us_tax_id(value, tax_id_context)
                || is_vat_id(value)
                || (allow_dutch_btw_number && is_dutch_btw_tax_number(value))
        })
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(HeaderDetection {
        result: DetectionResult {
            data_type: DataType::TaxId,
            confidence,
            sample_matches: match_count,
            total_samples,
            trace: None,
        },
        signal,
    })
}

fn is_header_phone_value(value: &str) -> bool {
    let trimmed = value.trim();
    is_valid_phone_number(trimmed) || is_formatted_phone_fallback(trimmed, 7, true)
}

fn is_postal_code(value: &str) -> bool {
    let trimmed = value.trim();
    (3..=12).contains(&trimmed.len())
        && trimmed.chars().any(|character| character.is_ascii_digit())
        && trimmed
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, ' ' | '-'))
}

fn is_plausible_address(value: &str) -> bool {
    let trimmed = value.trim();
    if !(5..=200).contains(&trimmed.len())
        || !trimmed.chars().any(|character| character.is_ascii_digit())
        || !trimmed.chars().any(|character| character.is_alphabetic())
    {
        return false;
    }

    let normalized = trimmed.to_lowercase();
    if address_keywords()
        .iter()
        .any(|keyword| normalized.contains(keyword))
    {
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

fn address_keywords() -> &'static [&'static str] {
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

fn detect_name_type(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<HeaderDetection> {
    let header_terms = header::terms(column_name);
    let signal = header::best_signal_for_kinds(
        &header_terms,
        &["first_name", "last_name", "full_name", "generic_name"],
    )?;
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
        if data_type == DataType::FullName && header::infer_generic_name(column_name) {
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

#[cfg(test)]
mod tests;
