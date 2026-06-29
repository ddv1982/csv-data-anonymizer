use crate::report_notes::push_unselected_column_note;
use crate::service::{preview_warning_for_column, redaction_changes_structured_scalar_type};
use crate::strategies::STRUCTURED_SCALAR_REDACTION_WARNING;
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, ColumnReleaseReport, ColumnRole, DpBudgetReport,
    DpBudgetStatus, PrivacyConfig, ReleaseEvidenceItem, ReleaseEvidenceStatus, ReleaseMode,
    ReleaseReadiness, ReleaseReadinessStatus, TransformReport, UtilityMetric,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ReportContext<'a> {
    pub roles: Option<&'a [ColumnRole]>,
    pub transform_report: Option<&'a TransformReport>,
    pub formal_min_class_size: Option<usize>,
    pub formal_suppressed_rows: Option<usize>,
    pub formal_released_rows: Option<usize>,
    pub dp_budget: Option<&'a DpBudgetReport>,
    pub dp_group_count: Option<usize>,
    pub synthetic_rows: Option<usize>,
    pub deterministic: bool,
}

pub(crate) fn build_readiness(
    mode: ReleaseMode,
    columns: &[ColumnMetadata],
    config: Option<&PrivacyConfig>,
    context: &ReportContext<'_>,
) -> ReleaseReadiness {
    let mut blockers = Vec::new();
    let mut review_items = Vec::new();
    let mut verified_items = Vec::new();

    if context.deterministic {
        verified_items.push("Repeatable replacements used a non-empty private seed.".to_string());
    } else {
        verified_items.push(
            "Randomized replacements do not reuse a persisted deterministic seed.".to_string(),
        );
    }

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

    match mode {
        ReleaseMode::Standard => {
            review_items.push(
                "Standard CSV transform is risk reduction, not a formal anonymity guarantee."
                    .to_string(),
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
        }
        ReleaseMode::FormalTabular => {
            if columns.iter().any(|column| !column.is_selected) {
                blockers.push(
                    "Formal tabular releases require every source column to be selected."
                        .to_string(),
                );
            }
            if let (Some(min_class), Some(config)) = (
                context.formal_min_class_size,
                config.map(|config| &config.formal),
            ) {
                if min_class >= config.k {
                    verified_items.push(format!(
                        "Every released equivalence class met k >= {}.",
                        config.k
                    ));
                } else {
                    review_items.push(format!(
                        "Minimum released equivalence class size was {min_class}, below k={}.",
                        config.k
                    ));
                }
            }
        }
        ReleaseMode::DifferentialPrivacyAggregate => {
            verified_items.push(
                "DP aggregate mode wrote noisy statistics instead of source rows.".to_string(),
            );
            if let Some(report) = context.dp_budget {
                if report.status == DpBudgetStatus::OverBudget {
                    review_items.push(
                        "This DP release exceeded the configured local budget and was allowed by warn mode."
                            .to_string(),
                    );
                } else {
                    verified_items.push(format!(
                        "Local DP budget is {} after this release.",
                        budget_status_label(report.status)
                    ));
                }
            } else {
                review_items.push(
                    "No local DP budget history was attached to this release report.".to_string(),
                );
            }
        }
        ReleaseMode::SyntheticData => {
            verified_items.push(
                "Synthetic/test data mode wrote generated rows, not source rows.".to_string(),
            );
            review_items.push(
                "Synthetic rows are generated independently per column and do not provide a DP guarantee."
                    .to_string(),
            );
        }
    }

    let status = if !blockers.is_empty() {
        ReleaseReadinessStatus::Blocked
    } else if review_items.is_empty() {
        ReleaseReadinessStatus::Verified
    } else {
        ReleaseReadinessStatus::Review
    };

    ReleaseReadiness {
        status,
        blockers,
        review_items,
        verified_items,
    }
}

pub(crate) fn build_evidence(
    mode: ReleaseMode,
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

    match mode {
        ReleaseMode::FormalTabular => {
            if let Some(min_class) = context.formal_min_class_size {
                evidence.push(ReleaseEvidenceItem {
                    id: "formal-class-size".to_string(),
                    label: "Equivalence classes".to_string(),
                    status: ReleaseEvidenceStatus::Info,
                    detail: format!("Minimum released equivalence class size: {min_class}."),
                });
            }
            if let Some(suppressed) = context.formal_suppressed_rows {
                evidence.push(ReleaseEvidenceItem {
                    id: "formal-suppression".to_string(),
                    label: "Suppression".to_string(),
                    status: if suppressed == 0 {
                        ReleaseEvidenceStatus::Verified
                    } else {
                        ReleaseEvidenceStatus::Review
                    },
                    detail: format!(
                        "{suppressed} source row(s) suppressed by formal release settings."
                    ),
                });
            }
        }
        ReleaseMode::DifferentialPrivacyAggregate => {
            if let Some(budget) = context.dp_budget {
                evidence.push(ReleaseEvidenceItem {
                    id: "dp-budget".to_string(),
                    label: "DP budget".to_string(),
                    status: if budget.status == DpBudgetStatus::OverBudget {
                        ReleaseEvidenceStatus::Review
                    } else {
                        ReleaseEvidenceStatus::Verified
                    },
                    detail: format!(
                        "Spent {} before, {} after, limit {}, remaining {}.",
                        budget.spent_epsilon_before,
                        budget.spent_epsilon_after,
                        budget.limit_epsilon,
                        budget.remaining_epsilon
                    ),
                });
            }
        }
        ReleaseMode::SyntheticData => {
            evidence.push(ReleaseEvidenceItem {
                id: "synthetic-rows".to_string(),
                label: "Generated rows".to_string(),
                status: ReleaseEvidenceStatus::Info,
                detail: format!(
                    "{} generated row(s) written.",
                    context.synthetic_rows.unwrap_or_default()
                ),
            });
        }
        ReleaseMode::Standard => {}
    }

    evidence
}

pub(crate) fn build_column_reports(
    mode: ReleaseMode,
    columns: &[ColumnMetadata],
    roles: Option<&[ColumnRole]>,
) -> Vec<ColumnReleaseReport> {
    columns
        .iter()
        .map(|column| {
            let role = roles.and_then(|roles| roles.get(column.index)).copied();
            let (action, status, detail) = column_action(mode, column, role);
            ColumnReleaseReport {
                column_index: column.index,
                column_name: column.name.clone(),
                selected: column.is_selected,
                detected_type: column.detected_type,
                pii_risk: column.pii_risk,
                strategy: column.strategy,
                role,
                action,
                status,
                detail,
            }
        })
        .collect()
}

pub(crate) fn build_utility_metrics(
    mode: ReleaseMode,
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
        detail: Some(
            "Columns outside selection can remain unchanged depending on release mode.".to_string(),
        ),
    });

    match mode {
        ReleaseMode::Standard => {
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
                    detail: Some(
                        if report.smart_replacement_rejections > 0 {
                            format!(
                                "Accepted structured Local AI replacements before rule-based fallback. Rejections: {}.",
                                smart_rejection_summary(report)
                            )
                        } else {
                            "Accepted structured Local AI replacements before rule-based fallback."
                                .to_string()
                        },
                    ),
                });
            }
        }
        ReleaseMode::FormalTabular => {
            if let (Some(suppressed), Some(released)) =
                (context.formal_suppressed_rows, context.formal_released_rows)
            {
                metrics.push(UtilityMetric {
                    label: "Released rows".to_string(),
                    value: released.to_string(),
                    status: ReleaseEvidenceStatus::Info,
                    detail: Some(format!("{suppressed} row(s) were suppressed.")),
                });
            }
        }
        ReleaseMode::DifferentialPrivacyAggregate => {
            if let Some(count) = context.dp_group_count {
                metrics.push(UtilityMetric {
                    label: "Aggregate rows".to_string(),
                    value: count.to_string(),
                    status: ReleaseEvidenceStatus::Info,
                    detail: Some(
                        "DP output row count is aggregate rows, not source rows.".to_string(),
                    ),
                });
            }
        }
        ReleaseMode::SyntheticData => {
            metrics.push(UtilityMetric {
                label: "Synthetic rows".to_string(),
                value: context.synthetic_rows.unwrap_or_default().to_string(),
                status: ReleaseEvidenceStatus::Info,
                detail: Some(
                    "Generated independently per column for test-data utility.".to_string(),
                ),
            });
        }
    }

    metrics
}

