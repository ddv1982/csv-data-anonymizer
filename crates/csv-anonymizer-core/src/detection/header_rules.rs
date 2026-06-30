use crate::types::{Confidence, DataType, DetectionResult};

use super::candidate::{DetectorCandidate, DetectorCandidateSpec, DetectorEvidence};
use super::header;
use super::scoring::{calculate_confidence, detection_result};
use super::validators::{
    is_dutch_btw_tax_number, is_formatted_phone_fallback, is_tax_id, is_unformatted_tax_id,
    is_us_ein, is_us_ssn, is_valid_phone_number, is_vat_id,
};
use super::value::is_unsigned_integer;

pub(in crate::detection) type HeaderDetector =
    fn(&str, &[&String], usize) -> Option<HeaderDetection>;

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
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
    total_non_empty: usize,
    rules: &[HeaderDetectionRule],
) -> Option<DetectionResult> {
    rules.iter().find_map(|rule| {
        (rule.detect)(column_name, non_empty_values, total_samples).map(|detection| {
            let trace_candidate = DetectorCandidate::from_spec(DetectorCandidateSpec {
                data_type: detection.result.data_type,
                reason: format!(
                    "{}: {} ({:?}, {:?} confidence)",
                    rule.trace_reason,
                    detection.signal.concept,
                    detection.signal.data_type,
                    detection.signal.confidence
                ),
                match_count: detection.result.sample_matches,
                total_considered: total_non_empty,
                confidence: detection.result.confidence,
                evidence: DetectorEvidence::Header,
                specificity: specificity_for_header(detection.result.data_type),
                order: 0,
            });

            detection_result(
                detection.result.data_type,
                detection.result.confidence,
                detection.result.sample_matches,
                detection.result.total_samples,
                total_non_empty,
                format!("{} {}", detection.signal.reason, rule.selected_reason),
                vec![trace_candidate.trace_item()],
            )
        })
    })
}

fn specificity_for_header(data_type: DataType) -> u8 {
    match data_type {
        DataType::TaxId => 100,
        DataType::Address => 90,
        DataType::Phone => 80,
        DataType::PostalCode => 70,
        DataType::NumericId => 60,
        DataType::FullName | DataType::FirstName | DataType::LastName => 50,
        _ => 20,
    }
}

pub(in crate::detection) fn detect_header_numeric_id(
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

pub(in crate::detection) fn detect_name_type(
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
