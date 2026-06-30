use crate::types::{Confidence, DataType, PrivacyFinding, PrivacyFindingKind};
use regex::Regex;
use std::sync::OnceLock;

use super::utf16_index_for_byte;
use super::validators::{is_iban, is_payment_card_number, is_tax_id, is_vat_id};

#[derive(Debug, Clone, Copy)]
pub struct PrivacySpan<'a> {
    pub field_name: &'static str,
    pub kind: PrivacyFindingKind,
    pub data_type: DataType,
    pub start: usize,
    pub end: usize,
    pub value: &'a str,
    pub confidence: Confidence,
    pub score: u8,
    pub detector: &'static str,
    pub reason: &'static str,
    pub priority: usize,
}

struct SpanSpec {
    field_name: &'static str,
    kind: PrivacyFindingKind,
    data_type: DataType,
    regex: &'static Regex,
    confidence: Confidence,
    score: u8,
    detector: &'static str,
    reason: &'static str,
    priority: usize,
}

pub fn collect_privacy_spans(content: &str) -> Vec<PrivacySpan<'_>> {
    let mut candidates = Vec::new();
    push_secret_spans(content, &mut candidates);
    push_account_number_spans(content, &mut candidates);
    for spec in pattern_span_specs() {
        push_pattern_spans(content, &mut candidates, spec);
    }
    push_tax_id_spans(content, &mut candidates);
    select_non_overlapping_spans(candidates)
}

fn pattern_span_specs() -> [SpanSpec; 7] {
    [
        SpanSpec {
            field_name: "email",
            kind: PrivacyFindingKind::Contact,
            data_type: DataType::Email,
            regex: inline_email_pattern(),
            confidence: Confidence::High,
            score: 96,
            detector: "pattern:email",
            reason: "Email address pattern.",
            priority: 20,
        },
        SpanSpec {
            field_name: "url",
            kind: PrivacyFindingKind::Url,
            data_type: DataType::Url,
            regex: inline_url_pattern(),
            confidence: Confidence::Medium,
            score: 78,
            detector: "pattern:url",
            reason: "URL pattern.",
            priority: 30,
        },
        SpanSpec {
            field_name: "uuid",
            kind: PrivacyFindingKind::NetworkOrDeviceId,
            data_type: DataType::Uuid,
            regex: inline_uuid_pattern(),
            confidence: Confidence::Medium,
            score: 76,
            detector: "pattern:uuid",
            reason: "UUID-like identifier pattern.",
            priority: 40,
        },
        SpanSpec {
            field_name: "date",
            kind: PrivacyFindingKind::PrivateDate,
            data_type: DataType::Timestamp,
            regex: inline_timestamp_pattern(),
            confidence: Confidence::Low,
            score: 54,
            detector: "pattern:date",
            reason: "Date or timestamp pattern; review context before treating it as private.",
            priority: 50,
        },
        SpanSpec {
            field_name: "ipAddress",
            kind: PrivacyFindingKind::NetworkOrDeviceId,
            data_type: DataType::IpAddress,
            regex: inline_ip_address_pattern(),
            confidence: Confidence::Medium,
            score: 78,
            detector: "pattern:ip",
            reason: "IPv4 address pattern.",
            priority: 60,
        },
        SpanSpec {
            field_name: "macAddress",
            kind: PrivacyFindingKind::NetworkOrDeviceId,
            data_type: DataType::MacAddress,
            regex: inline_mac_address_pattern(),
            confidence: Confidence::Medium,
            score: 76,
            detector: "pattern:mac",
            reason: "MAC address pattern.",
            priority: 70,
        },
        SpanSpec {
            field_name: "phone",
            kind: PrivacyFindingKind::Contact,
            data_type: DataType::Phone,
            regex: inline_phone_pattern(),
            confidence: Confidence::High,
            score: 90,
            detector: "pattern:phone",
            reason: "Formatted phone number pattern.",
            priority: 90,
        },
    ]
}

