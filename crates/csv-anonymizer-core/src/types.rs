use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The largest detection basis any entry point accepts, for any input kind.
///
/// One limit rather than one per workflow, because the figure comes from one
/// setting. "Sample rows" is not per-workflow: a user who raises it to work on a
/// large CSV file has raised it for the paste workflow too, so a value that reaches
/// the setting has to be a value every entry point will honour.
///
/// Enforced twice, and both sites read this: `settings::sanitize_settings` clamps
/// what can be stored, and the paste entry points reject an oversized request
/// outright, since they are reachable by callers that never went through settings.
pub const MAX_SAMPLE_ROW_COUNT: usize = 10_000;

/// The largest display window any entry point accepts, for any input kind. One
/// limit for the same reason as [`MAX_SAMPLE_ROW_COUNT`] — it comes from the
/// "Preview rows" setting, which is likewise not per-workflow.
pub const MAX_PREVIEW_SAMPLE_COUNT: usize = 100;

/// The smallest detection basis any entry point classifies on, for any input kind.
///
/// A floor rather than a default: `service::detection_sample_rows` and
/// `direct_input::shared::paste_detection_sample_rows` both raise a caller's "Sample
/// rows" to at least this, so the setting can only ask for more evidence than the
/// default, never less.
///
/// One constant rather than one per workflow, because the file and paste workflows
/// promise the user that they classify on the same basis. Two literals held that
/// promise by coincidence: changing either alone left a pasted CSV and the same file
/// on disk detecting different types, and so being offered different strategies, with
/// nothing failing to say so.
pub(crate) const DETECTION_SAMPLE_ROW_FLOOR: usize = 100;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedSample {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Data rows read while building the sample. This is the input's full
    /// data-row count only when `scanned_entire_input` is true; it is always
    /// >= `rows.len()`, because a spread sample thins what it keeps.
    pub data_rows_scanned: usize,
    /// Whether every data row was read. False only for a head window that hit
    /// its cap; detection samples always scan the whole input.
    pub scanned_entire_input: bool,
}

/// What the two figures in a detection-coverage disclosure count.
///
/// The unit is carried rather than assumed because "rows" is only true for the two
/// tabular entry points. A field-based paste — JSON, YAML, XML, or free text scanned
/// for privacy spans — has no rows to sample: `direct_input::shared::detection_coverage`
/// counts the values of the busiest field, and `PasteAnalyzeData::row_count` is derived
/// separately and can disagree outright. A pasted `{"users": [500 objects]}` shows one
/// row in the UI, and free text shows a match count that is neither its line count nor
/// its row count, so a disclosure hard-coded to "rows" states a figure the user cannot
/// find anywhere on screen and cannot check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectionCoverageUnit {
    Rows,
    Values,
}

impl DetectionCoverageUnit {
    /// The noun a disclosure sentence uses for these figures.
    pub(crate) fn plural_noun(self) -> &'static str {
        match self {
            Self::Rows => "rows",
            Self::Values => "values",
        }
    }
}

/// How much of an input detection actually classified.
///
/// Detection votes on a bounded sample, so a value occurring in few rows can be
/// missed — and a column whose sensitive values were all missed is never
/// auto-selected, so it is written unchanged. That is a deliberate trade for
/// bounded memory on inputs of any size, but it is only an honest one if the user
/// is told the verdict rests on a sample. This carries the figures needed to say so
/// and the unit they are counted in.
///
/// Not a DTO. It is `pub(crate)` on purpose: [`Self::new`] is the only place the
/// `examined <= total` invariant is established, and a `Deserialize` impl would let
/// a wire value walk straight past it and report a sample larger than the input it
/// was drawn from. What crosses the IPC boundary is [`DetectionCoverageSummary`],
/// a flat snapshot taken after clamping, plus the report notes and preflight review
/// item built from this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DetectionCoverage {
    /// Rows or values kept for classification.
    examined: usize,
    /// Rows or values the input holds.
    total: usize,
    unit: DetectionCoverageUnit,
}

impl DetectionCoverage {
    /// Coverage of the sample a detection pass actually kept.
    ///
    /// Read off the sample rather than recomputed from the requested row count,
    /// because the request is a ceiling and the sample is what happened: a file with
    /// fewer rows than the request is fully covered, and asking the request would
    /// claim otherwise.
    pub(crate) fn from_detection_sample(sample: &ParsedSample) -> Self {
        Self::rows(sample.rows.len(), sample.data_rows_scanned)
    }

    /// Coverage for an input that has nothing to sample.
    ///
    /// Both figures are zero, which is not a missing figure but the only truthful
    /// pair here: quick-generate has no source input, so no row went unexamined and
    /// none was examined either. Only [`Self::is_partial`] is consulted for such an
    /// input, and it answers false — the disclosure stays silent, which is right,
    /// because there is no sampling to disclose. Nothing may print
    /// [`Self::examined`] and [`Self::total`] without gating on
    /// [`Self::is_partial`] first, or this constructor renders as "0 of 0".
    pub(crate) fn complete() -> Self {
        Self::rows(0, 0)
    }

    /// Coverage counted in data rows: the two tabular entry points, CSV file and
    /// pasted CSV text.
    pub(crate) fn rows(examined: usize, total: usize) -> Self {
        Self::new(examined, total, DetectionCoverageUnit::Rows)
    }

    /// Coverage counted in field values: JSON, YAML, XML and free-text pastes, which
    /// have fields rather than rows. See [`DetectionCoverageUnit`].
    pub(crate) fn values(examined: usize, total: usize) -> Self {
        Self::new(examined, total, DetectionCoverageUnit::Values)
    }

    fn new(examined: usize, total: usize, unit: DetectionCoverageUnit) -> Self {
        Self {
            examined: examined.min(total),
            total,
            unit,
        }
    }

    /// Whether some of the input went unclassified.
    pub(crate) fn is_partial(self) -> bool {
        self.examined < self.total
    }

    pub(crate) fn examined(self) -> usize {
        self.examined
    }

    pub(crate) fn total(self) -> usize {
        self.total
    }

    pub(crate) fn unit(self) -> DetectionCoverageUnit {
        self.unit
    }

    /// The IPC-facing snapshot of this coverage.
    pub(crate) fn summary(self) -> DetectionCoverageSummary {
        DetectionCoverageSummary {
            examined: self.examined,
            total: self.total,
            unit: self.unit,
            is_partial: self.is_partial(),
        }
    }
}

/// What detection classified, as the paste analyze result reports it.
///
/// This exists so a paste user learns that detection sampled before choosing
/// columns, not after the output already exists. The file workflow gets the same
/// disclosure from preflight; the paste workflow has no preflight, so until this
/// crossed the boundary the only place it appeared was the post-transform privacy
/// report — advice ("raise Sample rows") that by then costs a whole second run.
///
/// Counts only. No value, column name or excerpt goes on the wire here, so the
/// disclosure cannot itself leak what detection missed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionCoverageSummary {
    pub examined: usize,
    pub total: usize,
    pub unit: DetectionCoverageUnit,
    /// `examined < total`, decided here rather than in the client.
    ///
    /// The comparison is trivial, which is exactly why it would drift: a client that
    /// re-derives it is free to write `<=` or to compare against a row count from a
    /// different field, and either mistake silences the warning rather than
    /// producing a visible error.
    pub is_partial: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProcessOptions<'a> {
    pub smart_replacements: Option<&'a crate::smart::SmartReplacementMap>,
    /// Mapping entries this run may hold before it refuses to continue, or `None` for
    /// `TransformState::MAPPING_ENTRY_CEILING`.
    ///
    /// A run option rather than a constant read at the point of use because the
    /// ceiling is a resource limit, and a resource limit that cannot be set to a
    /// reachable value cannot be tested: the real one stands for about 5 GB of
    /// mapping, so the only way to show that the run loop *consults* it — and that
    /// refusing leaves no partial output — is to hand the loop a smaller one.
    pub mapping_entry_ceiling: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessProgress {
    pub rows_processed: usize,
}

