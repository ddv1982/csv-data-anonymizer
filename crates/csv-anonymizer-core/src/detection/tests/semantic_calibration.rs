//! Synthetic semantic calibration corpus and release gates.
//!
//! Unlike a collection of one-off regression assertions, this suite reports
//! accuracy by decision axis. A change can therefore improve format detection
//! while accidentally making semantic claims less precise, or preserve semantic
//! accuracy while silently reducing auto-selection recall. Both regressions are
//! visible and gated independently.

use super::*;
use crate::types::{
    ColumnMetadata, PrivacyFindingKind, RedactionPlaceholderSource, SemanticSpecificity,
    SemanticStatus,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalibrationCase {
    id: String,
    group: String,
    header: String,
    values: Vec<String>,
    expected_format: DataType,
    #[serde(default)]
    expected_semantic: Option<PrivacyFindingKind>,
    #[serde(default)]
    forbidden_semantic: Option<PrivacyFindingKind>,
    expected_risk: PiiRisk,
    expected_selected: bool,
    expected_status: SemanticStatus,
    expected_specificity: SemanticSpecificity,
    expected_placeholder: String,
    expected_placeholder_source: RedactionPlaceholderSource,
}

#[derive(Default)]
struct Metrics {
    total: usize,
    format_correct: usize,
    semantic_population: usize,
    semantic_correct: usize,
    forbidden_population: usize,
    forbidden_absent: usize,
    sensitive_population: usize,
    sensitive_selected: usize,
    benign_population: usize,
    benign_unselected: usize,
    failures: Vec<String>,
    groups: BTreeMap<String, (usize, usize)>,
}

fn corpus() -> Vec<CalibrationCase> {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/semantic_calibration_corpus.json"
    )))
    .expect("semantic calibration corpus must be valid JSON")
}

fn metadata(case: &CalibrationCase) -> ColumnMetadata {
    let rows = case
        .values
        .iter()
        .map(|value| vec![value.clone()])
        .collect::<Vec<_>>();
    crate::metadata::build_column_metadata(std::slice::from_ref(&case.header), &rows).remove(0)
}

fn contains_kind(column: &ColumnMetadata, kind: PrivacyFindingKind) -> bool {
    column
        .privacy_evidence
        .iter()
        .any(|evidence| evidence.kind == kind)
}

fn score(cases: &[CalibrationCase]) -> Metrics {
    let mut metrics = Metrics::default();
    for case in cases {
        let column = metadata(case);
        metrics.total += 1;

        let format_ok = column.detected_type == case.expected_format;
        metrics.format_correct += usize::from(format_ok);

        let expected_semantic_name = case
            .expected_semantic
            .map(semantic_kind_name)
            .unwrap_or("unknown");
        let semantic_ok = column.evidence_profile.semantic_decision.kind == expected_semantic_name
            && column.evidence_profile.semantic_decision.status == case.expected_status
            && column.evidence_profile.semantic_decision.specificity == case.expected_specificity;
        if case.expected_semantic.is_some() {
            metrics.semantic_population += 1;
            metrics.semantic_correct += usize::from(semantic_ok);
        }

        let forbidden_ok = case
            .forbidden_semantic
            .is_none_or(|kind| !contains_kind(&column, kind));
        if case.forbidden_semantic.is_some() {
            metrics.forbidden_population += 1;
            metrics.forbidden_absent += usize::from(forbidden_ok);
        }

        let risk_ok = column.pii_risk == case.expected_risk
            && column.evidence_profile.privacy_decision.risk == case.expected_risk;
        let redaction_ok = column.evidence_profile.redaction_decision.placeholder
            == case.expected_placeholder
            && column.evidence_profile.redaction_decision.source
                == case.expected_placeholder_source;
        // Metadata is deliberately constructed unselected; this production
        // predicate is the authoritative decision used to select elevated-risk
        // columns when analysis results are presented.
        let auto_selected = crate::metadata::should_auto_select_column(&column);
        let selected_ok = auto_selected == case.expected_selected;
        if case.expected_selected {
            metrics.sensitive_population += 1;
            metrics.sensitive_selected += usize::from(auto_selected);
        } else {
            metrics.benign_population += 1;
            metrics.benign_unselected += usize::from(!auto_selected);
        }

        let passed =
            format_ok && semantic_ok && forbidden_ok && risk_ok && selected_ok && redaction_ok;
        let group = metrics.groups.entry(case.group.clone()).or_default();
        group.1 += 1;
        group.0 += usize::from(passed);
        if !passed {
            metrics.failures.push(format!(
                "{}: expected format={:?}, semantic={:?}, status={:?}, placeholder={:?}, source={:?}, risk={:?}, selected={}; got format={:?}, semantic={}, status={:?}, placeholder={:?}, source={:?}, risk={:?}, selected={}, evidence={:?}",
                case.id,
                case.expected_format,
                case.expected_semantic,
                case.expected_status,
                case.expected_placeholder,
                case.expected_placeholder_source,
                case.expected_risk,
                case.expected_selected,
                column.detected_type,
                column.evidence_profile.semantic_decision.kind,
                column.evidence_profile.semantic_decision.status,
                column.evidence_profile.redaction_decision.placeholder,
                column.evidence_profile.redaction_decision.source,
                column.pii_risk,
                auto_selected,
                column
                    .privacy_evidence
                    .iter()
                    .map(|evidence| evidence.kind)
                    .collect::<Vec<_>>()
            ));
        }
    }
    metrics
}

