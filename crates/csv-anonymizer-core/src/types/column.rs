use super::*;
use serde::{Deserialize, Serialize};
/// What `strategies::structured::transform_generic_string` keeps of a value.
///
/// Named once because two callers state it about the same transform: the detected
/// types that have no transformer of their own, and the Local AI fallback for a
/// pass-through type, which lands on the same function. See
/// [`DataType::pseudonymization_preserves_structure`].
pub(crate) const GENERIC_STRING_STRUCTURE_DISCLOSURE: &str = "the replacement is random text of roughly the original's length (within about 20%), so value length survives approximately";

/// The default is [`DataType::Unknown`], and it is the only variant that may be one.
///
/// A default is reached when nothing said what a column holds, so it has to be the
/// answer that claims nothing. Every other variant is a claim, and the ones that would
/// tempt a reader — `String`, `Enum` — are claims in the wrong direction: `Enum` is
/// pass-through, so a defaulted column would be described as deliberately kept
/// unchanged. `Unknown` takes the generic-string transform and states no structure it
/// preserves, which is the honest reading of a column nobody classified.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataType {
    Email,
    Uuid,
    Timestamp,
    NumericId,
    NumericValue,
    PostalCode,
    Address,
    IpAddress,
    Url,
    MacAddress,
    TaxId,
    Boolean,
    Currency,
    Percentage,
    CountryCode,
    Phone,
    FirstName,
    LastName,
    FullName,
    Enum,
    String,
    #[default]
    Unknown,
}

impl DataType {
    pub(crate) fn privacy_finding_kind_and_reason(
        self,
    ) -> Option<(PrivacyFindingKind, &'static str)> {
        match self {
            DataType::Email | DataType::Phone => Some((
                PrivacyFindingKind::Contact,
                "Column type indicates contact information.",
            )),
            DataType::FirstName | DataType::LastName | DataType::FullName => Some((
                PrivacyFindingKind::Person,
                "Column type indicates person names.",
            )),
            DataType::Address => Some((
                PrivacyFindingKind::PrivateAddress,
                "Column type indicates private address data.",
            )),
            DataType::PostalCode => Some((
                PrivacyFindingKind::AddressRegion,
                "Column type indicates postal address context.",
            )),
            DataType::TaxId => Some((
                PrivacyFindingKind::GovernmentId,
                "Column type indicates government or tax identifier data.",
            )),
            // Identifier-shaped, but nothing here says *what* it identifies. A
            // financial classification needs evidence: the `account_number` header
            // kind, or the IBAN or payment-card validator. Absent that, this is a
            // surrogate key.
            DataType::NumericId => Some((
                PrivacyFindingKind::RecordIdentifier,
                "Column type indicates identifier-shaped values; review context.",
            )),
            DataType::Uuid => Some((
                PrivacyFindingKind::RecordIdentifier,
                "Column type indicates persistent identifier-shaped values; what they identify is unknown.",
            )),
            DataType::IpAddress | DataType::MacAddress => Some((
                PrivacyFindingKind::NetworkOrDeviceId,
                "Column type indicates network or device identifiers.",
            )),
            DataType::Url => Some((PrivacyFindingKind::Url, "Column type indicates URLs.")),
            DataType::NumericValue
            | DataType::Timestamp
            | DataType::Boolean
            | DataType::Currency
            | DataType::Percentage
            | DataType::CountryCode
            | DataType::Enum
            | DataType::String
            | DataType::Unknown => None,
        }
    }

    pub(crate) fn report_identifier_class(self) -> Option<ReportIdentifierClass> {
        match self {
            DataType::Email
            | DataType::Phone
            | DataType::FullName
            | DataType::FirstName
            | DataType::LastName
            | DataType::TaxId
            | DataType::Address => Some(ReportIdentifierClass::Direct),
            DataType::Uuid
            | DataType::NumericId
            | DataType::PostalCode
            | DataType::IpAddress
            | DataType::Url
            | DataType::MacAddress
            | DataType::Timestamp
            | DataType::CountryCode => Some(ReportIdentifierClass::Quasi),
            DataType::NumericValue
            | DataType::Boolean
            | DataType::Currency
            | DataType::Percentage
            | DataType::Enum
            | DataType::String
            | DataType::Unknown => None,
        }
    }

    pub(crate) fn uses_default_pass_through(self) -> bool {
        matches!(
            self,
            DataType::CountryCode
                | DataType::Enum
                | DataType::Boolean
                | DataType::Currency
                | DataType::Percentage
        )
    }