pub struct ProcessControl<'a> {
    pub on_progress: Option<&'a mut dyn FnMut(ProcessProgress)>,
    pub should_cancel: Option<&'a dyn Fn() -> bool>,
}

impl ProcessControl<'_> {
    pub fn none() -> Self {
        Self {
            on_progress: None,
            should_cancel: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResult {
    pub row_count: usize,
    pub output_path: PathBuf,
    pub duration_ms: u128,
    pub transform_report: TransformReport,
}

/// What one column's consistent pseudonyms reveal about the values behind them.
///
/// A consistent pseudonym preserves equality, which is why the strategies that
/// produce one keep a dataset joinable. The cost is that equality also preserves
/// the *shape* of the value distribution, and the shape is enough to work against
/// the mapping:
///
/// - Few `distinct_values` over many `total_values` means the mapping can be
///   relabelled by frequency, by anyone who knows how the real field is
///   distributed. The tokens stay opaque; the histogram does not.
/// - A `singleton_values` entry is a pseudonym covering exactly one row, which
///   singles that record out however unguessable the token looks.
/// - `max_value_occurrences` is the most common value's row count, and it is the
///   sharpest of the four for frequency inversion — the dominant value is the
///   attacker's easiest anchor, and inverting that one pseudonym recovers
///   `max_value_occurrences / total_values` of the column in a single step. Acted on
///   as a *share* rather than a count, because a count means something different in a
///   200-row file than in a 5-million-row one. The threshold it is compared against is
///   `MIN_INVERTIBLE_DOMINANT_SHARE`, which is private, so this names it rather than
///   linking to it.
///
/// Reported per column because the risk is per column: one low-cardinality column
/// is not made safer by a high-cardinality neighbour.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnValueDistribution {
    pub column_index: usize,
    pub distinct_values: usize,
    pub total_values: usize,
    pub singleton_values: usize,
    /// Values seen exactly twice. Carried for the same reason as the singleton
    /// count: the two together are what let a *sampled* distribution say anything
    /// about the input it was drawn from — see `estimated_distinct_values`, which is
    /// private, so this names it rather than linking to it.
    #[serde(default)]
    pub doubleton_values: usize,
    pub max_value_occurrences: usize,
}

/// Minimum values before the cardinality test means anything.
///
/// A floor is not optional. `distinct_values <= total_values`, so any
/// `distinct_values < K` test is *vacuously true* whenever `total_values < K` — on a
/// 5-row fixture every column looks low-cardinality, including the unique ID
/// columns. Measured on `tests/fixtures/large.csv` truncated to varying lengths: at
/// 5 rows all 7 columns trip the test, including `id`, `email` and `user_uuid`; the
/// mis-flags disappear at 10 rows, i.e. at exactly K.
///
/// 50 rather than the minimum 10 because the margin matters: at 20 rows the
/// highest-cardinality realistic column (`name`) shows 19 distinct values, one step
/// from the boundary, while at 50 rows it shows 42 — a margin of 32. It also
/// silences every short test fixture, which is where all the noise was.
const CARDINALITY_FLOOR: usize = 50;

/// Distinct values below which a mapping is treated as frequency-invertible.
///
/// The two genuinely low-cardinality columns in the corpus saturate at 8 (`country`)
/// and 4 (`status`); the next column up saturates at 100 (`name`). The data pins this
/// constant only to the interval (8, 100] — the decade between is unpopulated — so 10
/// is the conservative end of the measured interval, and it agrees with the common
/// convention that fewer than about ten buckets makes a frequency table trivially
/// labellable.
const MAX_INVERTIBLE_DISTINCT_VALUES: usize = 10;

/// Distinct-to-total ratio below which a mapping is treated as frequency-invertible,
/// for columns too large for the absolute test to catch.
///
/// Reads as "each pseudonym covers more than twenty rows on average". It exists for
/// the case the absolute test misses: `large.csv name` has 100 distinct values over
/// 10500 rows, so `distinct < 10` is false while the ratio is 0.0095.
///
/// 0.05 rather than a looser 0.20 by a subsumption argument rather than measurement.
/// `distinct/total < 0.05` is already implied by `distinct < 10` for any total below
/// 200, so the ratio term is inert on small and medium inputs and the verdict there
/// rests entirely on the stable statistic — the absolute count, which saturates. The
/// ratio only takes over once there are enough rows for it to have converged. A 0.20
/// threshold would instead activate immediately above the floor and flag, say, 2000
/// distinct names in a 10500-row file, which is a five-row group.
const MAX_INVERTIBLE_DISTINCT_RATIO: f64 = 0.05;

/// Sample coverage below which a *sampled* distribution is treated as saying nothing
/// about the input's distinct count, so the ratio test is skipped. Good–Turing coverage,
/// `1 - singletons/values`; the measurements pin it only to the interval (0.40, 0.87] and
/// 0.75 sits inside with margin at both ends. Not measured on real production data,
/// non-Zipf skew, or columns sitting near the gate itself.
/// See docs/calibration.md#min_sample_coverage for the measurements behind this.
const MIN_SAMPLE_COVERAGE: f64 = 0.75;

/// Share of a column's values carried by its single most common value, at or above which
/// the mapping is treated as frequency-invertible. Catches the shape neither other term
/// sees: thousands of distinct values, one of them covering most of the rows. A share and
/// not a count, so it means the same in a 200-row file and a 5-million-row one. The
/// measurements pin it to [1/3, 0.35]; not measured on real production columns.
/// See docs/calibration.md#min_invertible_dominant_share for the measurements behind this.
const MIN_INVERTIBLE_DOMINANT_SHARE: f64 = 1.0 / 3.0;

/// Which of the three tests judged a distribution frequency-invertible.
///
/// Exists so a warning can name the evidence it actually has. The three terms catch
/// genuinely different shapes — a handful of values, one value dominating a diverse
/// column, and many small groups across a large one — and a single wording cannot
/// describe all three without describing at least two of them wrongly. Reporting a
/// column of 101 values where one covers half the rows as holding "only 101 distinct
/// value(s)" would be true, would read as reassuring, and would name a risk the column
/// does not have while staying silent about the one it does.
///
/// Each variant carries the figure its wording needs, computed at the point the test
/// fires, because [`ColumnValueDistribution::estimated_distinct_values`] and
/// [`ColumnValueDistribution::dominant_value_share`] are private and a message
/// builder outside this module cannot recover them from the public fields.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FrequencyInversionRisk {
    /// Few enough distinct values that the whole frequency table is labellable.
    FewDistinctValues,
    /// One value carries [`MIN_INVERTIBLE_DOMINANT_SHARE`] or more of the column, so
    /// inverting that single pseudonym recovers `share` of it.
    DominantValue { share: f64 },
    /// Enough rows per distinct value that the groups can be matched by size, even
    /// though no single value dominates and there are too many to enumerate.
    LargeGroups { estimated_distinct_values: usize },
}

impl ColumnValueDistribution {
    /// Builds the distribution of `values`, skipping the ones a transform would skip.
    ///
    /// Uses [`crate::detection::is_empty_value`] and [`crate::smart::value_identity_key`]
    /// so that a distribution measured over a detection sample is comparable with one
    /// accumulated during a run: the transform returns early on empty values and folds
    /// case and padding, so counting either differently here would make the pre-run
    /// warning disagree with the post-run report on the same data.
    pub(crate) fn from_values(column_index: usize, values: &[String]) -> Self {
        let mut occurrences: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut total_values = 0usize;
        for value in values {
            let trimmed = value.trim();
            if crate::detection::is_empty_value(trimmed) {
                continue;
            }
            total_values += 1;
            *occurrences
                .entry(crate::smart::value_identity_key(trimmed))
                .or_insert(0) += 1;
        }

        Self {
            column_index,
            distinct_values: occurrences.len(),
            total_values,
            singleton_values: occurrences.values().filter(|count| **count == 1).count(),
            doubleton_values: occurrences.values().filter(|count| **count == 2).count(),
            max_value_occurrences: occurrences.values().copied().max().unwrap_or(0),
        }
    }

