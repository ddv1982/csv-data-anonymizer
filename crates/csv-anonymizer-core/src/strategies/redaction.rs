use crate::types::{ColumnMetadata, DataType, PrivacyFindingKind, RedactionPlaceholder};

pub(crate) const REDACTED: &str = "[REDACTED]";
pub(crate) const EMAIL: &str = "[EMAIL]";
pub(crate) const PHONE: &str = "[PHONE]";
pub(crate) const PERSON: &str = "[PERSON]";
pub(crate) const ADDRESS: &str = "[ADDRESS]";
pub(crate) const DATE: &str = "[DATE]";
pub(crate) const ACCOUNT_ID: &str = "[ACCOUNT_ID]";
pub(crate) const RECORD_ID: &str = "[RECORD_ID]";
pub(crate) const GOVERNMENT_ID: &str = "[GOVERNMENT_ID]";
pub(crate) const SECRET: &str = "[SECRET]";
pub(crate) const URL: &str = "[URL]";
pub(crate) const NETWORK_ID: &str = "[NETWORK_ID]";
pub(crate) const CONTACT: &str = "[CONTACT]";

pub(crate) const STRUCTURED_SCALAR_REDACTION_WARNING: &str =
    "Redact uses string placeholders and may change scalar value types.";

pub(super) fn placeholder_for_column(column: &ColumnMetadata) -> &'static str {
    column
        .detected_type
        .redaction_placeholder()
        .map(placeholder_text)
        .unwrap_or_else(|| placeholder_from_evidence(column))
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
        RedactionPlaceholder::Date => DATE,
        RedactionPlaceholder::RecordId => RECORD_ID,
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
/// for why Low findings are recorded but not asserted. With none left, `[REDACTED]`
/// claims nothing, which is the honest answer.
///
/// The filter can only change the all-Low case: Low findings score 54-55 and every
/// Medium or High path scores at least 68, so a Low item could never have outranked a
/// stronger one. It removes a wrong label, not a wrong ranking.
fn placeholder_from_evidence(column: &ColumnMetadata) -> &'static str {
    let Some(evidence) = column
        .privacy_evidence
        .iter()
        .filter(|item| item.is_actionable())
        .max_by_key(|item| (item.score, evidence_kind_priority(item.kind)))
    else {
        return REDACTED;
    };

    match evidence.kind {
        PrivacyFindingKind::Person => PERSON,
        PrivacyFindingKind::Contact => match evidence.data_type {
            DataType::Email => EMAIL,
            DataType::Phone => PHONE,
            _ => CONTACT,
        },
        PrivacyFindingKind::PrivateAddress | PrivacyFindingKind::AddressRegion => ADDRESS,
        PrivacyFindingKind::PrivateDate => DATE,
        PrivacyFindingKind::AccountOrFinancialId => ACCOUNT_ID,
        PrivacyFindingKind::RecordIdentifier => RECORD_ID,
        PrivacyFindingKind::GovernmentId => GOVERNMENT_ID,
        PrivacyFindingKind::CredentialOrSecret => SECRET,
        PrivacyFindingKind::NetworkOrDeviceId => NETWORK_ID,
        PrivacyFindingKind::Url => URL,
        PrivacyFindingKind::MixedSensitiveText => REDACTED,
    }
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