    /// What a rule-based pseudonym for this type keeps of the original value, or
    /// `None` when it keeps nothing worth disclosing.
    ///
    /// Every transformer in `strategies` is format-preserving on purpose: a
    /// pseudonymized timestamp has to parse as a timestamp, a pseudonymized amount has
    /// to stay a number, or the output is unusable. This sentence is what the release
    /// report states about the part of the source that comes through intact.
    ///
    /// The sharpest case is `Timestamp`: `strategies::structured::transform_timestamp`
    /// splits the value at its ten-byte ISO date prefix and concatenates the remainder
    /// verbatim, so `2024-06-15 10:30:45.123450` keeps `10:30:45.123450` exactly. In an
    /// event log a microsecond time-of-day is very nearly a primary key, so the
    /// "anonymized" file joins straight back to the source on a column the report
    /// marked verified. The date moves by at most 365 days, which also means a date of
    /// birth keeps its year to within one and the subject keeps their age.
    ///
    /// Returning a sentence rather than a bool because the sentence is the point: a
    /// caller cannot write one wording that is true of "keeps the domain after @" and
    /// of "keeps the digit count and sign" at once.
    ///
    /// No wildcard arm. A data type added to the enum has to be classified here rather
    /// than defaulting into the silent half — silence is the defect this closes.
    pub(crate) fn pseudonymization_preserves_structure(self) -> Option<&'static str> {
        match self {
            DataType::Email => Some(
                "the local part is replaced but the domain after @ is kept verbatim, so recipients of a rare or personal domain stay identifiable",
            ),
            DataType::Timestamp => Some(
                "the time of day is kept exactly — including sub-second digits, which are close to unique per event — and only the date moves, by at most a year, so an age or a year of birth survives",
            ),
            DataType::Phone => Some(
                "only the digits are redrawn: the digit count, country prefix punctuation and separator layout are kept",
            ),
            DataType::NumericId => Some(
                "the digit count and any leading zeros are kept, so the magnitude of the identifier is preserved",
            ),
            DataType::NumericValue => Some(
                "the sign, digit count and number of decimal places are kept, so the magnitude of the value is preserved",
            ),
            DataType::FirstName | DataType::LastName | DataType::FullName => Some(
                "replacements are drawn from a fixed name pool and the number of name parts is kept",
            ),
            // Generic-string pseudonymization draws a random value of 80–120% of the
            // original's length, so the length survives approximately. Everything that
            // is not handled by a transformer of its own lands here.
            DataType::Address
            | DataType::PostalCode
            | DataType::IpAddress
            | DataType::Url
            | DataType::MacAddress
            | DataType::TaxId
            | DataType::String
            | DataType::Unknown => Some(GENERIC_STRING_STRUCTURE_DISCLOSURE),
            // A UUID is machine-generated and carries no structure about its subject;
            // the transform keeps only the UUID format and the original's letter case,
            // neither of which narrows anyone down.
            DataType::Uuid => None,
            // Returned unchanged under Auto and Pseudonymize, so there is no transform
            // to describe — `uses_default_pass_through` is what the report says about
            // these, and it says it plainly. Under a rejected Local AI candidate they
            // take the generic-string path instead; the Local AI column report states
            // that separately, because it is a property of the strategy rather than of
            // the type.
            DataType::Enum
            | DataType::CountryCode
            | DataType::Boolean
            | DataType::Currency
            | DataType::Percentage => None,
        }
    }

    pub(crate) fn transforms_generated_quick_value(self) -> bool {
        matches!(
            self,
            DataType::Email
                | DataType::Uuid
                | DataType::Timestamp
                | DataType::NumericId
                | DataType::NumericValue
                | DataType::Phone
                | DataType::FirstName
                | DataType::LastName
                | DataType::FullName
                | DataType::String
                | DataType::Unknown
        )
    }

    pub(crate) fn redaction_changes_structured_scalar_type(self) -> bool {
        matches!(
            self,
            DataType::NumericId
                | DataType::NumericValue
                | DataType::Boolean
                | DataType::Currency
                | DataType::Percentage
        )
    }

    pub(crate) fn redaction_placeholder(self) -> Option<RedactionPlaceholder> {
        match self {
            DataType::Email => Some(RedactionPlaceholder::Email),
            DataType::Phone => Some(RedactionPlaceholder::Phone),
            DataType::FirstName | DataType::LastName | DataType::FullName => {
                Some(RedactionPlaceholder::Person)
            }
            DataType::Address => Some(RedactionPlaceholder::Address),
            DataType::TaxId => Some(RedactionPlaceholder::GovernmentId),
            DataType::Url => Some(RedactionPlaceholder::Url),
            DataType::IpAddress | DataType::MacAddress => Some(RedactionPlaceholder::NetworkId),
            DataType::String
            | DataType::Unknown
            | DataType::Enum
            | DataType::Uuid
            | DataType::Timestamp
            | DataType::NumericId
            | DataType::PostalCode
            | DataType::NumericValue
            | DataType::Boolean
            | DataType::Currency
            | DataType::Percentage
            | DataType::CountryCode => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReportIdentifierClass {
    Direct,
    Quasi,
}

/// Placeholders a column's *detected type* can justify on its own.
///
/// Deliberately has no account variant. "This is a bank account" is a claim about
/// what a value means, not about its shape, so it can only come from evidence — the
/// IBAN or card validator, or an `account_number` header — which reaches
/// `[ACCOUNT_ID]` through `placeholder_from_evidence` instead. A column of plain
/// integers gets a non-linkable placeholder derived from its column header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedactionPlaceholder {
    Email,
    Phone,
    Person,
    Address,
    GovernmentId,
    Url,
    NetworkId,
}

/// The default is [`Confidence::Low`], the only reading that asserts nothing.
///
/// `is_actionable` is false at Low, so a detection nobody measured cannot act as
/// evidence. Defaulting to High or Medium would let an unmeasured column carry the
/// weight of a measured one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    High,
    Medium,
    #[default]
    Low,
}

