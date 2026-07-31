use crate::types::{
    AnonymizationStrategy, ColumnEvidenceProfile, ColumnMetadata, ColumnReviewReason, Confidence,
    DataType, FormatEvidence, FormatEvidenceBasis, PrivacyDecision, PrivacyFindingKind,
    RedactionDecision, RedactionPlaceholder, RedactionPlaceholderSource, SemanticDecision,
    SemanticSpecificity, SemanticStatus,
};

pub(crate) const EMAIL: &str = "[EMAIL]";
pub(crate) const PHONE: &str = "[PHONE]";
pub(crate) const PERSON: &str = "[PERSON]";
pub(crate) const ADDRESS: &str = "[ADDRESS]";
pub(crate) const DATE: &str = "[DATE]";
pub(crate) const ACCOUNT_ID: &str = "[ACCOUNT_ID]";
pub(crate) const GOVERNMENT_ID: &str = "[GOVERNMENT_ID]";
pub(crate) const SECRET: &str = "[SECRET]";
pub(crate) const URL: &str = "[URL]";
pub(crate) const NETWORK_ID: &str = "[NETWORK_ID]";
pub(crate) const CONTACT: &str = "[CONTACT]";

pub(crate) const STRUCTURED_SCALAR_REDACTION_WARNING: &str =
    "Redact uses string placeholders and may change scalar value types.";

pub(super) fn placeholder_for_column(column: &ColumnMetadata) -> String {
    if !column
        .evidence_profile
        .redaction_decision
        .placeholder
        .is_empty()
    {
        return column
            .evidence_profile
            .redaction_decision
            .placeholder
            .clone();
    }
    computed_placeholder(column).0
}

fn computed_placeholder(
    column: &ColumnMetadata,
) -> (
    String,
    RedactionPlaceholderSource,
    Option<PrivacyFindingKind>,
) {
    if let Some(placeholder) = column
        .detected_type
        .redaction_placeholder()
        .map(placeholder_text)
    {
        return (
            placeholder.to_string(),
            RedactionPlaceholderSource::Typed,
            column
                .detected_type
                .privacy_finding_kind_and_reason()
                .map(|(kind, _)| kind),
        );
    }
    if let Some((placeholder, kind)) = placeholder_from_evidence(column) {
        return (
            placeholder.to_string(),
            RedactionPlaceholderSource::Typed,
            Some(kind),
        );
    }
    (
        column_placeholder(column),
        RedactionPlaceholderSource::ColumnHeader,
        strongest_actionable_evidence(column).map(|evidence| evidence.kind),
    )
}

