use crate::types::{ColumnMetadata, DataType, PrivacyFindingKind};

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
    match column.detected_type {
        DataType::Email => EMAIL,
        DataType::Phone => PHONE,
        DataType::FirstName | DataType::LastName | DataType::FullName => PERSON,
        DataType::Address | DataType::PostalCode => ADDRESS,
        DataType::Timestamp => DATE,
        DataType::NumericId | DataType::Uuid => ACCOUNT_ID,
        DataType::TaxId => GOVERNMENT_ID,
        DataType::Url => URL,
        DataType::IpAddress | DataType::MacAddress => NETWORK_ID,
        DataType::String | DataType::Unknown | DataType::Enum => placeholder_from_evidence(column),
        DataType::NumericValue
        | DataType::Boolean
        | DataType::Currency
        | DataType::Percentage
        | DataType::CountryCode => placeholder_from_evidence(column),
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
