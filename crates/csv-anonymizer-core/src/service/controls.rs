use crate::detection::{analyze_column_privacy, classify_pii_risk, max_pii_risk};
use crate::error::{AnonymizerError, Result};
use crate::strategies::STRUCTURED_SCALAR_REDACTION_WARNING;
use crate::types::{
    AnonymizationStrategy, ColumnControl, ColumnMetadata, PreviewWarning, WarningSeverity,
};

pub(crate) fn validate_column_indices(
    metadata: &[ColumnMetadata],
    columns: &[usize],
) -> Result<()> {
    let max_index = metadata.len().saturating_sub(1);
    for index in columns {
        if *index >= metadata.len() {
            return Err(AnonymizerError::ColumnOutOfRange {
                index: *index,
                max_index,
            });
        }
    }
    Ok(())
}

pub(crate) fn apply_column_controls(
    metadata: &[ColumnMetadata],
    controls: &[ColumnControl],
) -> Result<Vec<ColumnMetadata>> {
    let mut controlled = metadata.to_vec();
    for control in controls {
        let Some(column) = controlled.get_mut(control.column_index) else {
            return Err(AnonymizerError::ColumnOutOfRange {
                index: control.column_index,
                max_index: metadata.len().saturating_sub(1),
            });
        };

        if let Some(data_type) = control.type_override {
            column.detected_type = data_type;
            let privacy = analyze_column_privacy(
                &column.name,
                column.index,
                &column.sample_values,
                data_type,
                column.confidence,
            );
            for finding in privacy.findings {
                if !column.privacy_findings.contains(&finding) {
                    column.privacy_findings.push(finding);
                }
            }
            for evidence in privacy.evidence {
                let already_recorded = column.privacy_evidence.iter().any(|existing| {
                    existing.kind == evidence.kind
                        && existing.data_type == evidence.data_type
                        && existing.detector == evidence.detector
                        && existing.reason == evidence.reason
                });
                if !already_recorded {
                    column.privacy_evidence.push(evidence);
                }
            }
            column.pii_risk = max_pii_risk(
                column.pii_risk,
                max_pii_risk(classify_pii_risk(data_type), privacy.pii_risk),
            );
        }
        column.strategy = control.strategy;
    }
    Ok(controlled)
}

pub(crate) fn preview_warning_for_column(column: &ColumnMetadata) -> Option<PreviewWarning> {
    let message = match column.strategy {
        AnonymizationStrategy::PassThrough => {
            "Pass-through leaves selected values unchanged.".to_string()
        }
        AnonymizationStrategy::LocalAi => {
            "Smart replacement uses Local AI on your device. Review the preview before writing output."
                .to_string()
        }
        AnonymizationStrategy::Redact if redaction_changes_structured_scalar_type(column) => {
            STRUCTURED_SCALAR_REDACTION_WARNING.to_string()
        }
        AnonymizationStrategy::Redact => return None,
        AnonymizationStrategy::Mask | AnonymizationStrategy::Tokenize => return None,
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            if column.detected_type.uses_default_pass_through() {
                format!("{} currently uses pass-through behavior.", column.name)
            } else {
                return None;
            }
        }
    };

    Some(PreviewWarning {
        column_index: column.index,
        column_name: column.name.clone(),
        message,
        severity: WarningSeverity::Warning,
    })
}

pub(crate) fn redaction_changes_structured_scalar_type(column: &ColumnMetadata) -> bool {
    column.strategy == AnonymizationStrategy::Redact
        && is_json_or_yaml_source(column)
        && column
            .detected_type
            .redaction_changes_structured_scalar_type()
}

fn is_json_or_yaml_source(column: &ColumnMetadata) -> bool {
    column.source_path.as_deref().is_some_and(|path| {
        matches!(path, "json" | "yaml") || path.starts_with("json/") || path.starts_with("yaml/")
    })
}