/// Rebuilds the serialized decisions from the evidence and controls on a column.
///
/// Detection, preview and transformation all call this same policy. The transform
/// subsequently reads `redaction_decision.placeholder`, making the backend-issued
/// marker authoritative instead of asking each consumer to reproduce these rules.
pub(crate) fn build_evidence_profile(column: &ColumnMetadata) -> ColumnEvidenceProfile {
    let sample_count = column
        .sample_values
        .iter()
        .filter(|value| !crate::detection::is_empty_value(value))
        .count();
    let matching_trace = column.detection_trace.as_ref().and_then(|trace| {
        trace
            .candidates
            .iter()
            .find(|candidate| candidate.accepted && candidate.data_type == column.detected_type)
    });
    let (format_match_count, format_sample_count, format_basis) =
        if let Some(candidate) = matching_trace {
            (
                candidate.match_count,
                candidate.total_considered,
                FormatEvidenceBasis::DetectionSample,
            )
        } else if let Some(trace) = &column.detection_trace {
            (0, trace.total_non_empty, FormatEvidenceBasis::UserOverride)
        } else {
            (
                sample_count,
                sample_count,
                FormatEvidenceBasis::RetainedPreviewValues,
            )
        };
    let mut detectors = column
        .privacy_evidence
        .iter()
        .filter(|evidence| evidence.data_type == column.detected_type)
        .flat_map(|evidence| {
            std::iter::once(evidence.detector.clone()).chain(evidence.detectors.iter().cloned())
        })
        .filter(|detector| !detector.is_empty())
        .collect::<Vec<_>>();
    detectors.sort();
    detectors.dedup();

    let strongest = strongest_actionable_evidence(column);
    let semantic_kind = strongest.map(|evidence| evidence.kind).or_else(|| {
        column
            .detected_type
            .privacy_finding_kind_and_reason()
            .map(|(kind, _)| kind)
    });
    let semantic_confidence = strongest
        .map(|evidence| evidence.confidence)
        .unwrap_or(column.confidence);
    let conflicting = column
        .review_reasons
        .contains(&ColumnReviewReason::DetectorsDisagree);
    let insufficient = sample_count == 0
        || column
            .review_reasons
            .contains(&ColumnReviewReason::InsufficientSample);
    let specificity = match semantic_kind {
        Some(PrivacyFindingKind::RecordIdentifier | PrivacyFindingKind::MixedSensitiveText)
        | None => SemanticSpecificity::Generic,
        Some(_) => SemanticSpecificity::Specific,
    };
    let status = if conflicting {
        SemanticStatus::Conflicting
    } else if insufficient {
        SemanticStatus::Uncertain
    } else if semantic_kind.is_some() && semantic_confidence != Confidence::Low {
        SemanticStatus::Resolved
    } else {
        SemanticStatus::Uncertain
    };
    let supporting_evidence = strongest
        .map(|evidence| {
            let mut supporting = evidence.detectors.clone();
            if supporting.is_empty() && !evidence.detector.is_empty() {
                supporting.push(evidence.detector.clone());
            }
            supporting
        })
        .unwrap_or_default();
    let conflicting_evidence = if conflicting {
        column
            .privacy_evidence
            .iter()
            .map(|evidence| evidence.detector.clone())
            .filter(|detector| !detector.is_empty())
            .collect()
    } else {
        Vec::new()
    };
    let semantic_reason = strongest
        .map(|evidence| evidence.reason.clone())
        .or_else(|| {
            column
                .detected_type
                .privacy_finding_kind_and_reason()
                .map(|(_, reason)| reason.to_string())
        })
        .unwrap_or_else(|| {
            "The value format does not establish a specific privacy meaning.".into()
        });
    let (placeholder, source, _) = computed_placeholder(column);
    let is_typed = source == RedactionPlaceholderSource::Typed;
    let placeholder_reason = if is_typed {
        semantic_reason.clone()
    } else {
        "Semantic evidence is not specific enough; using one constant marker derived from the published column header.".into()
    };
    let recommended_strategy = if column.pii_risk.is_elevated() {
        AnonymizationStrategy::Redact
    } else {
        AnonymizationStrategy::Auto
    };

    ColumnEvidenceProfile {
        format_evidence: FormatEvidence {
            data_type: column.detected_type,
            confidence: column.confidence,
            match_count: format_match_count,
            sample_count: format_sample_count,
            basis: format_basis,
            detectors,
        },
        semantic_decision: SemanticDecision {
            kind: semantic_kind
                .map(semantic_kind_name)
                .unwrap_or("unknown")
                .to_string(),
            confidence: semantic_confidence,
            specificity,
            status,
            supporting_evidence,
            conflicting_evidence,
            reason: semantic_reason,
        },
        privacy_decision: PrivacyDecision {
            risk: column.pii_risk,
            recommended_strategy,
            auto_selected: !column.sample_values.is_empty() && column.pii_risk.is_elevated(),
            reason: if column.pii_risk.is_elevated() {
                "Privacy evidence is elevated, so redaction is recommended.".into()
            } else {
                "No actionable elevated privacy evidence was established.".into()
            },
        },
        redaction_decision: RedactionDecision {
            placeholder,
            source,
            is_typed,
            preserves_equality: false,
            reason: placeholder_reason,
        },
    }
}