    /// Whether consistent pseudonyms over this distribution could be relabelled by
    /// frequency analysis.
    ///
    /// Singleton counts deliberately play no part. A singleton rule reads as the
    /// record-isolation risk and sounds right, but measured against the corpus it
    /// flags every unique-key column — `id`, `email`, `user_uuid`, `created_at` —
    /// which are exactly the columns users pseudonymize as a matter of course. That
    /// isolation is a property of the input being unique already, not something
    /// consistent pseudonymization introduces, and `distinct == total` says it more
    /// directly. A warning that fires on every email column is noise.
    ///
    /// See [`CARDINALITY_FLOOR`], [`MAX_INVERTIBLE_DISTINCT_VALUES`],
    /// [`MIN_INVERTIBLE_DOMINANT_SHARE`], [`MAX_INVERTIBLE_DISTINCT_RATIO`] and
    /// [`MIN_SAMPLE_COVERAGE`] for what each constant is measured against.
    ///
    /// For a distribution measured over the whole column. A sampled one has to say
    /// how large the column really is — see [`Self::frequency_inversion_risk_in`].
    pub(crate) fn risks_frequency_inversion(&self) -> bool {
        self.frequency_inversion_risk().is_some()
    }

    /// [`Self::risks_frequency_inversion`], with the reason it answered yes.
    pub(crate) fn frequency_inversion_risk(&self) -> Option<FrequencyInversionRisk> {
        self.frequency_inversion_risk_in(self.total_values)
    }

    /// As [`Self::risks_frequency_inversion`], for a distribution measured over a
    /// sample of a column that holds `population_values` values in total.
    ///
    /// The distinction is not pedantic — getting it wrong is what made the pre-run
    /// warning miss the case it exists for. The ratio test divides distinct values by
    /// the column's size, and a sample's size is the *sample's*, capped at a hundred
    /// or so. Measured against that cap, `distinct / total < 0.05` needs fewer than
    /// five distinct values in a hundred, which the absolute test has already caught:
    /// the ratio term was inert on every sampled distribution while being the only
    /// term that catches a column of thirty values in five thousand rows. Such a
    /// column drew no warning until after its output had been written.
    ///
    /// Dividing by the true row count instead is wrong in the other direction, and
    /// worse: a hundred sampled values cannot look like a million distinct ones, so a
    /// fully unique column would score 0.005 and be flagged as trivially invertible.
    /// [`MIN_SAMPLE_COVERAGE`] is the gate that separates the two, and
    /// [`Self::estimated_distinct_values`] is what gets compared once it opens.
    ///
    /// `population_values` is the column's row count, an upper bound on how many
    /// values it holds — empty cells are not values. Erring high makes the ratio
    /// smaller and the warning likelier, which is the safe direction for a warning
    /// the user can dismiss by choosing another strategy.
    ///
    /// The dominant-value term needs no equivalent adjustment, and that is the reason it
    /// sits where it does. It compares two figures the distribution measured *itself* —
    /// a share of its own values — so `population_values` never enters. A share is also
    /// the one statistic a small sample can estimate here, unlike its distinct count,
    /// which is why only the distinct-count term needs a population figure and a gate.
    /// See docs/calibration.md#sample-share-vs-distinct-count for the measurements.
    ///
    /// The order of the three tests is the order of decreasing certainty, and it is
    /// also what each variant means: a later variant implies the earlier tests
    /// declined. So `LargeGroups` is only ever reported for a column with at least
    /// [`MAX_INVERTIBLE_DISTINCT_VALUES`] distinct values and no dominant one, which is
    /// what lets its wording talk about average group size without qualification.
    pub(crate) fn frequency_inversion_risk_in(
        &self,
        population_values: usize,
    ) -> Option<FrequencyInversionRisk> {
        if self.total_values < CARDINALITY_FLOOR {
            return None;
        }
        if self.distinct_values < MAX_INVERTIBLE_DISTINCT_VALUES {
            return Some(FrequencyInversionRisk::FewDistinctValues);
        }
        // Deliberately ahead of the coverage gate. The shape this catches — one dominant
        // value over a long tail of near-unique ones — is singleton-heavy and therefore
        // *low*-coverage, so behind the gate this term would be silent on exactly the
        // See docs/calibration.md#min_sample_coverage for the figures.
        let share = self.dominant_value_share();
        if share >= MIN_INVERTIBLE_DOMINANT_SHARE {
            return Some(FrequencyInversionRisk::DominantValue { share });
        }
        if self.sample_coverage() < MIN_SAMPLE_COVERAGE {
            return None;
        }
        let estimated_distinct_values = self.estimated_distinct_values();
        if (estimated_distinct_values as f64) / (population_values.max(1) as f64)
            < MAX_INVERTIBLE_DISTINCT_RATIO
        {
            return Some(FrequencyInversionRisk::LargeGroups {
                estimated_distinct_values,
            });
        }
        None
    }

    /// Share of this distribution's values carried by its single most common value.
    ///
    /// The fraction of the column that one inverted pseudonym recovers, so it is also
    /// the size of the leak the warning is reporting. `0.0` for a distribution that
    /// measured nothing, and `1 / total_values` for a fully unique column — which is
    /// below [`MIN_INVERTIBLE_DOMINANT_SHARE`] for any column past
    /// [`CARDINALITY_FLOOR`], so a unique column cannot trip the term that reads this.
    fn dominant_value_share(&self) -> f64 {
        if self.total_values == 0 {
            return 0.0;
        }
        (self.max_value_occurrences as f64) / (self.total_values as f64)
    }

    /// Good–Turing sample coverage: the estimated share of this column's values that
    /// belong to a group the distribution has already seen.
    ///
    /// Exactly `1.0` for a distribution measured over a whole column with no
    /// singletons, and `0.0` for one where every value was seen once — which is both
    /// a unique column measured exactly and a sample that has learned nothing.
    /// [`Self::frequency_inversion_risk_in`] is what distinguishes those, by only
    /// consulting coverage once the absolute test has declined.
    fn sample_coverage(&self) -> f64 {
        if self.total_values == 0 {
            return 0.0;
        }
        1.0 - (self.singleton_values as f64) / (self.total_values as f64)
    }