fn push_pattern_spans<'a>(content: &'a str, candidates: &mut Vec<PrivacySpan<'a>>, spec: SpanSpec) {
    for regex_match in spec.regex.find_iter(content) {
        candidates.push(PrivacySpan {
            field_name: spec.field_name,
            kind: spec.kind,
            data_type: spec.data_type,
            start: regex_match.start(),
            end: regex_match.end(),
            value: regex_match.as_str(),
            confidence: spec.confidence,
            score: spec.score,
            detector: spec.detector,
            reason: spec.reason,
            priority: spec.priority,
        });
    }
}

fn push_tax_id_spans<'a>(content: &'a str, candidates: &mut Vec<PrivacySpan<'a>>) {
    for regex_match in inline_tax_id_pattern().find_iter(content) {
        if !is_tax_id(regex_match.as_str()) {
            continue;
        }
        candidates.push(PrivacySpan {
            field_name: "taxId",
            kind: PrivacyFindingKind::GovernmentId,
            data_type: DataType::TaxId,
            start: regex_match.start(),
            end: regex_match.end(),
            value: regex_match.as_str(),
            confidence: Confidence::High,
            score: 94,
            detector: "validator:tax-id:us",
            reason: "US SSN or EIN value passed validator.",
            priority: 80,
        });
    }
}

fn push_secret_spans<'a>(content: &'a str, candidates: &mut Vec<PrivacySpan<'a>>) {
    for captures in secret_assignment_pattern().captures_iter(content) {
        if let Some(secret_value) = captures.get(1) {
            candidates.push(PrivacySpan {
                field_name: "secret",
                kind: PrivacyFindingKind::CredentialOrSecret,
                data_type: DataType::String,
                start: secret_value.start(),
                end: secret_value.end(),
                value: secret_value.as_str(),
                confidence: Confidence::High,
                score: 98,
                detector: "pattern:secret-assignment",
                reason: "Credential or secret assignment pattern.",
                priority: 0,
            });
        }
    }

    for captures in bearer_token_pattern().captures_iter(content) {
        if let Some(secret_value) = captures.get(1) {
            candidates.push(PrivacySpan {
                field_name: "secret",
                kind: PrivacyFindingKind::CredentialOrSecret,
                data_type: DataType::String,
                start: secret_value.start(),
                end: secret_value.end(),
                value: secret_value.as_str(),
                confidence: Confidence::High,
                score: 96,
                detector: "pattern:bearer-token",
                reason: "Bearer token pattern.",
                priority: 1,
            });
        }
    }

    for regex_match in private_key_marker_pattern().find_iter(content) {
        candidates.push(PrivacySpan {
            field_name: "secret",
            kind: PrivacyFindingKind::CredentialOrSecret,
            data_type: DataType::String,
            start: regex_match.start(),
            end: regex_match.end(),
            value: regex_match.as_str(),
            confidence: Confidence::High,
            score: 99,
            detector: "pattern:private-key",
            reason: "Private key marker pattern.",
            priority: 2,
        });
    }
}

fn push_account_number_spans<'a>(content: &'a str, candidates: &mut Vec<PrivacySpan<'a>>) {
    for regex_match in payment_card_candidate_pattern().find_iter(content) {
        let digits = regex_match
            .as_str()
            .chars()
            .filter(|character| character.is_ascii_digit())
            .collect::<String>();
        if !is_payment_card_number(&digits) {
            continue;
        }
        candidates.push(PrivacySpan {
            field_name: "accountNumber",
            kind: PrivacyFindingKind::AccountOrFinancialId,
            data_type: DataType::NumericId,
            start: regex_match.start(),
            end: regex_match.end(),
            value: regex_match.as_str(),
            confidence: Confidence::High,
            score: 94,
            detector: "validator:card",
            reason: "Payment-card-shaped number passed brand, length, and Luhn validation.",
            priority: 10,
        });
    }

    for regex_match in iban_candidate_pattern().find_iter(content) {
        if !is_iban(regex_match.as_str()) {
            continue;
        }
        candidates.push(PrivacySpan {
            field_name: "accountNumber",
            kind: PrivacyFindingKind::AccountOrFinancialId,
            data_type: DataType::String,
            start: regex_match.start(),
            end: regex_match.end(),
            value: regex_match.as_str(),
            confidence: Confidence::High,
            score: 92,
            detector: "validator:iban",
            reason: "IBAN account identifier passed checksum validation.",
            priority: 11,
        });
    }

    for regex_match in vat_candidate_pattern().find_iter(content) {
        if !is_vat_id(regex_match.as_str()) {
            continue;
        }
        candidates.push(PrivacySpan {
            field_name: "taxId",
            kind: PrivacyFindingKind::GovernmentId,
            data_type: DataType::TaxId,
            start: regex_match.start(),
            end: regex_match.end(),
            value: regex_match.as_str(),
            confidence: Confidence::High,
            score: 92,
            detector: "validator:vat",
            reason: "VAT identifier passed country-specific validation.",
            priority: 12,
        });
    }
}

