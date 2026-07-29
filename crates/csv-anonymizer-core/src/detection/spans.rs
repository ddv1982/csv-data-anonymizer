use crate::types::{Confidence, DataType, PrivacyFinding, PrivacyFindingKind};
use regex::Regex;
use std::sync::OnceLock;

use super::patterns;
use super::utf16_index_for_byte;
use super::validators::{
    is_email, is_formatted_phone_span, is_iban, is_payment_card_number, is_tax_id, is_url,
    is_vat_id,
};
#[cfg(test)]
use super::value;

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
    /// Confirms that the matched text really is an instance of the type.
    ///
    /// Every spec claiming High confidence has one. The column path will not
    /// call a match High without a validator behind it, and a span that reports
    /// High off a bare regex hit claims more certainty than a regex can give —
    /// which then feeds a High privacy risk and an auto-selection. Specs whose
    /// shape is inherently self-validating (UUID, MAC) or deliberately tentative
    /// (a date that may or may not be private) stay `None`, and their confidence
    /// reflects that. `high_confidence_span_specs_are_validator_backed` pins the
    /// rule.
    validator: Option<fn(&str) -> bool>,
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

fn pattern_span_specs() -> [SpanSpec; 8] {
    [
        SpanSpec {
            field_name: "email",
            kind: PrivacyFindingKind::Contact,
            data_type: DataType::Email,
            regex: inline_email_pattern(),
            confidence: Confidence::High,
            score: 96,
            detector: "validator:email",
            reason: "Email address passed validator.",
            priority: 20,
            validator: Some(is_email),
        },
        SpanSpec {
            field_name: "url",
            kind: PrivacyFindingKind::Url,
            data_type: DataType::Url,
            regex: inline_url_pattern(),
            confidence: Confidence::Medium,
            score: 78,
            detector: "validator:url",
            reason: "URL passed validator.",
            priority: 30,
            validator: Some(is_url),
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
            validator: None,
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
            validator: None,
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
            validator: None,
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
            validator: None,
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
            validator: Some(is_formatted_phone_span),
        },
        // The same shape without the formatting: a bare run of ten-plus digits.
        // It is reported rather than dropped, because a phone number typed
        // without separators is still a phone number and free text is still
        // redacted span by span. But it is only Low confidence, because a bare
        // digit run is more often an order or account number, and `pii_risk`
        // ignores Low findings — so this recovers the redaction without letting a
        // regex hit escalate a column to High risk on its own. The formatted spec
        // above has the lower priority number, so it wins wherever both match.
        SpanSpec {
            field_name: "phone",
            kind: PrivacyFindingKind::Contact,
            data_type: DataType::Phone,
            regex: inline_phone_pattern(),
            confidence: Confidence::Low,
            score: 55,
            detector: "pattern:phone-digits",
            reason: "Unformatted digit run in phone-number shape; may be an identifier.",
            priority: 95,
            validator: None,
        },
    ]
}

fn push_pattern_spans<'a>(content: &'a str, candidates: &mut Vec<PrivacySpan<'a>>, spec: SpanSpec) {
    for regex_match in spec.regex.find_iter(content) {
        if let Some(validator) = spec.validator
            && !validator(regex_match.as_str())
        {
            continue;
        }
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
    PATTERN.get_or_init(|| patterns::inside_text(patterns::UUID))
}

fn inline_timestamp_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| patterns::inside_text(patterns::TIMESTAMP))
}

fn inline_ip_address_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| patterns::inside_text(patterns::IPV4))
}

fn inline_mac_address_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| patterns::inside_text(patterns::MAC_ADDRESS))
}