    /// Chao1: the distinct count this distribution implies for the whole column,
    /// including groups it did not see.
    ///
    /// `distinct + f1² / 2·f2` over singletons and doubletons — the standard
    /// lower-bound estimator, on the reasoning that how many groups you saw *once*
    /// tells you how many you missed entirely. Returns the observed count unchanged
    /// for a distribution with no singletons, which is what a fully measured column
    /// with every value repeated looks like.
    ///
    /// A lower bound, so it under-estimates rather than over-estimates a diverse
    /// column, which is the wrong direction for a warning — hence
    /// [`MIN_SAMPLE_COVERAGE`] refusing the comparison before this is consulted.
    /// `pub(crate)` because the preflight memory projection needs the same estimate
    /// and a second copy of a statistical estimator is a copy that drifts.
    pub(crate) fn estimated_distinct_values(&self) -> usize {
        if self.doubleton_values > 0 {
            return self.distinct_values
                + self.singleton_values.saturating_pow(2) / (2 * self.doubleton_values);
        }
        // The bias-corrected form, for when nothing was seen exactly twice and the
        // ratio above would divide by zero.
        self.distinct_values
            + self
                .singleton_values
                .saturating_mul(self.singleton_values.saturating_sub(1))
                / 2
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformReport {
    pub unique_pseudonym_values: usize,
    pub reused_pseudonym_values: usize,
    pub collisions_avoided: usize,
    pub exhausted_pseudonym_pools: usize,
    pub opaque_token_values: usize,
    pub smart_replacement_requests: usize,
    pub smart_replacement_values: usize,
    pub smart_replacement_rejections: usize,
    pub smart_replacement_rejection_reasons: Vec<SmartReplacementRejectionCount>,
    pub smart_replacement_fallbacks: usize,
    pub shape_fallback_values: usize,
    pub column_value_distributions: Vec<ColumnValueDistribution>,
    pub row_uniqueness: Option<RowUniquenessSummary>,
}

/// How exposed the released rows are once every column is read together.
///
/// The rest of this crate's privacy figures are per column, so a file can be reported as
/// having no unselected high or medium risk column while postcode, birth date and job
/// title jointly single out a third of its rows. This is the figure that says so.
///
/// Measured over the columns an outsider could match against data they already hold —
/// see `crate::uniqueness::LinkableProjection` for how that subset is decided and what it
/// deliberately leaves out. Every count here is over that subset except
/// `distinct_rows_all_columns`.
///
/// Absent rather than zeroed where there are no rows: unstructured text and single pasted
/// values never populate it, and a summary claiming zero unique rows would read as a clean
/// result from a check that never ran.
///
/// A DTO, like [`DetectionCoverageSummary`] and unlike the private `DetectionCoverage`: a flat
/// snapshot with no invariant of its own to protect, so `Deserialize` costs nothing here.
/// The relationships between these figures — that the counts are over the columns named,
/// that a stopped measurement zeroes the rest — are established in
/// `crate::uniqueness::RowUniquenessTracker::summary`, which is the only thing that builds
/// one outside tests.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowUniquenessSummary {
    /// Data rows hashed. Blank rows are written through untransformed and are not counted.
    pub rows_measured: usize,
    /// The columns the measure read, each with what an outsider could match it on, in
    /// column order. Empty means nothing released is matchable — a statement about how the
    /// columns were transformed, not a finding that the data is anonymous.
    ///
    /// Only columns that actually yielded something are listed. A column whose projection
    /// came back empty on every row contributed nothing to any class and is named nowhere,
    /// so no claim here can rest on a column that in fact carried no signal.
    pub matched_columns: Vec<MatchedColumn>,
    /// Distinct equivalence classes over the subset.
    pub distinct_classes: usize,
    /// Rows alone in their class: someone holding those columns for a person finds
    /// exactly their row.
    pub unique_rows: usize,
    /// The k-anonymity floor — the smallest class present. One freak record sets it, which
    /// is why `fifth_percentile_class_size` is reported beside it.
    pub smallest_class: usize,
    /// The class size at or below which the most exposed 5% of rows sit.
    pub fifth_percentile_class_size: usize,
    /// Distinct rows over *every* released column, subset rule not applied. Answers the
    /// separate and simpler question of whether the file could be shuffled or aggregated,
    /// and acts as a check on the subset rule: a wide gap between this and
    /// `distinct_classes` means the rule is doing a lot of work and deserves a look.
    ///
    /// `None` when this histogram alone outgrew what the check keeps. It fills faster than
    /// the other one by construction — whole rows against projections of a subset of them —
    /// so it is suppressed on its own rather than taking the joint measure down with it.
    pub distinct_rows_all_columns: Option<usize>,
    /// The *joint* measurement stopped early because the file held more classes than the
    /// check keeps. Every count above is then a lower bound, and no verified claim may rest
    /// on them. Set only by the linkable histogram: `distinct_rows_all_columns` going
    /// absent does not make the joint figures incomplete.
    pub measurement_incomplete: bool,
    /// What `unique_rows` would have been with each matched column dropped, ascending by
    /// that count, then by column index.
    ///
    /// The only figure here a reader can act on. `unique_rows` says how exposed the file is;
    /// this says which column to change to fix it, which is the difference between a report
    /// and an alarm.
    ///
    /// Empty when `drop_attribution_incomplete` is set, and also when the file has no matched
    /// column to drop. The two are told apart by that flag rather than by the emptiness of
    /// this list, because "we did not measure" and "there is nothing to drop" are opposite
    /// findings that would otherwise look identical.
    pub drop_column_effects: Vec<DropColumnEffect>,
    /// The attribution was not run, or was stopped, so `drop_column_effects` is empty for a
    /// reason other than there being nothing to say.
    ///
    /// Set when the joint measurement itself is incomplete (there is no baseline to compare
    /// against), when the file has more columns than the attribution tracks, or when the
    /// leave-one-out histograms outgrew their shared budget. Reported rather than hidden: a
    /// reader who is told nothing about which column to drop should know whether that is
    /// because no column would help or because nobody looked.
    pub drop_attribution_incomplete: bool,
}

/// What dropping one column would do to the count of rows that stand alone.
///
/// Exact, not an estimate: the count is read off a second equivalence-class histogram built
/// over the same rows with this column's contribution removed, in the same single pass. That
/// matters because the intuitive estimate is wrong in both directions — dropping a column
/// whose projection is nearly constant changes almost nothing however revealing the column
/// looks, and dropping one of two correlated columns can change almost nothing either.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropColumnEffect {
    /// `ColumnMetadata::index`, as everywhere else.
    pub column_index: usize,
    /// Rows that would still be alone in their class with this column dropped, every other
    /// column unchanged. Compare against `RowUniquenessSummary::unique_rows`; it can never
    /// be larger, since removing a column only ever merges classes.
    pub unique_rows_without: usize,
}

/// One column the joint measure read, and what it was matched on.
///
/// The pairing is the point, and it must not be split back into two lists of column
/// indices — value-carrying and format-only. Two categories cannot express three kinds of
/// contribution, and the missing one is the partial match, the most common kind on a
/// pseudonymized file: a report built from two lists tells the reader that rows "share their
/// combination of birth_date, email" when what they share is a decade and a domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchedColumn {
    /// `ColumnMetadata::index`, which is how every other report structure names a column.
    pub column_index: usize,
    pub matched_on: MatchedPart,
    /// Whether every measured row actually carried `matched_on`, or only some of them did.
    ///
    /// `matched_on` is decided once per column, from the column's strategy and detected type
    /// alone — no cell value can change it. The values can still disagree with it: a cell
    /// that does not fit its column's detected shape is pseudonymized generically, and the
    /// projection returns nothing for that row. So a `Timestamp` column where one value in a
    /// hundred parses is `DateDecadeAndTime`, and the finding said the rows "share the decade
    /// and time of birth_date" of ninety-nine rows that carry no decade.
    ///
    /// A `bool` and not a count, because the report needs to know only whether to qualify the
    /// phrase. Quoting "matched on 1 of 100 rows" would invite the reader to weigh a number
    /// that is not the one that matters: those rows were counted as sharing nothing there,
    /// so the arithmetic is already right and only the wording was over-claiming.
    pub matched_every_row: bool,
}

/// What survived a column that an outsider holding the original could match against.
///
/// Written so each variant completes the sentence "rows share **…** with each other", which
/// is what forces the distinction: `WholeValue` licenses naming the column bare, and
/// nothing else does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchedPart {
    /// The released cell is the original, or a skeleton of it anyone can derive. The column
    /// may be named on its own.
    WholeValue,
    /// Everything from the last `@`: the employer, not the person.
    EmailDomain,
    /// The decade of the released date, and the time of day exactly. Approximate on the date
    /// half by construction — see `crate::uniqueness::LinkableProjection` — and named as an
    /// approximation so a reader is not told their rows share a birth date when they share a
    /// decade.
    DateDecadeAndTime,
    /// No part of the value, only a format property: a digit count, a separator layout, a
    /// number of name parts, a mask's word and letter counts. Counted like any other,
    /// because a joint measure is where individually weak signals combine, but never named
    /// as though it were the value.
    SurvivingFormat,
    /// Not the cell at all — only whether it was blank, and with which blank token.
    ///
    /// A cell the engine reads as empty is written through verbatim before any strategy
    /// runs, so even a redacted column publishes its missingness pattern, and someone
    /// holding the original record knows which of its fields were blank. Named apart because
    /// "the blank-cell pattern of address" and "address" are wildly different claims, and
    /// because the remedy differs too: no strategy fixes this one.
    BlankPattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformContext<'a> {
    pub column_name: &'a str,
    pub column_index: usize,
    pub row_index: usize,
    pub empty_format: EmptyFormat,
}