fn select_non_overlapping_spans(mut candidates: Vec<PrivacySpan<'_>>) -> Vec<PrivacySpan<'_>> {
    candidates.sort_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then(left.priority.cmp(&right.priority))
            .then((right.end - right.start).cmp(&(left.end - left.start)))
    });

    let mut selected = Vec::new();
    let mut last_end = 0;
    for candidate in candidates {
        if candidate.start < last_end {
            continue;
        }
        last_end = candidate.end;
        selected.push(candidate);
    }
    selected
}

pub(super) fn finding_from_span(
    row_index: usize,
    span: &PrivacySpan<'_>,
    sample_value: &str,
) -> PrivacyFinding {
    PrivacyFinding {
        kind: span.kind,
        data_type: span.data_type,
        row_index,
        start: utf16_index_for_byte(sample_value, span.start),
        end: utf16_index_for_byte(sample_value, span.end),
        match_value: span.value.to_string(),
        sample_value: sample_value.to_string(),
        confidence: span.confidence,
        score: span.score,
        detector: span.detector.to_string(),
        reason: span.reason.to_string(),
    }
}

fn inline_email_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b").unwrap())
}

fn inline_url_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r#"\b(?:https?://|www\.)[^\s<>'"]+"#).unwrap())
}

fn inline_uuid_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
        )
        .unwrap()
    })
}

fn inline_timestamp_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\b\d{4}-\d{2}-\d{2}(?:[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?)?\b").unwrap()
    })
}

fn inline_ip_address_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\b(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)\b")
            .unwrap()
    })
}

fn inline_mac_address_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}\b").unwrap())
}

fn inline_tax_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b(?:\d{3}-\d{2}-\d{4}|\d{2}-\d{7})\b").unwrap())
}

fn inline_phone_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?:\+\d{1,3}[\s.-]?)?(?:\(?\d{3}\)?[\s.-]?)\d{3}[\s.-]?\d{4}\b").unwrap()
    })
}

fn secret_assignment_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?i)\b(?:api[_-]?key|access[_-]?token|auth[_-]?token|secret|password|passwd|pwd|private[_-]?key)\b\s*[:=]\s*["']?([A-Za-z0-9][A-Za-z0-9_\-./+=]{7,})"#,
        )
        .unwrap()
    })
}

fn bearer_token_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)\bbearer\s+([A-Za-z0-9._~+/\-]{12,}=*)").unwrap())
}

fn private_key_marker_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"-----BEGIN [A-Z ]*PRIVATE KEY-----").unwrap())
}

fn payment_card_candidate_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"\b\d(?:[ -]?\d){12,18}\b").unwrap())
}

fn iban_candidate_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"(?i)\b[A-Z]{2}\d{2}(?:\s?[A-Z0-9]){11,30}\b").unwrap())
}

fn vat_candidate_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?i)\b[A-Z]{2,3}[\s./-]?[A-Z0-9](?:[\s./-]?[A-Z0-9]){6,14}\b").unwrap()
    })
}