/// How much a finding exposes, most severe first.
///
/// Declaration order is load-bearing: `Ord` is derived, so `High < Medium < Low` and sorting
/// ascending puts the most severe finding first. The privacy report relies on that when it
/// breaks ties between findings — a report that shows a Medium above a High has under-sold
/// what it found, which is the one direction these figures may not be wrong in.
///
/// The default is [`PiiRisk::High`], deliberately the most severe rather than the most
/// common. A default stands in for "nobody assessed this column", and the only reading
/// of that which cannot make a file look safer than it is, is the worst case. `Low`
/// would silently un-flag a column: `is_elevated` is what auto-selects a column,
/// defaults it to Redact, and names it when it is released unchanged, and a defaulted
/// `Low` turns all three off without anything having decided so.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PiiRisk {
    #[default]
    High,
    Medium,
    Low,
}

impl PiiRisk {
    /// Whether this risk is one the app acts on: auto-selects the column, defaults it
    /// to Redact, and names it when it is released unchanged.
    ///
    /// Named once because it is the app's privacy threshold rather than a comparison.
    /// Spelled out at each site, the five copies could be changed apart, and a site
    /// left behind would go on treating a Medium-risk column as ordinary — selecting
    /// it out of the run, or leaving it out of the report that says what the released
    /// file still exposes.
    pub(crate) fn is_elevated(self) -> bool {
        match self {
            Self::High | Self::Medium => true,
            Self::Low => false,
        }
    }
}

/// The categories of privacy evidence the report can attribute to a column.
///
/// Declaration order is load-bearing, but it is the *last* word rather than the first: the
/// privacy report breaks a tie on score and match count by `PiiRisk` first, so a High finding
/// always leads a Medium one, and only then by kind, which on a derived enum is declaration
/// order. Reordering these variants therefore changes which of two equally-risky findings a
/// reader sees first, so treat the order as user-facing output rather than as a free choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrivacyFindingKind {
    Person,
    Contact,
    PrivateAddress,
    /// An address component that narrows someone down to an area rather than to a
    /// doorstep: a postal code on its own.
    ///
    /// Separate from [`PrivacyFindingKind::PrivateAddress`] because a street address
    /// locates a person and a postal code locates a neighbourhood. Both matter, but
    /// only one is a direct identifier, and reporting a zip column as a private
    /// address overstated what the file actually contains.
    AddressRegion,
    PrivateDate,
    AccountOrFinancialId,
    /// A key that identifies a row without being sensitive in itself: an order
    /// number, a record id, a customer sequence number.
    ///
    /// Distinct from [`PrivacyFindingKind::AccountOrFinancialId`], which is for
    /// bank accounts, cards and IBANs. Both are identifiers, but only one exposes
    /// a payment instrument, and collapsing them made every column of order
    /// numbers report as financial data. A surrogate key still re-identifies a
    /// row, so it is Medium rather than Low.
    RecordIdentifier,
    GovernmentId,
    CredentialOrSecret,
    NetworkOrDeviceId,
    Url,
    MixedSensitiveText,
}

