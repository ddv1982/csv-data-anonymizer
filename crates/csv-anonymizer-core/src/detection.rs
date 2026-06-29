use crate::types::{
    Confidence, DataType, DetectionResult, DetectionTrace, DetectionTraceItem, EmptyFormat,
    PiiRisk, PrivacyEvidenceSummary, PrivacyFinding, PrivacyFindingKind,
};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

struct HeaderTerms {
    compact: String,
    tokens: HashSet<String>,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnPrivacyAnalysis {
    pub findings: Vec<PrivacyFinding>,
    pub evidence: Vec<PrivacyEvidenceSummary>,
    pub suggested_data_type: Option<DataType>,
    pub pii_risk: PiiRisk,
}

pub fn is_empty_value(value: &str) -> bool {
    value.is_empty() || value.eq_ignore_ascii_case("null")
}

pub fn collect_privacy_spans(content: &str) -> Vec<PrivacySpan<'_>> {
    let mut candidates = Vec::new();
    push_secret_spans(content, &mut candidates);
    push_account_number_spans(content, &mut candidates);
    push_pattern_spans(
        content,
        &mut candidates,
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
    );
    push_pattern_spans(
        content,
        &mut candidates,
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
    );
    push_pattern_spans(
        content,
        &mut candidates,
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
    );
    push_pattern_spans(
        content,
        &mut candidates,
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
    );
    push_pattern_spans(
        content,
        &mut candidates,
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
    );
    push_pattern_spans(
        content,
        &mut candidates,
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
    );
    push_pattern_spans(
        content,
        &mut candidates,
        SpanSpec {
            field_name: "taxId",
            kind: PrivacyFindingKind::GovernmentId,
            data_type: DataType::TaxId,
            regex: inline_tax_id_pattern(),
            confidence: Confidence::High,
            score: 94,
            detector: "pattern:tax-id",
            reason: "US SSN or EIN-shaped identifier pattern.",
            priority: 80,
        },
    );
    push_pattern_spans(
        content,
        &mut candidates,
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
    );

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

pub fn analyze_column_privacy(
    column_name: &str,
    _column_index: usize,
    values: &[String],
    detected_type: DataType,
    detection_confidence: Confidence,
) -> ColumnPrivacyAnalysis {
    let mut findings = Vec::new();
    for (row_index, value) in values.iter().enumerate() {
        if is_empty_value(value) {
            continue;
        }
        for span in collect_privacy_spans(value) {
            findings.push(finding_from_span(row_index, &span, value));
        }
    }

    add_full_cell_findings_from_detection(
        &mut findings,
        column_name,
        values,
        detected_type,
        detection_confidence,
    );
    add_full_cell_findings_from_header(&mut findings, column_name, values);

    let sample_count = values.iter().filter(|value| !is_empty_value(value)).count();
    let evidence = summarize_privacy_findings(&findings, sample_count);
    let suggested_data_type = evidence
        .iter()
        .max_by_key(|summary| summary.score)
        .map(|summary| summary.data_type)
        .filter(|data_type| {
            matches!(
                detected_type,
                DataType::String | DataType::Unknown | DataType::Enum
            ) && *data_type != DataType::String
        });
    let pii_risk = evidence
        .iter()
        .filter(|summary| summary.confidence != Confidence::Low)
        .map(|summary| risk_for_privacy_kind(summary.kind))
        .fold(PiiRisk::Low, max_pii_risk);

    ColumnPrivacyAnalysis {
        findings,
        evidence,
        suggested_data_type,
        pii_risk,
    }
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

struct FullCellFindingSpec {
    kind: PrivacyFindingKind,
    data_type: DataType,
    confidence: Confidence,
    score: u8,
    detector: &'static str,
    reason: &'static str,
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
        if digits.len() < 13 || digits.len() > 19 || !passes_luhn(&digits) {
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
            detector: "validator:luhn",
            reason: "Payment-card-shaped number with valid Luhn checksum.",
            priority: 10,
        });
    }