pub(crate) fn refresh_evidence_profile(column: &mut ColumnMetadata) {
    column.evidence_profile = build_evidence_profile(column);
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

/// How long a column label may get before it stops being readable in a cell.
///
/// A guard against pathological headers, not a meaningful boundary: real headers
/// are far shorter, and a label this long has already told the reader which column
/// they are in.
const MAX_LABEL_CHARS: usize = 40;

/// A placeholder naming the column the value came from and which distinct value of
/// that column this is: `[CUSTOMER_NOTES_1]`.
///
/// This exists for the case the type detectors cannot settle. When no validator
/// claims a value, the *header* is the only evidence left about what the cell held,
/// and it is evidence the output can carry: headers are never transformed, so naming
/// the column in the cell discloses nothing that row 1 does not already say.
///
/// Two properties are deliberate and neither is obvious:
///
/// - **The ordinal is per column, so identity is `(column, ordinal)` rather than a
///   document-wide namespace.** `[NOTES_1]` in one column is unrelated to `[NOTES_1]`
///   in another. A reader looking at a cell already knows which column they are in;
///   pooling cells out of their columns loses that, and the labels cannot recover it.
///   That scoping is also why a duplicated header has to be qualified: two columns
///   named `notes` would otherwise both begin at `[NOTES_1]` while holding unrelated
///   values, which reads as an equality nothing measured. See
///   [`ColumnMetadata::header_label_is_ambiguous`].
/// - **This is pseudonymisation, not anonymisation.** A stable ordinal preserves
///   equality, which is the point — it is what lets a reader see that two rows held
///   the same value. It is also a linkage key, and it exposes the column's value
///   distribution. That is why this is a strategy a user selects by name rather than
///   something applied on their behalf: `Redact` collapses a column and asserts
///   nothing, and silently turning that into a re-linkable mapping would be the
///   wrong kind of surprise.
pub(super) fn labelled_placeholder(column: &ColumnMetadata, ordinal: usize) -> String {
    let label = base_column_label(&column.name, column.index);
    if column.header_label_is_ambiguous {
        // Position before ordinal, so the qualifier reads as part of the column's
        // name rather than as a second counter.
        return format!("[{label}_{}_{ordinal}]", column.index);
    }
    format!("[{label}_{ordinal}]")
}

/// A non-linkable redaction marker derived only from the already-published header.
///
/// Unlike [`labelled_placeholder`], this carries no value ordinal: every non-empty
/// value in the column collapses to the same marker.
pub(super) fn column_placeholder(column: &ColumnMetadata) -> String {
    let label = base_column_label(&column.name, column.index);
    if column.header_label_is_ambiguous {
        return format!("[{label}_{}]", column.index);
    }
    format!("[{label}]")
}

/// The column's header as a placeholder-safe label: alphanumerics uppercased,
/// every other run collapsed to one `_`.
///
/// Falls back to the column's position when the header contributes no
/// alphanumerics at all — an unnamed or punctuation-only column still needs a
/// label, and its index is the one thing that always identifies it. Two unnamed
/// columns therefore land on distinct labels already, and are not ambiguous.
///
/// "Base" because it carries no positional qualifier for a duplicated header:
/// that is decided by comparing these labels across the whole column set, which is
/// why [`crate::metadata`] needs to compute the same thing this does.
pub(crate) fn base_column_label(name: &str, index: usize) -> String {
    let mut label = String::new();
    let mut character_count = 0usize;
    let mut separator_pending = false;

    for character in name.chars() {
        if !character.is_alphanumeric() {
            // Only pends a separator once something precedes it, so a leading run
            // of punctuation cannot produce a label starting with `_`.
            separator_pending = !label.is_empty();
            continue;
        }
        if separator_pending {
            label.push('_');
            character_count += 1;
            separator_pending = false;
        }
        // `to_uppercase` can widen one character into several, so the cap is
        // checked after the push rather than reserved before it.
        for uppercase in character.to_uppercase() {
            label.push(uppercase);
            character_count += 1;
        }
        if character_count >= MAX_LABEL_CHARS {
            break;
        }
    }

    if label.is_empty() {
        return format!("COLUMN_{index}");
    }
    label
}

fn placeholder_text(placeholder: RedactionPlaceholder) -> &'static str {
    match placeholder {
        RedactionPlaceholder::Email => EMAIL,
        RedactionPlaceholder::Phone => PHONE,
        RedactionPlaceholder::Person => PERSON,
        RedactionPlaceholder::Address => ADDRESS,
        RedactionPlaceholder::GovernmentId => GOVERNMENT_ID,
        RedactionPlaceholder::Url => URL,
        RedactionPlaceholder::NetworkId => NETWORK_ID,
    }
}

