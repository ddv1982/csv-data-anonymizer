use crate::report_notes::push_unselected_column_note;
use crate::service::redaction_changes_structured_scalar_type;
use crate::strategies::STRUCTURED_SCALAR_REDACTION_WARNING;
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, ColumnReleaseReport, DataType, FrequencyInversionRisk,
    ReleaseEvidenceItem, ReleaseEvidenceStatus, ReleaseReadiness, ReleaseReadinessStatus,
    TransformReport, UtilityMetric,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ReportContext<'a> {
    pub transform_report: Option<&'a TransformReport>,
}

pub(crate) fn build_readiness(
    columns: &[ColumnMetadata],
    context: &ReportContext<'_>,
) -> ReleaseReadiness {
    let mut review_items = Vec::new();
    let mut verified_items = Vec::new();

    verified_items.push(
        "Replacements are randomized per run; repeated source values stay consistent within the current output."
            .to_string(),
    );

    let unselected_risky = unselected_detector_risk_columns(columns);
    if unselected_risky.is_empty() {
        verified_items
            .push("No high/medium detector-risk columns were left unselected.".to_string());
    } else {
        review_items.push(format!(
            "{} high/medium detector-risk column(s) are outside this release: {}.",
            unselected_risky.len(),
            unselected_risky.join(", ")
        ));
    }

    review_items.push(
        "CSV transforms reduce exposure but are not a formal anonymity guarantee.".to_string(),
    );
    if let Some(report) = context.transform_report
        && report.smart_replacement_rejections > 0
    {
        review_items.push(format!(
            "{} Local AI replacement candidate(s) were rejected before fallback handling: {}.",
            report.smart_replacement_rejections,
            smart_rejection_summary(report)
        ));
    }
    if let Some(report) = context.transform_report
        && report.shape_fallback_values > 0
    {
        review_items.push(format!(
            "{} value(s) did not match their column's detected format and were replaced with generic pseudonyms instead of format-preserving ones.",
            report.shape_fallback_values
        ));
    }
    // A review item rather than a blocker. Whether few distinct values matter depends
    // on what the column holds — a six-valued column may carry nothing sensitive — so
    // a measured heuristic should inform the reviewer, not refuse the release. Note
    // this cannot change the readiness status on its own: the "not a formal anonymity
    // guarantee" item below is unconditional, so the status is already Review.
    if let Some(report) = context.transform_report {
        let invertible = report
            .column_value_distributions
            .iter()
            .filter(|distribution| distribution.risks_frequency_inversion())
            .count();
        if invertible > 0 {
            // "Repeated few enough values" described only the distinct-count test. A
            // column flagged for one dominant value has not repeated few values — it
            // repeated *one* value often — so the summary states the property all three
            // tests establish and leaves the specific evidence to the per-column note.
            review_items.push(format!(
                "The value distribution of {invertible} pseudonymized column(s) is uneven enough that the mapping could be matched back by value frequency."
            ));
        }
    }

    // Blocked status comes only from the preflight path in service.rs; the
    // report readiness built here can only be Verified or Review.
    let status = if review_items.is_empty() {
        ReleaseReadinessStatus::Verified
    } else {
        ReleaseReadinessStatus::Review
    };

    ReleaseReadiness {
        status,
        blockers: Vec::new(),
        review_items,
        verified_items,
    }
}