    for regex_match in iban_candidate_pattern().find_iter(content) {
        candidates.push(PrivacySpan {
            field_name: "accountNumber",
            kind: PrivacyFindingKind::AccountOrFinancialId,
            data_type: DataType::String,
            start: regex_match.start(),
            end: regex_match.end(),
            value: regex_match.as_str(),
            confidence: Confidence::Medium,
            score: 74,
            detector: "pattern:iban",
            reason: "IBAN-shaped account identifier pattern.",
            priority: 11,
        });
    }
}

fn finding_from_span(
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

fn add_full_cell_findings_from_detection(
    findings: &mut Vec<PrivacyFinding>,
    column_name: &str,
    values: &[String],
    detected_type: DataType,
    detection_confidence: Confidence,
) {
    if detected_type == DataType::Timestamp && infer_private_date_from_header(column_name) {
        promote_findings(
            findings,
            PrivacyFindingKind::PrivateDate,
            Confidence::Medium,
            72,
        );
    }

    let Some((kind, reason)) = detected_type_privacy_kind(detected_type) else {
        return;
    };
    for (row_index, value) in values.iter().enumerate() {
        if is_empty_value(value) || has_row_finding(findings, row_index, kind) {
            continue;
        }
        findings.push(full_cell_finding(
            row_index,
            value,
            FullCellFindingSpec {
                kind,
                data_type: detected_type,
                confidence: detection_confidence,
                score: score_for_confidence(detection_confidence),
                detector: "detector:column-type",
                reason,
            },
        ));
    }
}

fn add_full_cell_findings_from_header(
    findings: &mut Vec<PrivacyFinding>,
    column_name: &str,
    values: &[String],
) {
    let header = header_terms(column_name);
    let header_signal = if infer_secret_from_header(&header) {
        Some((
            PrivacyFindingKind::CredentialOrSecret,
            DataType::String,
            Confidence::Medium,
            82,
            "header:secret",
            "Header terms suggest a credential or secret value.",
        ))
    } else if infer_account_from_header(&header) {
        Some((
            PrivacyFindingKind::AccountOrFinancialId,
            DataType::NumericId,
            Confidence::Medium,
            76,
            "header:account",
            "Header terms suggest an account or financial identifier.",
        ))
    } else if infer_private_date_from_header(column_name) {
        Some((
            PrivacyFindingKind::PrivateDate,
            DataType::Timestamp,
            Confidence::Medium,
            70,
            "header:private-date",
            "Header terms suggest a private date.",
        ))
    } else {
        None
    };

    let Some((kind, data_type, confidence, score, detector, reason)) = header_signal else {
        return;
    };

    for (row_index, value) in values.iter().enumerate() {
        if is_empty_value(value)
            || has_row_finding(findings, row_index, kind)
            || !value_matches_header_signal(value, kind)
        {
            continue;
        }
        findings.push(full_cell_finding(
            row_index,
            value,
            FullCellFindingSpec {
                kind,
                data_type,
                confidence,
                score,
                detector,
                reason,
            },
        ));
    }
}

fn detected_type_privacy_kind(data_type: DataType) -> Option<(PrivacyFindingKind, &'static str)> {
    match data_type {
        DataType::Email | DataType::Phone => Some((
            PrivacyFindingKind::Contact,
            "Column type indicates contact information.",
        )),
        DataType::FirstName | DataType::LastName | DataType::FullName => Some((
            PrivacyFindingKind::Person,
            "Column type indicates person names.",
        )),
        DataType::Address => Some((
            PrivacyFindingKind::PrivateAddress,
            "Column type indicates private address data.",
        )),
        DataType::PostalCode => Some((
            PrivacyFindingKind::PrivateAddress,
            "Column type indicates postal address context.",
        )),
        DataType::TaxId => Some((
            PrivacyFindingKind::GovernmentId,
            "Column type indicates government or tax identifier data.",
        )),
        DataType::NumericId => Some((
            PrivacyFindingKind::AccountOrFinancialId,
            "Column type indicates identifier-shaped values; review context.",
        )),
        DataType::Uuid | DataType::IpAddress | DataType::MacAddress => Some((
            PrivacyFindingKind::NetworkOrDeviceId,
            "Column type indicates network, device, or persistent identifiers.",
        )),
        DataType::Url => Some((PrivacyFindingKind::Url, "Column type indicates URLs.")),
        DataType::NumericValue
        | DataType::Timestamp
        | DataType::Boolean
        | DataType::Currency
        | DataType::Percentage
        | DataType::CountryCode
        | DataType::Enum
        | DataType::String
        | DataType::Unknown => None,
    }
}

fn full_cell_finding(row_index: usize, value: &str, spec: FullCellFindingSpec) -> PrivacyFinding {
    PrivacyFinding {
        kind: spec.kind,
        data_type: spec.data_type,
        row_index,
        start: 0,
        end: utf16_len(value),
        match_value: value.to_string(),
        sample_value: value.to_string(),
        confidence: spec.confidence,
        score: spec.score,
        detector: spec.detector.to_string(),
        reason: spec.reason.to_string(),
    }
}

fn has_row_finding(
    findings: &[PrivacyFinding],
    row_index: usize,
    kind: PrivacyFindingKind,
) -> bool {
    findings
        .iter()
        .any(|finding| finding.row_index == row_index && finding.kind == kind)
}

fn promote_findings(
    findings: &mut [PrivacyFinding],
    kind: PrivacyFindingKind,
    confidence: Confidence,
    score: u8,
) {
    for finding in findings.iter_mut().filter(|finding| finding.kind == kind) {
        if confidence_rank(confidence) > confidence_rank(finding.confidence) {
            finding.confidence = confidence;
        }
        finding.score = finding.score.max(score);
    }
}

fn summarize_privacy_findings(
    findings: &[PrivacyFinding],
    sample_count: usize,
) -> Vec<PrivacyEvidenceSummary> {
    let mut summaries: HashMap<(PrivacyFindingKind, DataType), PrivacyEvidenceAccumulator> =
        HashMap::new();
    for finding in findings {
        let entry = summaries
            .entry((finding.kind, finding.data_type))
            .or_insert_with(|| PrivacyEvidenceAccumulator {
                summary: PrivacyEvidenceSummary {
                    kind: finding.kind,
                    data_type: finding.data_type,
                    confidence: finding.confidence,
                    match_count: 0,
                    sample_count,
                    score: finding.score,
                    reason: finding.reason.clone(),
                },
                matched_rows: HashSet::new(),
            });
        entry.matched_rows.insert(finding.row_index);
        entry.summary.match_count = entry.matched_rows.len();
        if finding.score > entry.summary.score {
            entry.summary.score = finding.score;
            entry.summary.reason = finding.reason.clone();
        }
        if confidence_rank(finding.confidence) > confidence_rank(entry.summary.confidence) {
            entry.summary.confidence = finding.confidence;
        }
    }

    let mut ordered = summaries
        .into_values()
        .map(|accumulator| accumulator.summary)
        .collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(right.match_count.cmp(&left.match_count))
            .then(format!("{:?}", left.kind).cmp(&format!("{:?}", right.kind)))
    });
    ordered
}