fn semantic_kind_name(kind: PrivacyFindingKind) -> &'static str {
    match kind {
        PrivacyFindingKind::Person => "person",
        PrivacyFindingKind::Contact => "contact",
        PrivacyFindingKind::PrivateAddress => "privateAddress",
        PrivacyFindingKind::AddressRegion => "addressRegion",
        PrivacyFindingKind::PrivateDate => "privateDate",
        PrivacyFindingKind::AccountOrFinancialId => "accountOrFinancialId",
        PrivacyFindingKind::RecordIdentifier => "recordIdentifier",
        PrivacyFindingKind::GovernmentId => "governmentId",
        PrivacyFindingKind::CredentialOrSecret => "credentialOrSecret",
        PrivacyFindingKind::NetworkOrDeviceId => "networkOrDeviceId",
        PrivacyFindingKind::Url => "url",
        PrivacyFindingKind::MixedSensitiveText => "mixedSensitiveText",
    }
}

fn percent(part: usize, total: usize) -> f64 {
    if total == 0 {
        100.0
    } else {
        part as f64 * 100.0 / total as f64
    }
}

#[test]
fn semantic_calibration_quality_gates() {
    let metrics = score(&corpus());
    eprintln!(
        "semantic calibration: format={}/{} ({:.1}%), semantic={}/{} ({:.1}%), \
         forbidden-specific-claims-absent={}/{} ({:.1}%), sensitive-selection={}/{} ({:.1}%), \
         benign-unselected={}/{} ({:.1}%)",
        metrics.format_correct,
        metrics.total,
        percent(metrics.format_correct, metrics.total),
        metrics.semantic_correct,
        metrics.semantic_population,
        percent(metrics.semantic_correct, metrics.semantic_population),
        metrics.forbidden_absent,
        metrics.forbidden_population,
        percent(metrics.forbidden_absent, metrics.forbidden_population),
        metrics.sensitive_selected,
        metrics.sensitive_population,
        percent(metrics.sensitive_selected, metrics.sensitive_population),
        metrics.benign_unselected,
        metrics.benign_population,
        percent(metrics.benign_unselected, metrics.benign_population),
    );
    for (group, (passed, total)) in &metrics.groups {
        eprintln!("  {group}: {passed}/{total}");
    }

    assert!(
        metrics.failures.is_empty(),
        "semantic calibration regressions:\n{}",
        metrics.failures.join("\n")
    );
    assert_eq!(
        metrics.sensitive_selected, metrics.sensitive_population,
        "unsafe selection miss: all synthetic sensitive columns must be selected"
    );
    assert_eq!(
        metrics.benign_unselected, metrics.benign_population,
        "false auto-selection: all synthetic benign columns must stay unselected"
    );
    assert_eq!(
        metrics.forbidden_absent, metrics.forbidden_population,
        "an unsupported specific semantic claim was emitted"
    );
}