pub(crate) fn build_evidence(
    columns: &[ColumnMetadata],
    context: &ReportContext<'_>,
) -> Vec<ReleaseEvidenceItem> {
    let mut evidence = Vec::new();
    let selected_count = columns.iter().filter(|column| column.is_selected).count();
    evidence.push(ReleaseEvidenceItem {
        id: "coverage".to_string(),
        label: "Column coverage".to_string(),
        status: if selected_count == columns.len() {
            ReleaseEvidenceStatus::Verified
        } else {
            ReleaseEvidenceStatus::Review
        },
        detail: format!(
            "{selected_count}/{} source column(s) selected for this workflow.",
            columns.len()
        ),
    });

    let unselected_risky = unselected_detector_risk_columns(columns);
    evidence.push(ReleaseEvidenceItem {
        id: "detector-risk".to_string(),
        label: "Detector risk review".to_string(),
        status: if unselected_risky.is_empty() {
            ReleaseEvidenceStatus::Verified
        } else {
            ReleaseEvidenceStatus::Review
        },
        detail: if unselected_risky.is_empty() {
            "No high/medium detector-risk column was left unchanged by selection.".to_string()
        } else {
            format!(
                "Review unselected high/medium detector-risk column(s): {}.",
                unselected_risky.join(", ")
            )
        },
    });

    if let Some(report) = context.transform_report
        && report.smart_replacement_requests > 0
    {
        evidence.push(ReleaseEvidenceItem {
            id: "local-ai-validation".to_string(),
            label: "Local AI validation".to_string(),
            status: if report.smart_replacement_rejections == 0
                && report.smart_replacement_fallbacks == 0
            {
                ReleaseEvidenceStatus::Verified
            } else {
                ReleaseEvidenceStatus::Review
            },
            detail: format!(
                "{} requested, {} accepted, {} rejected, {} fallback value(s).{}",
                report.smart_replacement_requests,
                report.smart_replacement_values,
                report.smart_replacement_rejections,
                report.smart_replacement_fallbacks,
                if report.smart_replacement_rejections > 0 {
                    format!(" Rejection reasons: {}.", smart_rejection_summary(report))
                } else {
                    String::new()
                }
            ),
        });
    }

    evidence
}

pub(crate) fn build_column_reports(columns: &[ColumnMetadata]) -> Vec<ColumnReleaseReport> {
    columns
        .iter()
        .map(|column| {
            let (action, status, detail) = column_action(column);
            ColumnReleaseReport {
                column_index: column.index,
                column_name: column.name.clone(),
                selected: column.is_selected,
                detected_type: column.detected_type,
                pii_risk: column.pii_risk,
                strategy: column.strategy,
                action,
                status,
                detail,
            }
        })
        .collect()
}

pub(crate) fn build_utility_metrics(
    columns: &[ColumnMetadata],
    context: &ReportContext<'_>,
) -> Vec<UtilityMetric> {
    let mut metrics = Vec::new();
    let selected = columns.iter().filter(|column| column.is_selected).count();
    metrics.push(UtilityMetric {
        label: "Selected columns".to_string(),
        value: format!("{selected}/{}", columns.len()),
        status: if selected == columns.len() {
            ReleaseEvidenceStatus::Verified
        } else {
            ReleaseEvidenceStatus::Review
        },
        detail: Some("Columns outside selection are written unchanged.".to_string()),
    });

    if let Some(report) = context.transform_report {
        metrics.push(UtilityMetric {
            label: "Repeat reuse".to_string(),
            value: report.reused_pseudonym_values.to_string(),
            status: ReleaseEvidenceStatus::Info,
            detail: Some(
                "Repeated source values reused the same pseudonym/token within the run."
                    .to_string(),
            ),
        });
        metrics.push(UtilityMetric {
            label: "Local AI accepted".to_string(),
            value: format!(
                "{}/{}",
                report.smart_replacement_values, report.smart_replacement_requests
            ),
            status: if report.smart_replacement_rejections == 0 {
                ReleaseEvidenceStatus::Verified
            } else {
                ReleaseEvidenceStatus::Review
            },
            detail: Some(if report.smart_replacement_rejections > 0 {
                format!(
                    "Accepted structured Local AI replacements before rule-based fallback. Rejections: {}.",
                    smart_rejection_summary(report)
                )
            } else {
                "Accepted structured Local AI replacements before rule-based fallback.".to_string()
            }),
        });
    }

    metrics
}