struct PrivacyEvidenceAccumulator {
    summary: PrivacyEvidenceSummary,
    matched_rows: HashSet<usize>,
}

fn utf16_index_for_byte(value: &str, byte_index: usize) -> usize {
    match value.get(..byte_index) {
        Some(prefix) => utf16_len(prefix),
        None => value
            .char_indices()
            .take_while(|(index, _)| *index < byte_index)
            .map(|(_, character)| character.len_utf16())
            .sum(),
    }
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

fn score_for_confidence(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::High => 88,
        Confidence::Medium => 72,
        Confidence::Low => 54,
    }
}

fn confidence_rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::High => 3,
        Confidence::Medium => 2,
        Confidence::Low => 1,
    }
}

fn risk_for_privacy_kind(kind: PrivacyFindingKind) -> PiiRisk {
    match kind {
        PrivacyFindingKind::Person
        | PrivacyFindingKind::Contact
        | PrivacyFindingKind::PrivateAddress
        | PrivacyFindingKind::AccountOrFinancialId
        | PrivacyFindingKind::GovernmentId
        | PrivacyFindingKind::CredentialOrSecret
        | PrivacyFindingKind::MixedSensitiveText => PiiRisk::High,
        PrivacyFindingKind::PrivateDate
        | PrivacyFindingKind::NetworkOrDeviceId
        | PrivacyFindingKind::Url => PiiRisk::Medium,
    }
}