pub(crate) fn standard_notes(
    columns: &[ColumnMetadata],
    transform_report: TransformReport,
    deterministic: bool,
) -> Vec<String> {
    let mut notes = vec![
        "Standard CSV transform changes selected cells in place with local strategies such as masking, redaction, tokenization, pseudonymization, pass-through, and optional Local AI replacement."
            .to_string(),
        "Treat this as risk reduction, not proof of anonymity; review the output against your sharing context and re-identification risk."
            .to_string(),
        "For k-anonymity, l-diversity, t-closeness, DP aggregate output, or synthetic/test rows, rerun with the matching Privacy Release mode selected."
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
                && preview_warning_for_column(column).is_none()
        })
    {
        if deterministic {
            notes.push(
                "Deterministic pseudonyms and tokens use keyed HMAC-SHA256 with the configured seed; treat that seed as sensitive."
                    .to_string(),
            );
        } else {
            notes.push(
                "Random-mode pseudonyms and tokens are tracked within each run so repeated source values stay consistent while distinct readable names avoid reuse while capacity remains."
                    .to_string(),
            );
        }
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

    notes
}

fn column_action(
    mode: ReleaseMode,
    column: &ColumnMetadata,
    role: Option<ColumnRole>,
) -> (String, ReleaseEvidenceStatus, String) {
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

    match mode {
        ReleaseMode::Standard => match column.strategy {
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
                "Selected values become stable opaque tokens.".to_string(),
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
                if preview_warning_for_column(column).is_some() {
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
        },
        ReleaseMode::FormalTabular => match role.unwrap_or(ColumnRole::Attribute) {
            ColumnRole::DirectIdentifier => (
                "Redacted".to_string(),
                ReleaseEvidenceStatus::Verified,
                "Direct identifiers are replaced with a redaction marker.".to_string(),
            ),
            ColumnRole::QuasiIdentifier => (
                "Generalized".to_string(),
                ReleaseEvidenceStatus::Verified,
                "Quasi-identifiers are generalized before formal checks.".to_string(),
            ),
            ColumnRole::Sensitive => (
                "Sensitive retained".to_string(),
                ReleaseEvidenceStatus::Review,
                "Sensitive values are retained for l-diversity/t-closeness checks.".to_string(),
            ),
            ColumnRole::Exclude => (
                "Excluded".to_string(),
                ReleaseEvidenceStatus::Verified,
                "Values are blanked in the release output.".to_string(),
            ),
            ColumnRole::Auto | ColumnRole::Attribute => (
                "Attribute retained".to_string(),
                ReleaseEvidenceStatus::Info,
                "Attribute values are retained in the release output.".to_string(),
            ),
        },
        ReleaseMode::DifferentialPrivacyAggregate => (
            "Aggregate input".to_string(),
            ReleaseEvidenceStatus::Verified,
            "Column may contribute to noisy aggregate output; source rows are not written.".to_string(),
        ),
        ReleaseMode::SyntheticData => (
            "Generated".to_string(),
            ReleaseEvidenceStatus::Info,
            "Column values are generated for test-data output.".to_string(),
        ),
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

fn budget_status_label(status: DpBudgetStatus) -> &'static str {
    match status {
        DpBudgetStatus::WithinBudget => "within budget",
        DpBudgetStatus::AtBudget => "at budget",
        DpBudgetStatus::OverBudget => "over budget",
    }
}