/// The default is [`EmptyFormat::EmptyString`], which is also what
/// [`crate::detection::detect_empty_format`] answers for a column in which nothing
/// null-shaped was seen. It carries no privacy claim either way — it decides how a blank
/// cell is written, not what survives in it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmptyFormat {
    #[default]
    EmptyString,
    Null,
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    pub data_type: DataType,
    pub confidence: Confidence,
    pub sample_matches: usize,
    pub total_samples: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<DetectionTrace>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionTrace {
    pub summary: String,
    pub selected_reason: String,
    pub total_non_empty: usize,
    pub candidates: Vec<DetectionTraceItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionTraceItem {
    pub data_type: DataType,
    pub reason: String,
    pub match_count: usize,
    pub total_considered: usize,
    pub confidence: Confidence,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyFinding {
    pub kind: PrivacyFindingKind,
    pub data_type: DataType,
    pub row_index: usize,
    pub start: usize,
    pub end: usize,
    pub match_value: String,
    pub sample_value: String,
    pub confidence: Confidence,
    pub score: u8,
    pub detector: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyEvidenceSummary {
    pub kind: PrivacyFindingKind,
    pub data_type: DataType,
    pub confidence: Confidence,
    pub match_count: usize,
    pub sample_count: usize,
    pub score: u8,
    #[serde(default)]
    pub detector: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detectors: Vec<String>,
}

impl PrivacyEvidenceSummary {
    /// Whether the app may act on this finding.
    ///
    /// A Low-confidence finding is a shape that resembles the thing more often than
    /// it is the thing — `pattern:phone-digits` fires on any bare digit run, and most
    /// bare digit runs are order numbers. Such a finding is worth *recording*, so a
    /// reviewer can see it and so free text still gets redacted span by span, but it
    /// is not worth *asserting*: it may not raise a column's risk and it may not put
    /// a specific type's name in the output.
    ///
    /// This lives on the summary because two modules decide it and they have to agree.
    /// `analyze_column_privacy` folds risk over the evidence; `placeholder_from_evidence`
    /// names the redaction placeholder from the same list. When only the risk fold
    /// filtered, a column the risk model had explicitly declined to trust still
    /// redacted to `[PHONE]` — the output file asserting a phone number was there on
    /// the evidence the app had just rejected. One predicate, so the next change to
    /// the threshold cannot reach one consumer and miss the other.
    pub(crate) fn is_actionable(&self) -> bool {
        self.confidence != Confidence::Low
    }
}

/// `Default` is for constructing one, not for accepting one.
///
/// Fifteen fields of which a given caller varies two or three is what
/// `crate::test_support::column` exists to absorb, and `Default` is what lets it. It
/// widens nothing on the deserialization side: the serde contract is unchanged, so a
/// field this struct requires on the wire is still required — only the fields already
/// carrying `#[serde(default)]`, each with its own reason stated below, may be absent.
///
/// Every defaulted field is the least-privileged reading of "nobody decided this":
/// `PiiRisk::High`, `DataType::Unknown`, `Confidence::Low`,
/// `AnonymizationStrategy::PassThrough`, `is_selected: false`, and a zeroed
/// distribution. A default may leave a column looking more exposed than it is; it may
/// never leave one looking safer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColumnReviewReason {
    DetectorsDisagree,
    LocalNerLowConfidence,
    AmbiguousContext,
    InsufficientSample,
}

/// How conclusive the analysis of a column was, independently of its privacy risk.
///
/// Risk answers "what would this expose if the finding is correct?" while this
/// disposition answers "did the available evidence justify a conclusion?" Keeping
/// them separate prevents an unrecognised string column from being presented as
/// tested-benign merely because no detector raised its risk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceDisposition {
    DetectedSensitive,
    TestedBenign,
    #[default]
    Uncertain,
    AnalysisIncomplete,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatEvidence {
    pub data_type: DataType,
    pub confidence: Confidence,
    pub match_count: usize,
    pub sample_count: usize,
    pub basis: FormatEvidenceBasis,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub detectors: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FormatEvidenceBasis {
    DetectionSample,
    UserOverride,
    #[default]
    RetainedPreviewValues,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticSpecificity {
    Specific,
    #[default]
    Generic,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SemanticStatus {
    Resolved,
    #[default]
    Uncertain,
    Conflicting,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticDecision {
    pub kind: String,
    pub confidence: Confidence,
    pub specificity: SemanticSpecificity,
    pub status: SemanticStatus,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supporting_evidence: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conflicting_evidence: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyDecision {
    pub risk: PiiRisk,
    pub recommended_strategy: AnonymizationStrategy,
    pub auto_selected: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RedactionPlaceholderSource {
    Typed,
    ColumnHeader,
    #[default]
    Generic,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactionDecision {
    pub placeholder: String,
    pub source: RedactionPlaceholderSource,
    pub is_typed: bool,
    pub preserves_equality: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnEvidenceProfile {
    pub format_evidence: FormatEvidence,
    pub semantic_decision: SemanticDecision,
    pub privacy_decision: PrivacyDecision,
    pub redaction_decision: RedactionDecision,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetadata {
    pub name: String,
    /// Whether some other column's header reduces to the same placeholder label as
    /// this one's.
    ///
    /// A duplicate header is legal in CSV, and on its own it is only untidy — the
    /// column table shows two rows with one name and the index tells them apart.
    /// It stops being cosmetic once a label reaches a *cell*: labels number each
    /// distinct value per column, so two columns named `notes` would both start at
    /// `[NOTES_1]`, and a reader comparing those cells would conclude the source
    /// values were equal when nothing of the kind was measured. Columns marked here
    /// fold their position into the label instead.
    ///
    /// Measured on the label rather than the raw header, because that is where the
    /// collision happens: `Notes`, `notes` and `notes!` all reduce to `NOTES`.
    ///
    /// Defaulted rather than optional so a caller that never compared headers gets
    /// the unqualified label, which is the behaviour for a column whose header is
    /// unique — the common case, and the readable one.
    #[serde(default)]
    pub header_label_is_ambiguous: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub index: usize,
    pub detected_type: DataType,
    pub confidence: Confidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection_trace: Option<DetectionTrace>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub privacy_findings: Vec<PrivacyFinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub privacy_evidence: Vec<PrivacyEvidenceSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_reasons: Vec<ColumnReviewReason>,
    #[serde(default)]
    pub evidence_disposition: EvidenceDisposition,
    #[serde(default)]
    pub evidence_profile: ColumnEvidenceProfile,
    pub pii_risk: PiiRisk,
    pub sample_values: Vec<String>,
    /// The value distribution of the *detection sample*, not of the whole input.
    ///
    /// This is what lets the column table warn before a run rather than after one,
    /// which is the only point at which the user can still change strategy. A
    /// sampled distinct count is a lower bound on the true one, which is the right
    /// direction to be wrong in: a high-cardinality column cannot be made to look
    /// low-cardinality by sampling, while a genuinely low-cardinality one saturates
    /// within a few dozen rows.
    ///
    /// Defaulted rather than optional so a caller that never measured produces a
    /// zeroed distribution, which fails the sample-size floor and therefore cannot
    /// raise a warning it has no evidence for.
    #[serde(default)]
    pub sample_value_distribution: ColumnValueDistribution,
    pub empty_format: EmptyFormat,
    pub is_selected: bool,
    pub strategy: AnonymizationStrategy,
}

/// The default is [`AnonymizationStrategy::PassThrough`], which is the variant that
/// claims the least rather than the one a user most often ends on.
///
/// A default stands in for "no strategy was chosen", and the report has to read that
/// as "nothing was done to this column". `PassThrough` does exactly that: the release
/// report calls it Review and says the values are kept unchanged, and
/// `uniqueness::LinkableProjection::for_column` treats it as `WholeValue`, the most
/// linkable projection there is. Every other variant would be a claim in the opposite
/// direction — `Redact` is reported Verified, so a defaulted column would carry a green
/// tick nobody earned for it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnonymizationStrategy {
    Auto,
    Pseudonymize,
    Tokenize,
    LocalAi,
    Mask,
    Label,
    Redact,
    #[default]
    PassThrough,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnControl {
    pub column_index: usize,
    pub type_override: Option<DataType>,
    pub strategy: AnonymizationStrategy,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LocalNerRunStatus {
    #[default]
    Disabled,
    Completed,
    Unavailable,
    Failed,
    Incomplete,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeterministicDetectionStatus {
    #[default]
    Completed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectionReviewReason {
    DetectorFailed,
    CandidateRejected,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionRunSummary {
    pub deterministic: DeterministicDetectionStatus,
    pub local_ner: LocalNerRunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    #[serde(default)]
    pub examined_cells: usize,
    #[serde(default)]
    pub total_eligible_cells: usize,
    #[serde(default)]
    pub skipped_oversized_cells: usize,
    #[serde(default)]
    pub accepted_candidates: usize,
    #[serde(default)]
    pub rejected_candidates: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review_reasons: Vec<DetectionReviewReason>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}