pub fn max_pii_risk(left: PiiRisk, right: PiiRisk) -> PiiRisk {
    match (left, right) {
        (PiiRisk::High, _) | (_, PiiRisk::High) => PiiRisk::High,
        (PiiRisk::Medium, _) | (_, PiiRisk::Medium) => PiiRisk::Medium,
        (PiiRisk::Low, PiiRisk::Low) => PiiRisk::Low,
    }
}

fn value_matches_header_signal(value: &str, kind: PrivacyFindingKind) -> bool {
    match kind {
        PrivacyFindingKind::CredentialOrSecret => {
            value.len() >= 8
                && value
                    .chars()
                    .any(|character| character.is_ascii_alphabetic())
        }
        PrivacyFindingKind::AccountOrFinancialId => {
            value
                .chars()
                .filter(|character| character.is_ascii_digit())
                .count()
                >= 4
        }
        PrivacyFindingKind::PrivateDate => is_timestamp(value) || value.len() >= 4,
        PrivacyFindingKind::Person
        | PrivacyFindingKind::Contact
        | PrivacyFindingKind::PrivateAddress
        | PrivacyFindingKind::GovernmentId
        | PrivacyFindingKind::NetworkOrDeviceId
        | PrivacyFindingKind::Url
        | PrivacyFindingKind::MixedSensitiveText => true,
    }
}

fn infer_secret_from_header(terms: &HeaderTerms) -> bool {
    matches!(
        terms.compact.as_str(),
        "apikey"
            | "accesstoken"
            | "authtoken"
            | "password"
            | "passwd"
            | "pwd"
            | "secret"
            | "token"
            | "privatekey"
    ) || terms.tokens.contains("secret")
        || terms.tokens.contains("password")
        || terms.tokens.contains("passwd")
        || terms.tokens.contains("pwd")
        || terms.tokens.contains("token")
        || terms.tokens.contains("key") && terms.tokens.contains("api")
}

fn infer_account_from_header(terms: &HeaderTerms) -> bool {
    terms.tokens.contains("account")
        || terms.tokens.contains("acct")
        || terms.tokens.contains("iban")
        || terms.tokens.contains("routing")
        || terms.tokens.contains("card")
        || terms.tokens.contains("pan")
        || terms.tokens.contains("bank") && terms.tokens.contains("number")
}

fn infer_private_date_from_header(column_name: &str) -> bool {
    let terms = header_terms(column_name);
    matches!(
        terms.compact.as_str(),
        "dob" | "dateofbirth" | "birthdate" | "birthday"
    ) || terms.tokens.contains("birth")
        || terms.tokens.contains("dob")
        || terms.tokens.contains("date") && terms.tokens.contains("birth")
}

