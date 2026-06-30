use crate::types::{ColumnMetadata, DataType, PrivacyFindingKind, RedactionPlaceholder};

pub(crate) const REDACTED: &str = "[REDACTED]";
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

pub(super) fn placeholder_for_column(column: &ColumnMetadata) -> &'static str {
    column
        .detected_type
        .redaction_placeholder()
        .map(placeholder_text)
        .unwrap_or_else(|| placeholder_from_evidence(column))
}

fn placeholder_text(placeholder: RedactionPlaceholder) -> &'static str {
    match placeholder {
        RedactionPlaceholder::Email => EMAIL,
        RedactionPlaceholder::Phone => PHONE,
        RedactionPlaceholder::Person => PERSON,
        RedactionPlaceholder::Address => ADDRESS,
        RedactionPlaceholder::Date => DATE,
        RedactionPlaceholder::AccountId => ACCOUNT_ID,
        RedactionPlaceholder::GovernmentId => GOVERNMENT_ID,
        RedactionPlaceholder::Url => URL,
        RedactionPlaceholder::NetworkId => NETWORK_ID,
    }
}

fn placeholder_from_evidence(column: &ColumnMetadata) -> &'static str {
    let Some(evidence) = column
        .privacy_evidence
        .iter()
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
        PrivacyFindingKind::PrivateAddress => ADDRESS,
        PrivacyFindingKind::PrivateDate => DATE,
        PrivacyFindingKind::AccountOrFinancialId => ACCOUNT_ID,
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
        PrivacyFindingKind::AccountOrFinancialId => 75,
        PrivacyFindingKind::PrivateDate => 70,
        PrivacyFindingKind::NetworkOrDeviceId => 65,
        PrivacyFindingKind::Url => 60,
        PrivacyFindingKind::MixedSensitiveText => 10,
    }
}