fn inline_tax_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| patterns::inside_text(patterns::US_TAX_ID))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A span reporting High confidence feeds a High privacy risk, which drives
    /// auto-selection. A regex alone cannot justify that, so every High spec has
    /// to name a validator; the column path holds itself to the same bar.
    #[test]
    fn high_confidence_span_specs_are_validator_backed() {
        for spec in pattern_span_specs() {
            if spec.confidence == Confidence::High {
                assert!(
                    spec.validator.is_some(),
                    "span spec {} claims High confidence with no validator behind it",
                    spec.field_name
                );
            }
        }
    }

    #[test]
    fn email_spans_require_a_valid_address() {
        let spans = collect_privacy_spans("write to ada@example.com about it");
        assert!(
            spans
                .iter()
                .any(|span| span.data_type == DataType::Email && span.value == "ada@example.com")
        );

        // Shape-only look-alikes the regex accepts but the validator rejects.
        for content in ["contact user@localhost now", "see foo@bar.c for details"] {
            assert!(
                !spans_contain_type(content, DataType::Email),
                "{content} should not yield an email span"
            );
        }
    }

    /// Formatting decides a phone span's *confidence*, not whether it exists.
    ///
    /// Only High-confidence findings feed `pii_risk`, so reporting the
    /// unformatted case at Low keeps free text redacted without letting a bare
    /// digit run — far more often an order or account number — escalate a column
    /// to High risk off nothing but a regex hit.
    #[test]
    fn phone_span_confidence_follows_formatting() {
        for content in ["call (415) 555-0100 tomorrow", "call +1 415 555 0100 now"] {
            assert_eq!(
                phone_span_confidence(content),
                Some(Confidence::High),
                "a formatted number should be a High-confidence phone span: {content}"
            );
        }

        assert_eq!(
            phone_span_confidence("order reference 4155550100 shipped"),
            Some(Confidence::Low),
            "an unformatted digit run should still be found, but only at Low confidence"
        );
    }

    /// A Low-confidence span is still a span: the free-text transform redacts
    /// every span it is handed, so downgrading the unformatted case must not have
    /// quietly stopped it being reported at all.
    #[test]
    fn unformatted_phone_runs_are_still_reported_as_spans() {
        let spans = collect_privacy_spans("bel 0612345678 voor info");

        assert!(
            spans
                .iter()
                .any(|span| span.data_type == DataType::Phone && span.value == "0612345678"),
            "got {:?}",
            spans.iter().map(|span| span.value).collect::<Vec<_>>()
        );
    }

    /// The column path and the span path must agree on what an IPv4 octet is.
    ///
    /// They used to disagree in both directions: the column path's hand-rolled
    /// parser accepted `001.002.003.004` where the span regex found nothing, and
    /// the regex accepted a `00` octet the parser's `u8` round-trip also let
    /// through. Both now compile the same shared body.
    ///
    /// Only the *shape* has to match. Where a value sits is a separate question the
    /// two paths answer differently on purpose — a column detector asks whether the
    /// whole value is an address, a span scanner asks whether one appears anywhere
    /// inside — so arity cases like `1.2.3.4.5` belong to the anchoring test below,
    /// not here.
    #[test]
    fn ipv4_octets_are_read_identically_by_the_column_and_span_paths() {
        let addresses = ["192.168.1.20", "8.8.8.8", "255.255.255.255", "0.0.0.0"];
        let not_addresses = [
            // Leading zeros: ambiguous between decimal and octal, so not an address.
            "001.002.003.004",
            "010.0.0.1",
            "00.0.0.0",
            "01.2.3.4",
            // Out of range, too few octets, non-numeric.
            "256.1.1.1",
            "1.2.3",
            "1.2.3.a",
        ];

        for value in addresses {
            assert!(
                value::is_ip_address_for_tests(value),
                "the column path should accept {value}"
            );
            assert!(
                spans_contain_type(value, DataType::IpAddress),
                "the span path should find {value}"
            );
        }

        for value in not_addresses {
            assert!(
                !value::is_ip_address_for_tests(value),
                "the column path should reject {value}"
            );
            assert!(
                !spans_contain_type(value, DataType::IpAddress),
                "the span path should not find an address in {value}"
            );
        }
    }

    /// Anchoring is where the two paths are meant to differ.
    ///
    /// `1.2.3.4.5` is not an address, and the column path says so. The span scanner
    /// still reports the `1.2.3.4` inside it, because its job is to find addresses
    /// embedded in longer text and it cannot tell a trailing `.5` from a following
    /// sentence. Pinned so that the shared pattern body is never "fixed" into
    /// whole-value matching on the span side.
    #[test]
    fn span_scanning_finds_addresses_inside_longer_text() {
        assert!(!value::is_ip_address_for_tests("1.2.3.4.5"));
        assert!(spans_contain_type("1.2.3.4.5", DataType::IpAddress));
        assert!(spans_contain_type(
            "connected from 192.168.1.20 at noon",
            DataType::IpAddress
        ));
    }

    fn phone_span_confidence(content: &str) -> Option<Confidence> {
        collect_privacy_spans(content)
            .iter()
            .find(|span| span.data_type == DataType::Phone)
            .map(|span| span.confidence)
    }

    fn spans_contain_type(content: &str, data_type: DataType) -> bool {
        collect_privacy_spans(content)
            .iter()
            .any(|span| span.data_type == data_type)
    }
}
