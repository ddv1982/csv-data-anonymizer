use crate::detection::{
    POSSIBLE_PERSON_NAME_DETECTOR, analyze_column_privacy, classify_pii_risk, max_pii_risk,
};
use crate::error::{AnonymizerError, Result};
use crate::strategies::STRUCTURED_SCALAR_REDACTION_WARNING;
use crate::types::{
    AnonymizationStrategy, ColumnControl, ColumnMetadata, ColumnValueDistribution,
    FrequencyInversionRisk, PreviewWarning, WarningSeverity,
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
        AnonymizationStrategy::Label => {
            "Labelled placeholders name the column and stay consistent, so repeated values remain linkable and the column's value distribution is preserved."
                .to_string()
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

/// Warns that this column's consistent pseudonyms could be relabelled by frequency.
///
/// Separate from [`preview_warning_for_column`] rather than another arm of it, and
/// that separation is load-bearing: two report builders read *whether* that function
/// returned a warning as a proxy for "this column is effectively pass-through". A
/// cardinality note folded into it would silently reclassify a pseudonymized column
/// as pass-through in the privacy report and in the release evidence.
///
/// Only strategies that preserve equality can leak a distribution, which is the same
/// set the transform ledger records. Redact collapses a column to one token and mask
/// rewrites each value independently, so neither exposes a histogram to invert.
pub(crate) fn cardinality_warning_for_column(
    column: &ColumnMetadata,
    population_values: usize,
) -> Option<PreviewWarning> {
    if !keeps_consistent_mapping(column) {
        return None;
    }

    let distribution = column.sample_value_distribution;
    // Judged against the column's real size, not the sample's: a sample of a hundred
    // cannot show a ratio the sample's own size makes unreachable.
    let risk = distribution.frequency_inversion_risk_in(population_values)?;

    Some(PreviewWarning {
        column_index: column.index,
        column_name: column.name.clone(),
        message: frequency_inversion_message(&column.name, distribution, population_values, risk),
        severity: WarningSeverity::Warning,
    })
}

/// Whether this column's strategy gives a repeated source value the same replacement,
/// which is what forces the transform to remember every distinct value it sees.
///
/// One predicate, two callers, because it is one property with two consequences.
/// [`cardinality_warning_for_column`] reads it as "this column exposes a value
/// distribution that can be inverted"; the preflight memory projection in
/// `service::preflight` reads it as "this column costs mapping entries". Both follow
/// from preserving equality, and a strategy listed in one place and not the other
/// would either warn about a column that leaks nothing or project no memory for a
/// column that holds a mapping.
///
/// Redact collapses a column to a single token and Mask rewrites every value
/// independently, so neither preserves equality: nothing to invert, nothing to
/// remember. Under Auto and Pseudonymize a type that defaults to pass-through is
/// returned unchanged and is equally free.
///
/// No wildcard arm on purpose. A strategy added to the enum has to be classified here
/// rather than falling into the free, silent half by default.
pub(crate) fn keeps_consistent_mapping(column: &ColumnMetadata) -> bool {
    match column.strategy {
        AnonymizationStrategy::Tokenize
        | AnonymizationStrategy::LocalAi
        | AnonymizationStrategy::Label => true,
        AnonymizationStrategy::Auto | AnonymizationStrategy::Pseudonymize => {
            !column.detected_type.uses_default_pass_through()
        }
        AnonymizationStrategy::Mask
        | AnonymizationStrategy::Redact
        | AnonymizationStrategy::PassThrough => false,
    }
}

/// The sentence shared by all three wordings below, and the reason it is a constant.
///
/// The mechanism is identical whichever test fired — a repeated value keeps one
/// replacement — so every message ends on it, and tests that only need to know *that* a
/// cardinality warning fired match on this rather than on the evidence clause. Keying a
/// filter on wording specific to one term is what made the earlier tests silently stop
/// covering the other two.
pub(crate) const FREQUENCY_INVERSION_MECHANISM: &str = "Repeated values keep the same replacement, so the mapping can be matched back by how often each value occurs.";

/// Names the evidence, not just the verdict.
///
/// Each wording leads with the figure that made the column suspect, then the shared
/// mechanism. The sample size and the row count both appear because they are what tell
/// a user whether the finding is measured or estimated: a 100-value sample of five
/// million rows and a fully counted 200-row column produce the same warning otherwise,
/// and only one of them is worth acting on without checking.
fn frequency_inversion_message(
    column_name: &str,
    distribution: ColumnValueDistribution,
    population_values: usize,
    risk: FrequencyInversionRisk,
) -> String {
    let evidence = match risk {
        FrequencyInversionRisk::FewDistinctValues => format!(
            "{column_name} holds only {} distinct value(s) in a {}-value sample of {population_values} row(s).",
            distribution.distinct_values, distribution.total_values
        ),
        // Rounded to whole percent: the share is an estimate drawn from a sample, and a
        // figure like "51.4%" claims a precision the measurement does not have.
        FrequencyInversionRisk::DominantValue { share } => format!(
            "One value fills {:.0}% of {column_name}'s {} measured value(s), out of {population_values} row(s).",
            share * 100.0,
            distribution.total_values
        ),
        FrequencyInversionRisk::LargeGroups {
            estimated_distinct_values,
        } => format!(
            "{column_name} holds an estimated {estimated_distinct_values} distinct value(s) across {population_values} row(s), so each replacement covers around {} of them.",
            population_values / estimated_distinct_values.max(1)
        ),
    };

    let mut message = evidence;
    message.push(' ');
    message.push_str(FREQUENCY_INVERSION_MECHANISM);
    if matches!(risk, FrequencyInversionRisk::DominantValue { .. }) {
        message
            .push_str(" Identifying that one value recovers that share of the column at a stroke.");
    }
    message
}

/// Reports a column that may hold people's names but that detection could not type.
///
/// The gap this closes: person-name detection is header-gated, and the taxonomy
/// enumerates `<word> name` compounds one at a time, so `agent_name`,
/// `employee_name`, `reviewer_name` and their kind were classified `String`/Low and
/// left unselected. A user who accepted the app's own selections wrote those names out
/// unchanged, with nothing anywhere saying so.
///
/// Why a warning rather than a higher risk classification. The evidence available here
/// is that the header ends in a name term and the values are shaped like names — which
/// is equally true of `city_name` holding `New York`, and no heuristic in this codebase
/// separates those, because English surnames are largely toponymic. Escalating the risk
/// would auto-select and redact both, so a column of cities would be destroyed by
/// default to protect a column of people. Saying nothing loses the person. A warning
/// says exactly what is known, and leaves a decision that needs a human to one.
///
/// Suppressed once the column is selected: at that point the user has seen it and
/// chosen a strategy, and the other preview warnings cover what that strategy does.
pub(crate) fn possible_person_name_warning_for_column(
    column: &ColumnMetadata,
) -> Option<PreviewWarning> {
    if column.is_selected {
        return None;
    }
    if !column
        .privacy_evidence
        .iter()
        .any(|evidence| evidence.detector == POSSIBLE_PERSON_NAME_DETECTOR)
    {
        return None;
    }

    Some(PreviewWarning {
        column_index: column.index,
        column_name: column.name.clone(),
        message: format!(
            "{} is not selected and its values look like names — but they could equally be places, products or organisations, which detection cannot tell apart. If they name people, select the column: left unselected, it is copied into the output unchanged.",
            column.name
        ),
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