fn passes_luhn(digits: &str) -> bool {
    let mut sum = 0;
    let mut double = false;
    for character in digits.chars().rev() {
        let Some(mut digit) = character.to_digit(10) else {
            return false;
        };
        if double {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
        double = !double;
    }
    sum > 0 && sum % 10 == 0
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

    if let Some(result) = detect_header_postal_code(column_name, &non_empty_values, values.len()) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Header terms and sample shape matched postal code detection.",
            "header postal code rule",
        );
    }

    if let Some(result) = detect_header_address(column_name, &non_empty_values, values.len()) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Header terms and sample shape matched address detection.",
            "header address rule",
        );
    }

    if let Some(result) = detect_header_tax_id(column_name, &non_empty_values, values.len()) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Header terms and sample shape matched tax ID detection.",
            "header tax ID rule",
        );
    }

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
            return detection_result(
                data_type,
                confidence,
                match_count,
                values.len(),
                total_non_empty,
                "Sample values matched a built-in pattern rule.",
                candidates,
            );
        }
    }

    if let Some(result) = detect_header_numeric_id(column_name, &non_empty_values, values.len()) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Header terms and integer sample shape matched numeric ID detection.",
            "header numeric ID rule",
        );
    }

    if let Some(result) = detect_numeric_value_type(values, total_non_empty) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Sample values matched numeric value detection after identifier rules were rejected.",
            "numeric value rule",
        );
    }

    if let Some(result) = detect_name_type(column_name, &non_empty_values, values.len()) {
        return attach_single_trace(
            result,
            total_non_empty,
            "Header terms and sample shape matched name detection.",
            "header name rule",
        );
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

fn is_email(value: &str) -> bool {
    email_pattern().is_match(value)
}

fn is_uuid(value: &str) -> bool {
    uuid_pattern().is_match(value)
}

fn is_timestamp(value: &str) -> bool {
    timestamp_pattern().is_match(value)
}

fn is_phone(value: &str) -> bool {
    let trimmed = value.trim();
    if !phone_pattern().is_match(trimmed) {
        return false;
    }

    let digit_count = trimmed
        .chars()
        .filter(|character| character.is_ascii_digit())
        .count();
    if !(10..=15).contains(&digit_count) {
        return false;
    }

    trimmed.starts_with('+') || trimmed.chars().any(is_phone_separator)
}

fn is_phone_separator(character: char) -> bool {
    matches!(character, ' ' | '-' | '(' | ')' | '.')
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

fn is_url(value: &str) -> bool {
    url_pattern().is_match(value)
}

fn is_tax_id(value: &str) -> bool {
    tax_id_pattern().is_match(value)
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

fn email_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap())
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

fn phone_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^\+?[\d\s\-().]{10,}$").unwrap())
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

fn url_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(https?://|www\.)[^\s/$.?#].[^\s]*$").unwrap())
}

fn tax_id_pattern() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^(\d{3}-\d{2}-\d{4}|\d{2}-\d{7})$").unwrap())
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
        Regex::new(r"\b(?:\+\d{1,3}[\s.-]?)?(?:\(?\d{3}\)?[\s.-]?)\d{3}[\s.-]?\d{4}\b").unwrap()
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
    PATTERN.get_or_init(|| Regex::new(r"\b[A-Z]{2}\d{2}[A-Z0-9]{11,30}\b").unwrap())
}

fn detect_header_numeric_id(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_numeric_id_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_unsigned_integer(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());

    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::NumericId,
        confidence,
        sample_matches: match_count,
        total_samples,
        trace: None,
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

fn infer_numeric_id_from_header(column_name: &str) -> bool {
    let terms = header_terms(column_name);

    matches!(
        terms.compact.as_str(),
        "id" | "userid"
            | "usernumber"
            | "customerid"
            | "customernumber"
            | "clientid"
            | "clientnumber"
            | "accountid"
            | "accountnumber"
            | "orderid"
            | "ordernumber"
            | "code"
    ) || terms.tokens.contains("id")
        || terms.tokens.contains("identifier")
        || terms.tokens.contains("code")
        || terms.tokens.contains("account") && terms.tokens.contains("number")
        || terms.tokens.contains("customer") && terms.tokens.contains("number")
        || terms.tokens.contains("client") && terms.tokens.contains("number")
        || terms.tokens.contains("order") && terms.tokens.contains("number")
}

fn detect_header_postal_code(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_postal_code_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_postal_code(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::PostalCode,
        confidence,
        sample_matches: match_count,
        total_samples,
        trace: None,
    })
}

fn detect_header_address(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_address_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_plausible_address(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::Address,
        confidence,
        sample_matches: match_count,
        total_samples,
        trace: None,
    })
}

fn detect_header_tax_id(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    if !infer_tax_id_from_header(column_name) {
        return None;
    }

    let match_count = non_empty_values
        .iter()
        .filter(|value| is_tax_id(value) || is_unformatted_tax_id(value))
        .count();
    let confidence = calculate_confidence(match_count, non_empty_values.len());
    if confidence == Confidence::Low {
        return None;
    }

    Some(DetectionResult {
        data_type: DataType::TaxId,
        confidence,
        sample_matches: match_count,
        total_samples,
        trace: None,
    })
}

fn infer_postal_code_from_header(column_name: &str) -> bool {
    let compact = compact_header(column_name);
    let tokens = header_tokens(column_name);

    matches!(
        compact.as_str(),
        "zip" | "zipcode" | "postalcode" | "postcode"
    ) || tokens.contains("zip")
        || tokens.contains("postal") && tokens.contains("code")
        || tokens.contains("post") && tokens.contains("code")
}

