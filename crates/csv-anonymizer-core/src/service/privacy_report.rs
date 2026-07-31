use crate::release_report::{
    ReportContext, build_column_reports, build_evidence, build_readiness, build_utility_metrics,
    standard_notes,
};
use crate::types::{
    AnonymizationStrategy, ColumnMetadata, DetectionCoverage, PrivacyFindingKind, PrivacyReport,
    ReportIdentifierClass, TransformReport,
};

pub(crate) fn build_privacy_report(
    columns: &[ColumnMetadata],
    transform_report: TransformReport,
    detection_coverage: DetectionCoverage,
) -> PrivacyReport {
    let mut report = PrivacyReport {
        detection_run_summary: None,
        direct_identifiers: 0,
        quasi_identifiers: 0,
        pseudonymized_columns: 0,
        smart_replacement_columns: 0,
        opaque_token_columns: 0,
        masked_columns: 0,
        labelled_columns: 0,
        redacted_columns: 0,
        pass_through_columns: 0,
        unique_pseudonym_values: transform_report.unique_pseudonym_values,
        reused_pseudonym_values: transform_report.reused_pseudonym_values,
        collisions_avoided: transform_report.collisions_avoided,
        exhausted_pseudonym_pools: transform_report.exhausted_pseudonym_pools,
        opaque_token_values: transform_report.opaque_token_values,
        smart_replacement_values: transform_report.smart_replacement_values,
        smart_replacement_rejections: transform_report.smart_replacement_rejections,
        smart_replacement_rejection_reasons: transform_report
            .smart_replacement_rejection_reasons
            .clone(),
        smart_replacement_fallbacks: transform_report.smart_replacement_fallbacks,
        shape_fallback_values: transform_report.shape_fallback_values,
        // Carried even though no UI component renders it, which makes it look like dead
        // payload. It is the evidence behind the frequency-inversion note, and it is the
        // only place the exact post-run figures are observable: `TransformReport` never
        // leaves the core, and the notes state distinct and total counts in prose but not
        // `max_value_occurrences`. Removing it was tried and reverted — it takes the
        // ledger's exactness out of reach of the tests that check it, which is the part
        // of this feature most worth being able to check.
        column_value_distributions: transform_report.column_value_distributions.clone(),
        row_uniqueness: transform_report.row_uniqueness.clone(),
        readiness: Default::default(),
        evidence: Vec::new(),
        column_reports: Vec::new(),
        utility_metrics: Vec::new(),
        notes: standard_notes(columns, transform_report.clone(), detection_coverage),
    };

    for column in columns.iter().filter(|column| column.is_selected) {
        match report_identifier_class_for_column(column) {
            Some(ReportIdentifierClass::Direct) => report.direct_identifiers += 1,
            Some(ReportIdentifierClass::Quasi) => report.quasi_identifiers += 1,
            None => {}
        }

        match column.strategy {
            AnonymizationStrategy::Mask => report.masked_columns += 1,
            AnonymizationStrategy::Redact => report.redacted_columns += 1,
            AnonymizationStrategy::PassThrough => report.pass_through_columns += 1,
            AnonymizationStrategy::Tokenize => report.opaque_token_columns += 1,
            AnonymizationStrategy::LocalAi => report.smart_replacement_columns += 1,
            AnonymizationStrategy::Label => report.labelled_columns += 1,
            AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
                // Asks the pass-through predicate directly rather than reading it off
                // `preview_warning_for_column`. Under these two strategies a warning
                // means exactly this, but the warning list is free to grow — a
                // cardinality note would otherwise silently reclassify a
                // pseudonymized column as pass-through.
                if column.detected_type.uses_default_pass_through() {
                    report.pass_through_columns += 1;
                } else {
                    report.pseudonymized_columns += 1;
                }
            }
        }
    }

    let context = ReportContext {
        transform_report: Some(&transform_report),
    };
    report.readiness = build_readiness(columns, &context);
    report.evidence = build_evidence(columns, &context);
    report.column_reports = build_column_reports(columns);
    report.utility_metrics = build_utility_metrics(columns, &context);

    report
}

fn report_identifier_class_for_column(column: &ColumnMetadata) -> Option<ReportIdentifierClass> {
    column.privacy_evidence.iter().fold(
        column.detected_type.report_identifier_class(),
        |current, evidence| {
            let evidence_class = evidence
                .data_type
                .report_identifier_class()
                .or_else(|| identifier_class_for_privacy_kind(evidence.kind));
            strongest_identifier_class(current, evidence_class)
        },
    )
}

fn identifier_class_for_privacy_kind(kind: PrivacyFindingKind) -> Option<ReportIdentifierClass> {
    match kind {
        PrivacyFindingKind::Person
        | PrivacyFindingKind::Contact
        | PrivacyFindingKind::PrivateAddress
        | PrivacyFindingKind::GovernmentId
        | PrivacyFindingKind::CredentialOrSecret
        | PrivacyFindingKind::MixedSensitiveText => Some(ReportIdentifierClass::Direct),
        PrivacyFindingKind::PrivateDate
        | PrivacyFindingKind::AddressRegion
        | PrivacyFindingKind::AccountOrFinancialId
        | PrivacyFindingKind::RecordIdentifier
        | PrivacyFindingKind::NetworkOrDeviceId
        | PrivacyFindingKind::Url => Some(ReportIdentifierClass::Quasi),
    }
}

fn strongest_identifier_class(
    left: Option<ReportIdentifierClass>,
    right: Option<ReportIdentifierClass>,
) -> Option<ReportIdentifierClass> {
    match (left, right) {
        (Some(ReportIdentifierClass::Direct), _) | (_, Some(ReportIdentifierClass::Direct)) => {
            Some(ReportIdentifierClass::Direct)
        }
        (Some(ReportIdentifierClass::Quasi), _) | (_, Some(ReportIdentifierClass::Quasi)) => {
            Some(ReportIdentifierClass::Quasi)
        }
        (None, None) => None,
    }
}

pub(crate) fn count_transforming_selected_columns(columns: &[ColumnMetadata]) -> usize {
    columns
        .iter()
        .filter(|column| column.is_selected && strategy_changes_output(column))
        .count()
}

fn strategy_changes_output(column: &ColumnMetadata) -> bool {
    match column.strategy {
        AnonymizationStrategy::Mask
        | AnonymizationStrategy::Redact
        | AnonymizationStrategy::Label
        | AnonymizationStrategy::Tokenize
        | AnonymizationStrategy::LocalAi => true,
        AnonymizationStrategy::PassThrough => false,
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            !column.detected_type.uses_default_pass_through()
        }
    }
}