impl<'a> TransformContext<'a> {
    /// The context for one cell of `column`.
    ///
    /// Every caller wants exactly this: the column's own name, index and empty
    /// format, plus where the value sits. Assembled by hand at each site, the fields
    /// are three same-shaped values a mistake can silently swap — a context built
    /// with another column's index makes the transform key its mapping under the
    /// wrong column, so two columns share replacements and a value redacted in one
    /// reappears in the other.
    pub fn for_column(column: &'a ColumnMetadata, row_index: usize) -> Self {
        Self {
            column_name: &column.name,
            column_index: column.index,
            row_index,
            empty_format: column.empty_format,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadersData {
    pub file_path: PathBuf,
    pub row_count: usize,
    pub row_count_is_complete: bool,
    pub default_output_path: PathBuf,
    #[serde(default)]
    pub detection_run_summary: DetectionRunSummary,
    pub columns: Vec<ColumnMetadata>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteDataFormat {
    Auto,
    Csv,
    Json,
    Xml,
    Yaml,
    PlainText,
    Logs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteAnalyzeParams {
    pub content: String,
    pub format: PasteDataFormat,
    pub sample_row_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteAnalyzeData {
    pub format: PasteDataFormat,
    pub row_count: usize,
    pub row_count_is_complete: bool,
    /// How much of the paste the detected types actually rest on.
    ///
    /// Carried by the analyze result rather than only by the privacy report so the
    /// column table can caveat itself *before* the user selects columns and
    /// transforms. See [`DetectionCoverageSummary`].
    pub detection_coverage: DetectionCoverageSummary,
    #[serde(default)]
    pub detection_run_summary: DetectionRunSummary,
    pub columns: Vec<ColumnMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prepared_analysis: Option<crate::PreparedAnalysisSnapshot>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteTransformParams {
    pub content: String,
    pub format: PasteDataFormat,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    /// Rows to classify on, matching the figure paste analyze was given.
    pub sample_row_count: usize,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PastePreviewParams {
    pub content: String,
    pub format: PasteDataFormat,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    /// Rows to display. A window on the paste, not evidence about it.
    pub sample_count: usize,
    /// Rows to classify on, matching the figure paste analyze was given.
    pub sample_row_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PasteTransformData {
    pub output: String,
    pub row_count: usize,
    pub columns_anonymized: usize,
    pub duration_ms: u128,
    pub privacy_report: PrivacyReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickTransformParams {
    pub input: String,
    pub data_type: DataType,
    pub strategy: AnonymizationStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickGenerateParams {
    pub data_type: DataType,
    pub strategy: AnonymizationStrategy,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickTransformData {
    pub output: String,
    pub row_count: usize,
    pub values: Vec<SampleTransform>,
    pub privacy_report: PrivacyReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SampleTransform {
    pub original: String,
    pub anonymized: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnPreview {
    pub column_index: usize,
    pub column_name: String,
    pub samples: Vec<SampleTransform>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWarning {
    pub column_index: usize,
    pub column_name: String,
    pub message: String,
    pub severity: WarningSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartReplacementEntry {
    pub column_index: usize,
    pub original: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SmartReplacementRejectionReason {
    UnexpectedOriginal,
    MissingOutput,
    EmptyOutput,
    SameAsOriginal,
    ContainsOriginal,
    /// The replacement was, or contained, a source value belonging to a *different*
    /// row of the same column.
    ///
    /// Kept apart from [`Self::ContainsOriginal`] because the two describe different
    /// events and only one of them moves a person's data between rows.
    /// `ContainsOriginal` means the model echoed back the value it was asked to
    /// replace, which wastes the request; this means it emitted somebody else's real
    /// value, which would have published that value against the wrong record. A
    /// report that merged them could not say which had happened.
    MatchesOtherOriginal,
    ControlCharacter,
    DuplicateOriginal,
    DuplicateOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartReplacementRejectionCount {
    pub reason: SmartReplacementRejectionReason,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WarningSeverity {
    Info,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewData {
    pub previews: Vec<ColumnPreview>,
    pub warnings: Vec<PreviewWarning>,
    pub smart_replacements: Vec<SmartReplacementEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewParams {
    pub file_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    /// Rows to display. A window on the input, not evidence about it.
    pub sample_count: usize,
    /// Rows to classify on — the same figure analyze and the run are given, so the
    /// preview shows the strategies the run will apply rather than the ones a
    /// display-sized sample happens to imply.
    pub sample_row_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnonymizeParams {
    pub file_path: PathBuf,
    pub output_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub force: bool,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnonymizeData {
    pub output_path: PathBuf,
    pub row_count: usize,
    pub columns_anonymized: usize,
    pub duration_ms: u128,
    pub privacy_report: PrivacyReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PreflightMode {
    Preview,
    Anonymize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightParams {
    pub mode: PreflightMode,
    pub file_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<PathBuf>,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub force: bool,
    pub sample_row_count: usize,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
    pub local_ai_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_ai_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightData {
    pub mode: PreflightMode,
    pub readiness: ReleaseReadiness,
    pub evidence: Vec<ReleaseEvidenceItem>,
    pub column_reports: Vec<ColumnReleaseReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyReport {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection_run_summary: Option<DetectionRunSummary>,
    pub direct_identifiers: usize,
    pub quasi_identifiers: usize,
    pub pseudonymized_columns: usize,
    pub smart_replacement_columns: usize,
    pub opaque_token_columns: usize,
    pub masked_columns: usize,
    #[serde(default)]
    pub labelled_columns: usize,
    #[serde(default)]
    pub redacted_columns: usize,
    pub pass_through_columns: usize,
    pub unique_pseudonym_values: usize,
    pub reused_pseudonym_values: usize,
    pub collisions_avoided: usize,
    pub exhausted_pseudonym_pools: usize,
    pub opaque_token_values: usize,
    pub smart_replacement_values: usize,
    #[serde(default)]
    pub smart_replacement_rejections: usize,
    #[serde(default)]
    pub smart_replacement_rejection_reasons: Vec<SmartReplacementRejectionCount>,
    pub smart_replacement_fallbacks: usize,
    #[serde(default)]
    pub shape_fallback_values: usize,
    #[serde(default)]
    pub readiness: ReleaseReadiness,
    #[serde(default)]
    pub evidence: Vec<ReleaseEvidenceItem>,
    #[serde(default)]
    pub column_reports: Vec<ColumnReleaseReport>,
    #[serde(default)]
    pub column_value_distributions: Vec<ColumnValueDistribution>,
    #[serde(default)]
    pub row_uniqueness: Option<RowUniquenessSummary>,
    #[serde(default)]
    pub utility_metrics: Vec<UtilityMetric>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseReadiness {
    pub status: ReleaseReadinessStatus,
    pub blockers: Vec<String>,
    pub review_items: Vec<String>,
    pub verified_items: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseReadinessStatus {
    Verified,
    #[default]
    Review,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseEvidenceItem {
    pub id: String,
    pub label: String,
    pub status: ReleaseEvidenceStatus,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseEvidenceStatus {
    Verified,
    Review,
    Blocked,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnReleaseReport {
    pub column_index: usize,
    pub column_name: String,
    pub selected: bool,
    pub detected_type: DataType,
    pub pii_risk: PiiRisk,
    pub strategy: AnonymizationStrategy,
    pub action: String,
    pub status: ReleaseEvidenceStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtilityMetric {
    pub label: String,
    pub value: String,
    pub status: ReleaseEvidenceStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn column_metadata_serializes_frontend_contract_shape() {
        let column = ColumnMetadata {
            header_label_is_ambiguous: false,
            name: "email".to_string(),
            source_path: Some("$.user.email".to_string()),
            index: 2,
            detected_type: DataType::Email,
            confidence: Confidence::High,
            detection_trace: Some(DetectionTrace {
                summary: "email evidence".to_string(),
                selected_reason: "value matched email".to_string(),
                total_non_empty: 1,
                candidates: vec![DetectionTraceItem {
                    data_type: DataType::Email,
                    reason: "valid email".to_string(),
                    match_count: 1,
                    total_considered: 1,
                    confidence: Confidence::High,
                    accepted: true,
                }],
            }),
            privacy_findings: Vec::new(),
            privacy_evidence: vec![PrivacyEvidenceSummary {
                kind: PrivacyFindingKind::Contact,
                data_type: DataType::Email,
                confidence: Confidence::High,
                match_count: 1,
                sample_count: 1,
                score: 100,
                detector: "email".to_string(),
                reason: "Column contains contact details.".to_string(),
                detectors: vec!["email".to_string()],
            }],
            review_reasons: vec![ColumnReviewReason::AmbiguousContext],
            evidence_profile: Default::default(),
            pii_risk: PiiRisk::High,
            sample_values: vec!["ada@example.com".to_string()],
            sample_value_distribution: Default::default(),
            empty_format: EmptyFormat::EmptyString,
            is_selected: true,
            strategy: AnonymizationStrategy::Redact,
        };

        let value = serde_json::to_value(&column).unwrap();

        assert_eq!(value["detectedType"], json!("email"));
        assert_eq!(value["detectionTrace"]["totalNonEmpty"], json!(1));
        assert_eq!(value["privacyEvidence"][0]["matchCount"], json!(1));
        assert_eq!(value["reviewReasons"], json!(["ambiguousContext"]));
        assert_eq!(value["piiRisk"], json!("high"));
        assert_eq!(value["sampleValues"], json!(["ada@example.com"]));
        assert_eq!(value["emptyFormat"], json!("emptyString"));
        assert_eq!(value["isSelected"], json!(true));
        assert_eq!(value["strategy"], json!("redact"));
        assert!(value.get("detected_type").is_none());
        assert!(value.get("pii_risk").is_none());

        let round_trip: ColumnMetadata = serde_json::from_value(value).unwrap();
        assert_eq!(round_trip, column);
    }

    #[test]
    fn preflight_params_serialize_optional_and_default_contract_fields() {
        let params = PreflightParams {
            mode: PreflightMode::Anonymize,
            file_path: PathBuf::from("/data/input.csv"),
            output_path: None,
            columns: vec![0, 2],
            controls: Vec::new(),
            force: false,
            sample_row_count: 10,
            preview_smart_replacements: Vec::new(),
            local_ai_ready: false,
            local_ai_message: None,
        };

        let value = serde_json::to_value(&params).unwrap();

        assert_eq!(value["mode"], json!("anonymize"));
        assert_eq!(value["sampleRowCount"], json!(10));
        assert_eq!(value["previewSmartReplacements"], json!([]));
        assert_eq!(value["localAiReady"], json!(false));
        assert!(value.get("outputPath").is_none());
        assert!(value.get("localAiMessage").is_none());
        assert!(value.get("sample_row_count").is_none());

        let minimal = json!({
            "mode": "preview",
            "filePath": "/data/input.csv",
            "columns": [1],
            "force": false,
            "sampleRowCount": 5,
            "localAiReady": true
        });
        let decoded: PreflightParams = serde_json::from_value(minimal).unwrap();
        assert_eq!(decoded.mode, PreflightMode::Preview);
        assert_eq!(decoded.controls, Vec::<ColumnControl>::new());
        assert_eq!(
            decoded.preview_smart_replacements,
            Vec::<SmartReplacementEntry>::new()
        );
        assert_eq!(decoded.output_path, None);
        assert_eq!(decoded.local_ai_message, None);
    }

    #[test]
    fn privacy_report_serializes_nested_release_and_smart_replacement_fields() {
        let report = PrivacyReport {
            detection_run_summary: None,
            direct_identifiers: 1,
            quasi_identifiers: 2,
            pseudonymized_columns: 1,
            smart_replacement_columns: 1,
            opaque_token_columns: 0,
            masked_columns: 0,
            labelled_columns: 0,
            redacted_columns: 1,
            pass_through_columns: 0,
            unique_pseudonym_values: 3,
            reused_pseudonym_values: 0,
            collisions_avoided: 0,
            exhausted_pseudonym_pools: 0,
            opaque_token_values: 0,
            smart_replacement_values: 2,
            smart_replacement_rejections: 1,
            smart_replacement_rejection_reasons: vec![SmartReplacementRejectionCount {
                reason: SmartReplacementRejectionReason::ContainsOriginal,
                count: 1,
            }],
            smart_replacement_fallbacks: 1,
            shape_fallback_values: 2,
            readiness: ReleaseReadiness {
                status: ReleaseReadinessStatus::Review,
                blockers: Vec::new(),
                review_items: vec!["Review Smart replacement output.".to_string()],
                verified_items: Vec::new(),
            },
            evidence: vec![ReleaseEvidenceItem {
                id: "local-ai".to_string(),
                label: "Local AI".to_string(),
                status: ReleaseEvidenceStatus::Review,
                detail: "Review generated values.".to_string(),
            }],
            column_reports: vec![ColumnReleaseReport {
                column_index: 2,
                column_name: "email".to_string(),
                selected: true,
                detected_type: DataType::Email,
                pii_risk: PiiRisk::High,
                strategy: AnonymizationStrategy::LocalAi,
                action: "Smart replacement".to_string(),
                status: ReleaseEvidenceStatus::Review,
                detail: "Generated replacements.".to_string(),
            }],
            column_value_distributions: vec![ColumnValueDistribution {
                column_index: 2,
                distinct_values: 3,
                total_values: 10,
                singleton_values: 1,
                doubleton_values: 0,
                max_value_occurrences: 6,
            }],
            row_uniqueness: Some(RowUniquenessSummary {
                rows_measured: 10,
                matched_columns: vec![
                    MatchedColumn {
                        column_index: 0,
                        matched_on: MatchedPart::WholeValue,
                        matched_every_row: true,
                    },
                    MatchedColumn {
                        column_index: 2,
                        matched_on: MatchedPart::DateDecadeAndTime,
                        matched_every_row: true,
                    },
                    MatchedColumn {
                        column_index: 3,
                        matched_on: MatchedPart::SurvivingFormat,
                        matched_every_row: true,
                    },
                ],
                distinct_classes: 7,
                unique_rows: 4,
                smallest_class: 1,
                fifth_percentile_class_size: 1,
                distinct_rows_all_columns: Some(10),
                measurement_incomplete: false,
                drop_column_effects: vec![
                    DropColumnEffect {
                        column_index: 2,
                        unique_rows_without: 1,
                    },
                    DropColumnEffect {
                        column_index: 0,
                        unique_rows_without: 3,
                    },
                ],
                drop_attribution_incomplete: false,
            }),
            utility_metrics: vec![UtilityMetric {
                label: "Rows".to_string(),
                value: "10".to_string(),
                status: ReleaseEvidenceStatus::Info,
                detail: Some("sample".to_string()),
            }],
            notes: vec!["Review generated replacements.".to_string()],
        };

        let value = serde_json::to_value(&report).unwrap();

        assert_eq!(value["directIdentifiers"], json!(1));
        assert_eq!(value["smartReplacementColumns"], json!(1));
        assert_eq!(
            value["smartReplacementRejectionReasons"][0]["reason"],
            json!("containsOriginal")
        );
        assert_eq!(value["readiness"]["status"], json!("review"));
        assert_eq!(value["evidence"][0]["status"], json!("review"));
        assert_eq!(value["columnReports"][0]["detectedType"], json!("email"));
        assert_eq!(value["utilityMetrics"][0]["status"], json!("info"));
        assert_eq!(
            value["columnValueDistributions"][0]["distinctValues"],
            json!(3)
        );
        assert_eq!(
            value["columnValueDistributions"][0]["singletonValues"],
            json!(1)
        );
        assert!(value.get("direct_identifiers").is_none());
        assert!(value.get("smart_replacement_columns").is_none());

        let round_trip: PrivacyReport = serde_json::from_value(value).unwrap();
        assert_eq!(round_trip, report);
    }

    #[test]
    fn privacy_report_accepts_defaulted_newer_fields_when_deserializing() {
        let value = json!({
            "directIdentifiers": 1,
            "quasiIdentifiers": 0,
            "pseudonymizedColumns": 1,
            "smartReplacementColumns": 0,
            "opaqueTokenColumns": 0,
            "maskedColumns": 0,
            "passThroughColumns": 0,
            "uniquePseudonymValues": 1,
            "reusedPseudonymValues": 0,
            "collisionsAvoided": 0,
            "exhaustedPseudonymPools": 0,
            "opaqueTokenValues": 0,
            "smartReplacementValues": 0,
            "smartReplacementFallbacks": 0,
            "notes": []
        });

        let report: PrivacyReport = serde_json::from_value(value).unwrap();

        assert_eq!(report.redacted_columns, 0);
        assert_eq!(report.smart_replacement_rejections, 0);
        assert_eq!(report.smart_replacement_rejection_reasons, Vec::new());
        assert_eq!(report.readiness, ReleaseReadiness::default());
        assert_eq!(report.evidence, Vec::new());
        assert_eq!(report.column_reports, Vec::new());
        assert_eq!(report.utility_metrics, Vec::new());
    }

    #[test]
    fn selected_enums_use_camel_case_wire_values() {
        assert_eq!(
            serde_json::to_value(DataType::NumericId).unwrap(),
            json!("numericId")
        );
        assert_eq!(
            serde_json::to_value(PasteDataFormat::PlainText).unwrap(),
            json!("plainText")
        );
        assert_eq!(
            serde_json::to_value(AnonymizationStrategy::PassThrough).unwrap(),
            json!("passThrough")
        );
        assert_eq!(
            serde_json::to_value(ReleaseEvidenceStatus::Info).unwrap(),
            json!("info")
        );
    }

    fn distribution(distinct: usize, total: usize) -> ColumnValueDistribution {
        ColumnValueDistribution {
            column_index: 0,
            distinct_values: distinct,
            total_values: total,
            // No singletons, so coverage is 1.0 and the ratio test is reachable.
            // The shapes that make coverage matter are pinned separately below.
            singleton_values: 0,
            doubleton_values: 0,
            max_value_occurrences: 0,
        }
    }

    /// The floor is the whole reason this predicate is not just `distinct < 10`.
    /// `distinct_values <= total_values`, so below the floor the absolute test is
    /// vacuously true and every short column — including unique-key columns — looks
    /// low-cardinality. Measured: at 5 rows all 7 columns of `large.csv` trip it.
    #[test]
    fn cardinality_risk_ignores_columns_with_too_few_values_to_judge() {
        // The vacuous case: every value distinct, yet fewer than the constant.
        assert!(!distribution(5, 5).risks_frequency_inversion());
        assert!(!distribution(49, 49).risks_frequency_inversion());
        // Genuinely low cardinality, but still not enough rows to claim it.
        assert!(!distribution(4, 49).risks_frequency_inversion());
        // One value more and the same shape is judgeable.
        assert!(distribution(4, 50).risks_frequency_inversion());
    }

    #[test]
    fn cardinality_risk_flags_few_distinct_values_at_the_boundary() {
        assert!(distribution(9, 60).risks_frequency_inversion());
        // Ten distinct over sixty is 0.167, above the ratio threshold too, so this
        // pins both terms at once: neither may fire here.
        assert!(!distribution(10, 60).risks_frequency_inversion());
    }

    /// The ratio term exists for columns the absolute term cannot reach. Measured on
    /// `large.csv name`: 100 distinct over 10500 values.
    #[test]
    fn cardinality_risk_flags_a_large_column_by_ratio() {
        assert!(distribution(100, 10_500).risks_frequency_inversion());
        // Same distinct count, far fewer rows: 0.1 is above the threshold, and the
        // absolute term does not reach 100 either.
        assert!(!distribution(100, 1_000).risks_frequency_inversion());
    }

    /// Below 200 values the ratio term is subsumed by the absolute one, which is why
    /// 0.05 was chosen: small and medium inputs are judged on the statistic that
    /// saturates rather than on one that keeps drifting as more rows are read.
    #[test]
    fn the_ratio_term_is_inert_below_two_hundred_values() {
        for total in [50usize, 100, 199] {
            for distinct in 10..=total {
                let subject = distribution(distinct, total);
                assert!(
                    !subject.risks_frequency_inversion(),
                    "{distinct} distinct over {total} fired without the absolute term"
                );
            }
        }
    }

    /// Counts the values a transform would count: empties skipped, case and padding
    /// folded. A pre-run warning that measured differently from the run would
    /// contradict the report built from the same data.
    #[test]
    fn distribution_from_values_matches_the_transform_value_identity() {
        let values = [
            "Sales".to_string(),
            "  sales  ".to_string(),
            "SALES".to_string(),
            "Legal".to_string(),
            String::new(),
            "null".to_string(),
            "NULL".to_string(),
            "   ".to_string(),
        ];

        let subject = ColumnValueDistribution::from_values(3, &values);

        assert_eq!(subject.column_index, 3);
        assert_eq!(subject.distinct_values, 2);
        assert_eq!(subject.total_values, 4);
        assert_eq!(subject.singleton_values, 1);
        assert_eq!(subject.doubleton_values, 0);
        assert_eq!(subject.max_value_occurrences, 3);
    }

    /// Singletons and doubletons are counted after identity folding too, since they
    /// are what [`ColumnValueDistribution::estimated_distinct_values`] reads.
    #[test]
    fn distribution_counts_singletons_and_doubletons() {
        let values = ["a", "a", "b", "b", "c", "d", "d", "d"]
            .map(str::to_string)
            .to_vec();

        let subject = ColumnValueDistribution::from_values(0, &values);

        assert_eq!(subject.distinct_values, 4);
        assert_eq!(subject.singleton_values, 1, "c");
        assert_eq!(subject.doubleton_values, 2, "a and b");
    }

    /// A sampled column of a large file is the case the ratio test could not see. The
    /// numbers are the measured ones for 30 departments over 5000 rows.
    #[test]
    fn a_sampled_distribution_is_judged_against_the_columns_real_size() {
        let sampled = ColumnValueDistribution {
            column_index: 1,
            distinct_values: 29,
            total_values: 100,
            singleton_values: 5,
            doubleton_values: 4,
            max_value_occurrences: 8,
        };

        // Against the sample's own size the ratio is 0.29 and the absolute term does
        // not reach 29 either, so judged this way the column looks safe.
        assert!(!sampled.risks_frequency_inversion());
        // Against the file it was drawn from, it is 30-odd values over 5000 rows.
        assert!(sampled.frequency_inversion_risk_in(5_000).is_some());
    }

    /// The counterweight, and why the ratio test cannot simply divide by the row
    /// count: a sample of a hundred cannot look like a million distinct values, so a
    /// fully unique column scores 0.005 on the ratio alone. Coverage is what
    /// distinguishes "few distinct values" from "a sample that learned nothing".
    #[test]
    fn a_sample_that_learned_nothing_cannot_raise_a_warning() {
        let every_value_new = ColumnValueDistribution {
            column_index: 0,
            distinct_values: 100,
            total_values: 100,
            singleton_values: 100,
            doubleton_values: 0,
            max_value_occurrences: 1,
        };

        assert!(
            every_value_new
                .frequency_inversion_risk_in(1_000_000)
                .is_none()
        );
    }

    /// Coverage gates the ratio term only. A column with few enough distinct values
    /// is invertible whatever the coverage figure says, so the absolute term has to
    /// answer first.
    #[test]
    fn the_absolute_term_is_not_gated_by_coverage() {
        let sparse_but_unsaturated = ColumnValueDistribution {
            column_index: 0,
            distinct_values: 9,
            total_values: 60,
            // Coverage 0.0, far below the gate.
            singleton_values: 60,
            doubleton_values: 0,
            max_value_occurrences: 1,
        };

        assert!(
            sparse_but_unsaturated
                .frequency_inversion_risk_in(1_000_000)
                .is_some()
        );
    }

    /// Chao1 estimates the groups a sample missed from how many it saw exactly once.
    /// Pinned on the measured shape rather than the formula, so a rewrite that keeps
    /// the behaviour passes and one that changes it fails.
    #[test]
    fn the_estimated_distinct_count_allows_for_unseen_values() {
        let sampled = ColumnValueDistribution {
            column_index: 0,
            distinct_values: 29,
            total_values: 100,
            singleton_values: 5,
            doubleton_values: 4,
            max_value_occurrences: 8,
        };
        // 29 + 25/8 = 32, against a true count of 30.
        assert_eq!(sampled.estimated_distinct_values(), 32);

        // Nothing seen twice: the bias-corrected form, and no division by zero.
        let no_doubletons = ColumnValueDistribution {
            distinct_values: 10,
            total_values: 100,
            singleton_values: 4,
            doubleton_values: 0,
            ..ColumnValueDistribution::default()
        };
        assert_eq!(no_doubletons.estimated_distinct_values(), 10 + 6);

        // Every value repeated: nothing was missed.
        let fully_measured = ColumnValueDistribution {
            distinct_values: 30,
            total_values: 5_000,
            singleton_values: 0,
            doubleton_values: 0,
            ..ColumnValueDistribution::default()
        };
        assert_eq!(fully_measured.estimated_distinct_values(), 30);
    }

    #[test]
    fn distribution_from_no_usable_values_is_empty_rather_than_risky() {
        let values = [String::new(), "null".to_string(), "  ".to_string()];

        let subject = ColumnValueDistribution::from_values(0, &values);

        assert_eq!(subject.total_values, 0);
        assert_eq!(subject.max_value_occurrences, 0);
        assert!(!subject.risks_frequency_inversion());
    }

    /// A default distribution is what a caller that never measured produces, so it
    /// must never be able to raise a warning it has no evidence for.
    #[test]
    fn a_defaulted_distribution_cannot_raise_a_warning() {
        assert!(!ColumnValueDistribution::default().risks_frequency_inversion());
    }

    /// One dominant value over forty-nine near-unique ones.
    ///
    /// Not invented: these are the exact figures the detection sample computes for
    /// `tests/fixtures/dominant-value.csv`, pinned in
    /// `service::tests::cardinality::the_dominant_value_fixture_has_the_shape_these_tests_depend_on`,
    /// so the unit tests here and the integration tests there are reasoning about one
    /// column rather than two that happen to resemble each other.
    fn dominant_value_sample() -> ColumnValueDistribution {
        ColumnValueDistribution {
            column_index: 1,
            distinct_values: 50,
            total_values: 100,
            singleton_values: 49,
            doubleton_values: 0,
            max_value_occurrences: 51,
        }
    }

    /// The gap this term closes. A column that is *diverse* — here 50 distinct values in
    /// the sample and thousands in the file — but whose most common value covers half the
    /// rows leaks half the column the moment one pseudonym is matched, and until this term
    /// existed it drew no warning at all: 50 distinct values is far above the absolute
    /// limit, and the Chao1 ratio is high precisely because the column is diverse.
    ///
    /// Both older terms are asserted against as well as the verdict. Without them a change
    /// that made the *absolute* term start firing at 50 distinct values would leave this
    /// test green while flagging most columns in the corpus.
    #[test]
    fn a_dominant_value_is_flagged_however_diverse_the_rest_of_the_column_is() {
        let subject = dominant_value_sample();

        assert!(subject.distinct_values >= MAX_INVERTIBLE_DISTINCT_VALUES);
        assert!(subject.sample_coverage() < MIN_SAMPLE_COVERAGE);

        assert!(subject.frequency_inversion_risk_in(5_000_000).is_some());
    }

    /// The same column with the dominance taken out of it, and nothing else changed.
    ///
    /// Half the rows moved off the dominant value onto values of their own, so the column
    /// is *more* diverse than the one above while holding the same 100 values. It has to
    /// go silent, or the term is reading something other than dominance.
    #[test]
    fn the_same_column_without_a_dominant_value_stays_silent() {
        let spread_out = ColumnValueDistribution {
            distinct_values: 76,
            singleton_values: 75,
            max_value_occurrences: 25,
            ..dominant_value_sample()
        };

        assert!(spread_out.frequency_inversion_risk_in(5_000_000).is_none());
    }

    /// The dominant-value term is deliberately ahead of the coverage gate, and this is
    /// the reason: the shape it catches is singleton-heavy, so its coverage is low. The
    /// fixture's coverage is 0.51 against a 0.75 gate. Behind the gate this term would have
    /// been silent on almost every column it was added for: of 2400 measured columns whose
    /// top value covered half the rows, none reached coverage 0.75, and of 2400 covering
    /// three fifths, six did.
    #[test]
    fn the_dominant_value_term_is_not_gated_by_coverage() {
        let subject = dominant_value_sample();

        assert!(subject.sample_coverage() < MIN_SAMPLE_COVERAGE);
        assert!(subject.frequency_inversion_risk_in(5_000_000).is_some());
    }

    /// A share and not a count, which is the whole point of the constant: the identical
    /// shape has to answer the same way at every file size. A count-based rule would
    /// have to be either silent on the small file or noisy on the large one.
    #[test]
    fn the_dominant_value_verdict_does_not_move_with_the_files_size() {
        for population in [100usize, 5_000, 1_000_000, 5_000_000] {
            assert!(
                dominant_value_sample()
                    .frequency_inversion_risk_in(population)
                    .is_some(),
                "silent at {population} values"
            );
        }
    }

    /// The boundary, measured on a fully counted column so no estimate is involved:
    /// 20 of 60 is exactly a third and fires, 19 of 60 does not.
    ///
    /// The surrounding counts are chosen so neither other term can answer — 15 distinct
    /// clears the absolute limit, and Chao1 over the column's own 60 values is 0.67,
    /// nowhere near the 0.05 ratio limit — so the verdict here is the dominant-value
    /// term's alone.
    #[test]
    fn the_dominant_share_boundary_sits_at_one_third() {
        let at_the_boundary = ColumnValueDistribution {
            column_index: 0,
            distinct_values: 15,
            total_values: 60,
            singleton_values: 10,
            doubleton_values: 2,
            max_value_occurrences: 20,
        };
        assert!(at_the_boundary.risks_frequency_inversion());

        let one_row_short = ColumnValueDistribution {
            max_value_occurrences: 19,
            ..at_the_boundary
        };
        assert!(!one_row_short.risks_frequency_inversion());
    }

    /// A genuinely unique column has to stay silent, and the dominant-value term must
    /// not be what breaks that. Its most common value covers one row, so its share is
    /// `1 / total_values` — below a third for any column past the floor, which is why
    /// the term needs no special case for it. Pinned at four sizes because a share is
    /// exactly the kind of quantity that misbehaves at the small end.
    #[test]
    fn a_unique_column_cannot_trip_the_dominant_value_term() {
        for total in [50usize, 60, 5_000, 1_000_000] {
            let unique = ColumnValueDistribution {
                column_index: 0,
                distinct_values: total,
                total_values: total,
                singleton_values: total,
                doubleton_values: 0,
                max_value_occurrences: 1,
            };

            assert!(
                unique.frequency_inversion_risk_in(total).is_none(),
                "{total} unique values were flagged"
            );
        }
    }

    /// The false-positive case the constant was calibrated against. This is a measured
    /// 100-value sample of a Zipf column with exponent 1.0 over 1000 labels — the
    /// ordinary shape of real categorical data, whose top value takes a seventh of the
    /// rows. Over 4000 such samples no draw reached a third, and a threshold low enough
    /// to catch this shape would fire on most text columns in most files.
    #[test]
    fn a_mildly_skewed_high_cardinality_column_stays_silent() {
        let ordinary_skew = ColumnValueDistribution {
            column_index: 0,
            distinct_values: 68,
            total_values: 100,
            singleton_values: 55,
            doubleton_values: 8,
            max_value_occurrences: 14,
        };

        assert!(ordinary_skew.frequency_inversion_risk_in(100_000).is_none());
    }
}