fn infer_address_from_header(column_name: &str) -> bool {
    let compact = compact_header(column_name);
    let tokens = header_tokens(column_name);

    matches!(
        compact.as_str(),
        "address" | "streetaddress" | "mailingaddress"
    ) || tokens.contains("address")
        || tokens.contains("street")
}

fn infer_tax_id_from_header(column_name: &str) -> bool {
    let compact = compact_header(column_name);
    let tokens = header_tokens(column_name);

    matches!(compact.as_str(), "ssn" | "taxid" | "taxnumber" | "ein")
        || tokens.contains("ssn")
        || tokens.contains("ein")
        || tokens.contains("tax") && (tokens.contains("id") || tokens.contains("number"))
}

fn compact_header(column_name: &str) -> String {
    column_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn header_tokens(column_name: &str) -> HashSet<String> {
    column_name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
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
    let trimmed = value.trim().to_ascii_lowercase();
    trimmed.chars().any(|character| character.is_ascii_digit())
        && [
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
        ]
        .iter()
        .any(|suffix| trimmed.contains(suffix))
}

fn is_unformatted_tax_id(value: &str) -> bool {
    value.len() == 9 && value.chars().all(|character| character.is_ascii_digit())
}

fn detect_name_type(
    column_name: &str,
    non_empty_values: &[&String],
    total_samples: usize,
) -> Option<DetectionResult> {
    let data_type = infer_name_type_from_header(column_name)?;
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
        if data_type == DataType::FullName && infer_generic_name_header(column_name) {
            match_count = non_empty_values
                .iter()
                .filter(|value| is_plausible_name_part(value, 1))
                .count();
            let confidence = calculate_confidence(match_count, non_empty_values.len());

            if confidence != Confidence::Low {
                return Some(DetectionResult {
                    data_type: DataType::FirstName,
                    confidence,
                    sample_matches: match_count,
                    total_samples,
                    trace: None,
                });
            }
        }

        return None;
    }

    Some(DetectionResult {
        data_type,
        confidence,
        sample_matches: match_count,
        total_samples,
        trace: None,
    })
}

fn infer_name_type_from_header(column_name: &str) -> Option<DataType> {
    let terms = header_terms(column_name);

    if matches!(
        terms.compact.as_str(),
        "firstname" | "givenname" | "forename"
    ) || terms.tokens.contains("firstname")
        || terms.tokens.contains("forename")
        || terms.tokens.contains("given")
        || terms.tokens.contains("first") && terms.tokens.contains("name")
    {
        return Some(DataType::FirstName);
    }

    if matches!(
        terms.compact.as_str(),
        "lastname" | "surname" | "familyname"
    ) || terms.tokens.contains("lastname")
        || terms.tokens.contains("surname")
        || terms.tokens.contains("family") && terms.tokens.contains("name")
        || terms.tokens.contains("last") && terms.tokens.contains("name")
    {
        return Some(DataType::LastName);
    }

    if matches!(
        terms.compact.as_str(),
        "name"
            | "fullname"
            | "displayname"
            | "legalname"
            | "personname"
            | "contactname"
            | "customername"
            | "clientname"
    ) || terms.tokens.contains("fullname")
        || terms.tokens.contains("display") && terms.tokens.contains("name")
        || terms.tokens.contains("legal") && terms.tokens.contains("name")
        || terms.tokens.contains("person") && terms.tokens.contains("name")
        || terms.tokens.contains("contact") && terms.tokens.contains("name")
        || terms.tokens.contains("customer") && terms.tokens.contains("name")
        || terms.tokens.contains("client") && terms.tokens.contains("name")
        || terms.tokens.contains("full") && terms.tokens.contains("name")
    {
        return Some(DataType::FullName);
    }

    None
}

fn header_terms(column_name: &str) -> HeaderTerms {
    HeaderTerms {
        compact: column_name
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect(),
        tokens: column_name
            .split(|character: char| !character.is_ascii_alphanumeric())
            .filter(|token| !token.is_empty())
            .map(|token| token.to_ascii_lowercase())
            .collect(),
    }
}

fn infer_generic_name_header(column_name: &str) -> bool {
    compact_header(column_name) == "name"
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