pub(crate) fn standard_notes(
    columns: &[ColumnMetadata],
    transform_report: TransformReport,
) -> Vec<String> {
    let mut notes = vec![
        "Standard CSV transform changes selected cells in place with local strategies such as masking, redaction, tokenization, pseudonymization, pass-through, and optional Local AI replacement."
            .to_string(),
        "Treat this as risk reduction, not proof of anonymity; review the output against your sharing context and re-identification risk."
            .to_string(),
    ];
    push_unselected_column_note(&mut notes, columns);

    if transform_report.unique_pseudonym_values > 0
        || transform_report.opaque_token_values > 0
        || columns.iter().any(|column| {
            column.is_selected
                && matches!(
                    column.strategy,
                    AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize
                )
                && !column.detected_type.uses_default_pass_through()
        })
    {
        notes.push(
            "Pseudonyms and tokens are tracked within each run so repeated source values stay consistent while distinct readable names avoid reuse while capacity remains."
                .to_string(),
        );
        // The sentence above describes consistency as the feature it is. It is also a
        // re-identification property, and saying only the first half is what the EDPB
        // anonymisation guidelines warn against: output that keeps records linkable is
        // pseudonymised, and pseudonymised data is still personal data. Naming that
        // costs nothing and stops the report implying more than it delivers.
        notes.push(
            "Because repeated values keep the same replacement, these columns are pseudonymized rather than anonymized: records stay linkable to each other, and the output remains personal data under GDPR. Redaction and masking do not preserve that link."
                .to_string(),
        );
    }

    let invertible: Vec<String> = transform_report
        .column_value_distributions
        .iter()
        .filter_map(|distribution| {
            let risk = distribution.frequency_inversion_risk()?;
            let name = columns
                .iter()
                .find(|column| column.index == distribution.column_index)
                .map(|column| column.name.as_str())
                .unwrap_or("(unnamed)");
            // These figures are exact rather than sampled — the ledger counted every
            // row — so unlike the pre-run warning this names no sample size.
            Some(match risk {
                // A distinct count would actively mislead here: a column of 101 values
                // where one covers half the rows renders as "101 distinct of 200
                // values", which is true, reads as reassuring, and describes a risk the
                // column does not have instead of the one it does.
                FrequencyInversionRisk::DominantValue { share } => format!(
                    "{name} (one value in {:.0}% of {} values)",
                    share * 100.0,
                    distribution.total_values
                ),
                FrequencyInversionRisk::FewDistinctValues
                | FrequencyInversionRisk::LargeGroups { .. } => format!(
                    "{name} ({} distinct of {} values)",
                    distribution.distinct_values, distribution.total_values
                ),
            })
        })
        .collect();
    if !invertible.is_empty() {
        notes.push(format!(
            "The replacement mapping for {} column(s) could be matched back by how often each value occurs: {}.",
            invertible.len(),
            invertible.join(", ")
        ));
    }
    if transform_report.collisions_avoided > 0 {
        notes.push(format!(
            "{} pseudonym candidate collision(s) were avoided by assigning unused alternatives.",
            transform_report.collisions_avoided
        ));
    }
    if transform_report.exhausted_pseudonym_pools > 0 {
        notes.push(format!(
            "{} pseudonym pool exhaustion event(s) used generated fallback values.",
            transform_report.exhausted_pseudonym_pools
        ));
    }
    if columns
        .iter()
        .any(|column| column.is_selected && column.strategy == AnonymizationStrategy::LocalAi)
    {
        notes.push(
            "Smart replacement used Local AI on this device to generate realistic replacement values; review outputs because this is not a formal anonymization guarantee."
                .to_string(),
        );
    }
    if columns
        .iter()
        .any(|column| column.is_selected && redaction_changes_structured_scalar_type(column))
    {
        notes.push(format!(
            "{STRUCTURED_SCALAR_REDACTION_WARNING} Use schema-preserving pseudonymization when downstream consumers require original scalar types."
        ));
    }
    if transform_report.smart_replacement_rejections > 0 {
        notes.push(format!(
            "{} smart replacement candidate(s) were rejected before fallback handling: {}.",
            transform_report.smart_replacement_rejections,
            smart_rejection_summary(&transform_report)
        ));
    }
    if transform_report.smart_replacement_fallbacks > 0 {
        notes.push(format!(
            "{} smart replacement value(s) fell back to rule-based pseudonymization after missing or invalid AI output.",
            transform_report.smart_replacement_fallbacks
        ));
    }
    if transform_report.shape_fallback_values > 0 {
        notes.push(format!(
            "{} value(s) did not match their column's detected format and were replaced with generic pseudonyms.",
            transform_report.shape_fallback_values
        ));
    }
    if columns.iter().any(|column| {
        column.is_selected
            && column.detected_type == DataType::Email
            && matches!(
                column.strategy,
                AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize
            )
    }) {
        notes.push(
            "Email pseudonymization keeps the original domain; use Redact or Tokenize when domains themselves are identifying (for example personal domains)."
                .to_string(),
        );
    }

    notes
}

