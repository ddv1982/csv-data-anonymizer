use crate::types::{
    Confidence, DataType, PiiRisk, PrivacyEvidenceSummary, PrivacyFinding, PrivacyFindingKind,
};
use std::collections::{HashMap, HashSet};

use super::header;
use super::spans::finding_from_span;
use super::validators::{is_dutch_btw_tax_number, is_tax_id, is_vat_id};
use super::{
    TaxIdHeaderContext, collect_privacy_spans, has_dutch_btw_context,
    is_contextual_unformatted_us_tax_id, is_empty_value, is_timestamp, tax_id_header_context,
    utf16_len,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnPrivacyAnalysis {
    pub findings: Vec<PrivacyFinding>,
    pub evidence: Vec<PrivacyEvidenceSummary>,
    pub suggested_data_type: Option<DataType>,
    pub pii_risk: PiiRisk,
}

struct FullCellFindingSpec {
    kind: PrivacyFindingKind,
    data_type: DataType,
    confidence: Confidence,
    score: u8,
    detector: String,
    reason: String,
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

fn add_full_cell_findings_from_detection(
    findings: &mut Vec<PrivacyFinding>,
    column_name: &str,
    values: &[String],
    detected_type: DataType,
    detection_confidence: Confidence,
) {
    if detected_type == DataType::Timestamp
        && (header::infer_private_date(column_name) || header::infer_user_event_date(column_name))
    {
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
    let fallback_detector = detector_for_detected_type(detected_type);
    let header_terms = header::terms(column_name);
    let allow_dutch_btw_number = has_dutch_btw_context(&header_terms);
    let tax_id_context = tax_id_header_context(&header_terms);
    for (row_index, value) in values.iter().enumerate() {
        if is_empty_value(value) || has_row_finding(findings, row_index, kind) {
            continue;
        }
        let (detector, reason) = if detected_type == DataType::TaxId {
            tax_id_detector_for_value(value, allow_dutch_btw_number, tax_id_context)
                .unwrap_or((fallback_detector, reason))
        } else {
            (fallback_detector, reason)
        };
        findings.push(full_cell_finding(
            row_index,
            value,
            FullCellFindingSpec {
                kind,
                data_type: detected_type,
                confidence: detection_confidence,
                score: score_for_confidence(detection_confidence),
                detector: detector.to_string(),
                reason: reason.to_string(),
            },
        ));
    }
}

fn add_full_cell_findings_from_header(
    findings: &mut Vec<PrivacyFinding>,
    column_name: &str,
    values: &[String],
) {
    let header = header::terms(column_name);
    let header_signal = if let Some(signal) = header::best_signal_for_kinds(&header, &["secret"]) {
        Some((
            PrivacyFindingKind::CredentialOrSecret,
            DataType::String,
            Confidence::Medium,
            82,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header::best_signal_for_kinds(&header, &["account_number"]) {
        Some((
            PrivacyFindingKind::AccountOrFinancialId,
            DataType::NumericId,
            Confidence::Medium,
            76,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header::best_signal_for_kinds(&header, &["private_date"]) {
        Some((
            PrivacyFindingKind::PrivateDate,
            DataType::Timestamp,
            Confidence::Medium,
            70,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header::best_signal_for_kinds(&header, &["user_event_date"]) {
        Some((
            PrivacyFindingKind::PrivateDate,
            DataType::Timestamp,
            Confidence::Medium,
            68,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header::best_signal_for_kinds(&header, &["account_identifier"]) {
        Some((
            PrivacyFindingKind::AccountOrFinancialId,
            DataType::String,
            Confidence::Medium,
            76,
            signal.detector,
            signal.reason,
        ))
    } else {
        None
    };

    let Some((kind, data_type, confidence, score, detector, reason)) = header_signal else {
        return;
    };

    for (row_index, value) in values.iter().enumerate() {
        if is_empty_value(value) || !value_matches_header_signal(value, kind) {
            continue;
        }

        if has_row_finding(findings, row_index, kind) {
            promote_row_findings(
                findings, row_index, kind, confidence, score, &detector, &reason,
            );
            if !has_row_detector_finding(findings, row_index, kind, &detector) {
                findings.push(full_cell_finding(
                    row_index,
                    value,
                    FullCellFindingSpec {
                        kind,
                        data_type,
                        confidence,
                        score,
                        detector: detector.clone(),
                        reason: reason.clone(),
                    },
                ));
            }
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
                detector: detector.clone(),
                reason: reason.clone(),
            },
        ));
    }
}

fn detected_type_privacy_kind(data_type: DataType) -> Option<(PrivacyFindingKind, &'static str)> {
    data_type.privacy_finding_kind_and_reason()
}

fn detector_for_detected_type(data_type: DataType) -> &'static str {
    match data_type {
        DataType::Phone => "validator:phone",
        DataType::TaxId => "validator:tax-id",
        _ => "detector:column-type",
    }
}

fn tax_id_detector_for_value(
    value: &str,
    allow_dutch_btw_number: bool,
    context: TaxIdHeaderContext,
) -> Option<(&'static str, &'static str)> {
    if is_vat_id(value) {
        return Some((
            "validator:vat",
            "VAT identifier passed country-specific validation.",
        ));
    }
    if allow_dutch_btw_number && is_dutch_btw_tax_number(value) {
        return Some((
            "pattern:tax-id:nl-btw-tax-number",
            "Dutch BTW/omzetbelastingnummer shape matched under Dutch BTW header context.",
        ));
    }
    if is_tax_id(value) || is_contextual_unformatted_us_tax_id(value, context) {
        return Some((
            "validator:tax-id:us",
            "US SSN or EIN value passed validator.",
        ));
    }
    None
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
        detector: spec.detector,
        reason: spec.reason,
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

fn has_row_detector_finding(
    findings: &[PrivacyFinding],
    row_index: usize,
    kind: PrivacyFindingKind,
    detector: &str,
) -> bool {
    findings.iter().any(|finding| {
        finding.row_index == row_index && finding.kind == kind && finding.detector == detector
    })
}

fn promote_row_findings(
    findings: &mut [PrivacyFinding],
    row_index: usize,
    kind: PrivacyFindingKind,
    confidence: Confidence,
    score: u8,
    detector: &str,
    reason: &str,
) {
    for finding in findings
        .iter_mut()
        .filter(|finding| finding.row_index == row_index && finding.kind == kind)
    {
        if confidence_rank(confidence) > confidence_rank(finding.confidence) {
            finding.confidence = confidence;
        }
        if score > finding.score {
            finding.score = score;
            finding.detector = detector.to_string();
            finding.reason = reason.to_string();
        }
    }
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
                    detector: finding.detector.clone(),
                    reason: finding.reason.clone(),
                    detectors: Vec::new(),
                },
                matched_rows: HashSet::new(),
                detectors: HashSet::new(),
            });
        entry.matched_rows.insert(finding.row_index);
        entry.detectors.insert(finding.detector.clone());
        entry.summary.match_count = entry.matched_rows.len();
        if finding.score > entry.summary.score {
            entry.summary.score = finding.score;
            entry.summary.detector = finding.detector.clone();
            entry.summary.reason = finding.reason.clone();
        }
        if confidence_rank(finding.confidence) > confidence_rank(entry.summary.confidence) {
            entry.summary.confidence = finding.confidence;
        }
    }

    let mut ordered = summaries
        .into_values()
        .map(|accumulator| {
            let mut summary = accumulator.summary;
            summary.detectors = accumulator.detectors.into_iter().collect();
            summary.detectors.sort();
            summary
        })
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
    detectors: HashSet<String>,
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
            let digit_count = value
                .chars()
                .filter(|character| character.is_ascii_digit())
                .count();
            digit_count >= 4 || is_account_identifier_value(value)
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

fn is_account_identifier_value(value: &str) -> bool {
    let trimmed = value.trim();
    (3..=64).contains(&trimmed.len())
        && trimmed
            .chars()
            .any(|character| character.is_ascii_alphabetic())
        && trimmed.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}
