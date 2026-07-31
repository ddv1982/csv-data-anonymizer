use crate::types::{
    Confidence, DataType, PiiRisk, PrivacyEvidenceSummary, PrivacyFinding, PrivacyFindingKind,
};
use std::collections::{HashMap, HashSet};

use super::header;
use super::header_rules::is_plausible_full_name;
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

    // One taxonomy scan for the whole column: the branches below consult it
    // several times over, and rescanning per branch was a measurable cost.
    let header = header::analyze(column_name);
    add_full_cell_findings_from_detection(
        &mut findings,
        &header,
        values,
        detected_type,
        detection_confidence,
    );
    add_full_cell_findings_from_header(&mut findings, &header, values);

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
        .filter(|summary| summary.is_actionable())
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
    header: &header::HeaderAnalysis,
    values: &[String],
    detected_type: DataType,
    detection_confidence: Confidence,
) {
    if detected_type == DataType::Timestamp
        && (header.matches_kind("private_date") || header.matches_kind("user_event_date"))
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
    let header_terms = header.terms();
    let allow_dutch_btw_number = has_dutch_btw_context(header_terms);
    let tax_id_context = tax_id_header_context(header_terms);
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
    header: &header::HeaderAnalysis,
    values: &[String],
) {
    let header_signal = if let Some(signal) = header.best_for_kinds(&["secret"]) {
        Some((
            PrivacyFindingKind::CredentialOrSecret,
            DataType::String,
            Confidence::Medium,
            82,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header.best_for_kinds(&["account_number"]) {
        Some((
            PrivacyFindingKind::AccountOrFinancialId,
            DataType::NumericId,
            Confidence::Medium,
            76,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header.best_for_kinds(&["private_date"]) {
        Some((
            PrivacyFindingKind::PrivateDate,
            DataType::Timestamp,
            Confidence::Medium,
            70,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header.best_for_kinds(&["user_event_date"]) {
        Some((
            PrivacyFindingKind::PrivateDate,
            DataType::Timestamp,
            Confidence::Medium,
            68,
            signal.detector,
            signal.reason,
        ))
    } else if let Some(signal) = header.best_for_kinds(&["account_identifier"]) {
        Some((
            PrivacyFindingKind::AccountOrFinancialId,
            DataType::String,
            Confidence::Medium,
            76,
            signal.detector,
            signal.reason,
        ))
    } else if header.best_for_kinds(&["possible_name"]).is_some()
        && column_values_look_like_person_names(values)
    {
        Some((
            PrivacyFindingKind::Person,
            // `String`, not `FullName`, and both halves of that are load-bearing.
            // `suggested_data_type` above only proposes a retype for a data type other
            // than `String`, so naming `FullName` here would offer to reclassify the
            // column — and `classify_pii_risk(FullName)` is High, which auto-selects it
            // and defaults it to Redact. That is the behaviour this branch deliberately
            // does not have.
            DataType::String,
            // Low, so `is_actionable()` is false. The risk fold above skips
            // non-actionable evidence, which is what keeps the column out of the
            // auto-selected set, and `placeholder_from_evidence` will not name a
            // redaction placeholder from it either. Both are correct: what this branch
            // knows is that the values are confidently names *of something*, and it
            // cannot tell a person from a city. Claiming High would assert evidence it
            // does not have; staying silent would hide a column of names. So it records
            // the finding and lets `possible_person_name_warning_for_column` surface it
            // for review.
            Confidence::Low,
            54,
            POSSIBLE_PERSON_NAME_DETECTOR.to_string(),
            "Header ends in a name term and the sampled values are shaped like names.".to_string(),
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

/// Names the detector behind a possible-person-name finding.
///
/// A constant rather than the taxonomy signal's own detector string, so that
/// `service::controls` can recognise this finding to raise its review warning without
/// matching on prose. The coupling is then a compile error if this moves, instead of a
/// warning that silently stops firing.
pub(crate) const POSSIBLE_PERSON_NAME_DETECTOR: &str = "possible person name";

/// Share of a column's non-empty values that must be shaped like a full name before a
/// `name`-suffixed header is treated as possibly holding people.
///
/// Three quarters, matching the spirit of the ratio in `calculate_confidence`: a real
/// name column carries the odd `N/A`, initial-only entry or organisation acting as a
/// party, and one such value must not disqualify the column. Conversely a column of
/// order references with two capitalised words in a couple of rows cannot reach it.
const MIN_NAME_SHAPED_SHARE_NUMERATOR: usize = 3;
const MIN_NAME_SHAPED_SHARE_DENOMINATOR: usize = 4;

/// Minimum non-empty values before the ratio above is allowed to decide anything.
///
/// Below this a single value dominates the share — one name-shaped value out of one is
/// 100% — and `<word> name` headers are common enough that a two-row preview would
/// warn about columns nobody has evidence against.
const MIN_NAME_SHAPED_VALUES: usize = 4;

/// Whether enough of `values` are shaped like full names to corroborate the header.
fn column_values_look_like_person_names(values: &[String]) -> bool {
    let considered = values
        .iter()
        .filter(|value| !is_empty_value(value))
        .collect::<Vec<_>>();
    if considered.len() < MIN_NAME_SHAPED_VALUES {
        return false;
    }
    let name_shaped = considered
        .iter()
        .filter(|value| is_plausible_full_name(value))
        .count();
    name_shaped * MIN_NAME_SHAPED_SHARE_DENOMINATOR
        >= considered.len() * MIN_NAME_SHAPED_SHARE_NUMERATOR
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

pub(crate) fn summarize_privacy_findings(
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
            // Ascending by risk, which `PiiRisk` declares most-severe-first. Score is derived
            // from confidence alone, so two High-confidence findings of different kinds tie at
            // 88 and reach this line: without it a Medium finding can be shown above a High
            // one, which under-sells what was found.
            .then(risk_for_privacy_kind(left.kind).cmp(&risk_for_privacy_kind(right.kind)))
            // Then ascending by kind, so a remaining tie falls to the declaration order of
            // `PrivacyFindingKind` — a stated order rather than whatever the names spell.
            .then(left.kind.cmp(&right.kind))
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

/// Lets the evidence-ordering test drive the summariser with hand-built findings.
#[cfg(test)]
pub(crate) fn summarize_privacy_findings_in_tests(
    findings: &[PrivacyFinding],
    sample_count: usize,
) -> Vec<PrivacyEvidenceSummary> {
    summarize_privacy_findings(findings, sample_count)
}

/// Lets the risk-consistency test compare this mapping against `classify_pii_risk`.
#[cfg(test)]
pub(crate) fn risk_for_privacy_kind_in_tests(kind: PrivacyFindingKind) -> PiiRisk {
    risk_for_privacy_kind(kind)
}

pub(crate) fn risk_for_privacy_kind(kind: PrivacyFindingKind) -> PiiRisk {
    match kind {
        PrivacyFindingKind::Person
        | PrivacyFindingKind::Contact
        | PrivacyFindingKind::PrivateAddress
        | PrivacyFindingKind::AccountOrFinancialId
        | PrivacyFindingKind::GovernmentId
        | PrivacyFindingKind::CredentialOrSecret
        | PrivacyFindingKind::MixedSensitiveText => PiiRisk::High,
        // Medium, not Low: these do not expose a person directly, but each one
        // narrows who a row could belong to, and Medium is still auto-selected and
        // still redacted by default — see `should_auto_select_column`.
        PrivacyFindingKind::PrivateDate
        | PrivacyFindingKind::AddressRegion
        | PrivacyFindingKind::RecordIdentifier
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
        | PrivacyFindingKind::AddressRegion
        | PrivacyFindingKind::RecordIdentifier
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