/// Names the placeholder after the strongest privacy finding the column carries.
///
/// A placeholder is a claim about what the cell held, so it may only be made on
/// evidence the app acts on elsewhere — see
/// [`PrivacyEvidenceSummary::is_actionable`](crate::types::PrivacyEvidenceSummary::is_actionable)
/// for why Low findings are recorded but not asserted. With none left, the constant
/// column-derived marker claims only what the published header already says.
///
/// The filter can only change the all-Low case: Low findings score 54-55 and every
/// Medium or High path scores at least 68, so a Low item could never have outranked a
/// stronger one. It removes a wrong label, not a wrong ranking.
fn strongest_actionable_evidence(
    column: &ColumnMetadata,
) -> Option<&crate::types::PrivacyEvidenceSummary> {
    column
        .privacy_evidence
        .iter()
        .filter(|item| item.is_actionable())
        .max_by_key(|item| (evidence_kind_priority(item.kind), item.score))
}

fn placeholder_from_evidence(
    column: &ColumnMetadata,
) -> Option<(&'static str, PrivacyFindingKind)> {
    let evidence = strongest_actionable_evidence(column)?;

    let placeholder = match evidence.kind {
        PrivacyFindingKind::Person => PERSON,
        PrivacyFindingKind::Contact => match evidence.data_type {
            DataType::Email => EMAIL,
            DataType::Phone => PHONE,
            _ => CONTACT,
        },
        PrivacyFindingKind::PrivateAddress | PrivacyFindingKind::AddressRegion => ADDRESS,
        PrivacyFindingKind::PrivateDate => DATE,
        PrivacyFindingKind::AccountOrFinancialId => ACCOUNT_ID,
        PrivacyFindingKind::RecordIdentifier => return None,
        PrivacyFindingKind::GovernmentId => GOVERNMENT_ID,
        PrivacyFindingKind::CredentialOrSecret => SECRET,
        PrivacyFindingKind::NetworkOrDeviceId => NETWORK_ID,
        PrivacyFindingKind::Url => URL,
        PrivacyFindingKind::MixedSensitiveText => return None,
    };
    Some((placeholder, evidence.kind))
}

fn evidence_kind_priority(kind: PrivacyFindingKind) -> u8 {
    match kind {
        PrivacyFindingKind::CredentialOrSecret => 100,
        PrivacyFindingKind::GovernmentId => 95,
        PrivacyFindingKind::Contact => 90,
        PrivacyFindingKind::Person => 85,
        PrivacyFindingKind::PrivateAddress => 80,
        PrivacyFindingKind::AddressRegion => 78,
        PrivacyFindingKind::AccountOrFinancialId => 75,
        PrivacyFindingKind::PrivateDate => 70,
        PrivacyFindingKind::NetworkOrDeviceId => 65,
        PrivacyFindingKind::Url => 60,
        // Least specific identifier there is, so any other finding names the column
        // better than "some key" does.
        PrivacyFindingKind::RecordIdentifier => 55,
        PrivacyFindingKind::MixedSensitiveText => 10,
    }
}