fn column_action(column: &ColumnMetadata) -> (String, ReleaseEvidenceStatus, String) {
    if !column.is_selected {
        return (
            "Unselected".to_string(),
            if matches!(
                column.pii_risk,
                crate::types::PiiRisk::High | crate::types::PiiRisk::Medium
            ) {
                ReleaseEvidenceStatus::Review
            } else {
                ReleaseEvidenceStatus::Info
            },
            "Column was outside the selected release set.".to_string(),
        );
    }

    match column.strategy {
        AnonymizationStrategy::Mask => (
            "Masked".to_string(),
            ReleaseEvidenceStatus::Verified,
            "Selected values are replaced with mask characters.".to_string(),
        ),
        AnonymizationStrategy::Redact => (
            "Redacted".to_string(),
            ReleaseEvidenceStatus::Verified,
            "Selected values are replaced with typed placeholders.".to_string(),
        ),
        AnonymizationStrategy::Tokenize => (
            "Tokenized".to_string(),
            ReleaseEvidenceStatus::Verified,
            "Selected values become opaque tokens that stay consistent within the run.".to_string(),
        ),
        AnonymizationStrategy::Label => (
            "Labelled".to_string(),
            // Review, not Verified: the output is readable and re-linkable by design,
            // so it is pseudonymised rather than anonymous. A reader has to decide
            // whether that is acceptable for the release, which is the definition of
            // a review item.
            ReleaseEvidenceStatus::Review,
            "Selected values become column-named placeholders that stay consistent within the run, which keeps repeated values linkable.".to_string(),
        ),
        AnonymizationStrategy::LocalAi => (
            "Smart replacement".to_string(),
            ReleaseEvidenceStatus::Review,
            "Local AI generated realistic replacements with rule-based fallback for rejected values.".to_string(),
        ),
        AnonymizationStrategy::PassThrough => (
            "Pass-through".to_string(),
            ReleaseEvidenceStatus::Review,
            "Selected values are intentionally kept unchanged.".to_string(),
        ),
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            if column.detected_type.uses_default_pass_through() {
                (
                    "No-op/pass-through".to_string(),
                    ReleaseEvidenceStatus::Review,
                    "This detected type currently keeps values unchanged under Auto/Pseudonymize.".to_string(),
                )
            } else {
                (
                    "Pseudonymized".to_string(),
                    ReleaseEvidenceStatus::Verified,
                    "Selected values use rule-based replacement.".to_string(),
                )
            }
        }
    }
}

fn smart_rejection_summary(report: &TransformReport) -> String {
    if report.smart_replacement_rejection_reasons.is_empty() {
        return "reason details unavailable".to_string();
    }

    report
        .smart_replacement_rejection_reasons
        .iter()
        .map(|entry| {
            format!(
                "{} {}",
                entry.count,
                smart_rejection_reason_label(entry.reason)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn smart_rejection_reason_label(
    reason: crate::types::SmartReplacementRejectionReason,
) -> &'static str {
    match reason {
        crate::types::SmartReplacementRejectionReason::UnexpectedOriginal => "unexpected source",
        crate::types::SmartReplacementRejectionReason::MissingOutput => "missing output",
        crate::types::SmartReplacementRejectionReason::EmptyOutput => "empty output",
        crate::types::SmartReplacementRejectionReason::SameAsOriginal => "copied source",
        crate::types::SmartReplacementRejectionReason::ContainsOriginal => "source text included",
        crate::types::SmartReplacementRejectionReason::ControlCharacter => "control character",
        crate::types::SmartReplacementRejectionReason::DuplicateOriginal => "duplicate source",
        crate::types::SmartReplacementRejectionReason::DuplicateOutput => "duplicate output",
    }
}

fn unselected_detector_risk_columns(columns: &[ColumnMetadata]) -> Vec<String> {
    columns
        .iter()
        .filter(|column| {
            !column.is_selected
                && matches!(
                    column.pii_risk,
                    crate::types::PiiRisk::High | crate::types::PiiRisk::Medium
                )
        })
        .map(|column| column.name.clone())
        .collect()
}
