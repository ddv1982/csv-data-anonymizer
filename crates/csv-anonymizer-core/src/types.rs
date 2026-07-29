use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// The largest detection basis any entry point accepts, for any input kind.
///
/// One limit rather than one per workflow, because the figure comes from one
/// setting. "Sample rows" is not per-workflow: a user who raises it to work on a
/// large CSV file has raised it for the paste workflow too, so a value that reaches
/// the setting has to be a value every entry point will honour. The paste path used
/// to cap itself an order of magnitude lower and reject the rest, which made a
/// perfectly valid setting break pasted input while files kept working.
///
/// Enforced twice, and both sites read this: `settings::sanitize_settings` clamps
/// what can be stored, and the paste entry points reject an oversized request
/// outright, since they are reachable by callers that never went through settings.
pub const MAX_SAMPLE_ROW_COUNT: usize = 10_000;

/// The largest display window any entry point accepts, for any input kind. One
/// limit for the same reason as [`MAX_SAMPLE_ROW_COUNT`] — it comes from the
/// "Preview rows" setting, which is likewise not per-workflow.
pub const MAX_PREVIEW_SAMPLE_COUNT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
            DataType::Uuid | DataType::IpAddress | DataType::MacAddress => Some((
                PrivacyFindingKind::NetworkOrDeviceId,
                "Column type indicates network, device, or persistent identifiers.",
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
            DataType::Address | DataType::PostalCode => Some(RedactionPlaceholder::Address),
            DataType::Timestamp => Some(RedactionPlaceholder::Date),
            // Neither is an account. A bare identifier column is a record key, and
            // a UUID is a machine-generated handle — which is also what its privacy
            // finding says (`NetworkOrDeviceId`), so redacting it as an account id
            // contradicted the classification shown next to it in the report.
            DataType::NumericId => Some(RedactionPlaceholder::RecordId),
            DataType::Uuid => Some(RedactionPlaceholder::NetworkId),
            DataType::TaxId => Some(RedactionPlaceholder::GovernmentId),
            DataType::Url => Some(RedactionPlaceholder::Url),
            DataType::IpAddress | DataType::MacAddress => Some(RedactionPlaceholder::NetworkId),
            DataType::String
            | DataType::Unknown
            | DataType::Enum
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
/// integers gets `[RECORD_ID]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedactionPlaceholder {
    Email,
    Phone,
    Person,
    Address,
    Date,
    RecordId,
    GovernmentId,
    Url,
    NetworkId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PiiRisk {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EmptyFormat {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnonymizationStrategy {
    Auto,
    Pseudonymize,
    Tokenize,
    LocalAi,
    Mask,
    Label,
    Redact,
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
    pub success: bool,
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
/// about the input's distinct count, so the ratio test is skipped.
///
/// Good–Turing coverage, `1 - singletons/values`: the estimated share of the column's
/// values that belong to groups the sample has already seen. Near 1 the sample has
/// found essentially every group and its distinct count is the column's; near 0 every
/// value seen was new, so the sample has learned nothing except that there are many.
///
/// This gate is what makes the ratio test safe on sampled data, and without it the
/// test is actively wrong. Simulated 100-value samples drawn evenly across columns of
/// known shape:
///
/// | column | coverage | Chao1 / rows | should warn |
/// | --- | --- | --- | --- |
/// | 3 statuses in 5k rows | 1.00 | 0.0006 | yes |
/// | 30 departments in 5k rows | 0.95 | 0.0066 | yes |
/// | 50 job titles in 100k rows | 0.87 | 0.0005 | yes |
/// | 200 cities in 5k rows | 0.40 | 0.0412 | no — 25-row groups |
/// | 1000 names in 5k rows | 0.12 | 0.1478 | no |
/// | unique in 5k rows | 0.04 | 0.4804 | no |
/// | **unique in 1M rows** | **0.00** | **0.0051** | **no** |
///
/// The last row is the reason this constant exists: a fully unique column in a large
/// file passes the ratio test outright, because 100 sampled values can never look
/// like a million distinct ones. Coverage is the statistic that separates it, and it
/// separates every case above — the data pins this constant only to the interval
/// (0.40, 0.87], and 0.75 sits inside it with margin at both ends.
///
/// The table above was measured on *uniform* draws, and the claim that skew only moves
/// coverage upward was an argument rather than a measurement. Re-measured on Zipf draws
/// with `zipf_column_file` in `service::tests::cardinality`: 100-value samples of a
/// 5000-row column, 20 draws per cell, worst (lowest) coverage of the 20:
///
/// | labels | s=0.5 | s=0.8 | s=1.0 | s=1.2 | s=1.5 | s=2.0 |
/// | --- | --- | --- | --- | --- | --- | --- |
/// | 200 | 0.32 | 0.48 | 0.60 | 0.68 | 0.78 | 0.87 |
/// | 1000 | 0.08 | 0.21 | 0.40 | 0.59 | 0.75 | 0.86 |
/// | 5000 | 0.00 | 0.10 | 0.24 | 0.47 | 0.74 | 0.88 |
///
/// The constant survives: coverage does rise monotonically with skew at every label
/// count, so the gate opens as a column becomes more invertible, and it stays shut on
/// every diverse column here — a Zipf-1.0 column over 1000 labels holds around 750
/// distinct values in 5000 rows and must not be flagged.
///
/// It does not open in time on its own, though. At 5000 labels a Zipf-1.5 column, whose
/// top value already takes 39% of the rows, still draws samples below the gate. And the
/// case that pushed hardest is one the uniform draws could not produce at all: skew
/// raises coverage only while the skew is in the *body* of the distribution. One dominant
/// value over a long unique tail is severely skewed and has *low* coverage, because
/// coverage counts singletons and the tail is all singletons. Measured with
/// `dominant_value_column_file` on one value covering `q` of a 5000-row column, the rest
/// spread over 5000 others, 20 draws per cell:
///
/// | q | 0.2 | 0.3 | 0.4 | 0.5 | 0.6 | 0.8 |
/// | --- | --- | --- | --- | --- | --- | --- |
/// | coverage | 0.15–0.28 | 0.23–0.40 | 0.33–0.51 | 0.46–0.59 | 0.54–0.70 | 0.71–0.86 |
///
/// At 20 draws per cell every draw up to q=0.6 sat below 0.75. Re-run at 400 draws per
/// cell the bound is not quite absolute: 0 of 2400 draws at q=0.5 reached the gate, and at
/// q=0.6 six did — and those six are exactly the six on which the ratio term fired, out of
/// 2400 columns where one value covered three fifths of the rows.
///
/// That is why [`MIN_INVERTIBLE_DOMINANT_SHARE`] is checked *before* this gate rather than
/// behind it: gating the dominant-value term on coverage would have silenced all but a
/// quarter of a percent of the shape the term exists for. This constant is right for what
/// it gates — a distinct-count estimate, which a singleton-heavy sample genuinely cannot
/// make — and carries no authority over anything else.
///
/// Still not tested: draws from real production data, non-Zipf skew (bimodal columns, a
/// few large groups with no tail at all), and columns sitting near the gate itself, where
/// coverage moves by several hundredths between draws of the same column and the gate's
/// answer is therefore a coin flip rather than a verdict.
const MIN_SAMPLE_COVERAGE: f64 = 0.75;

/// Share of a column's values carried by its single most common value, at or above
/// which the mapping is treated as frequency-invertible.
///
/// The failure this closes: a column with thousands of distinct values, one of which
/// covers most of the rows. Neither other term sees it. `distinct < 10` is nowhere near
/// true, and the Chao1 ratio is high precisely *because* the column is diverse — so a
/// column where one value covers 60% of five million rows drew no warning at all, even
/// though inverting that single pseudonym hands back 60% of the column to anyone who
/// knows which value is the common one. Measured with the dominant-value generator in
/// `service::tests::cardinality`: across 120 draws each at every combination of 1000 and
/// 5000 tail values and 1000, 5000 and 100000 rows, the absolute term fired 0 times and
/// the ratio term fired 0 times for every q from 0.2 to 0.6 inclusive.
///
/// A share and not a count. `max_value_occurrences > 5000` is nonsense in a 200-row file
/// and nearly always true in a 5-million-row one; the share is the quantity that means
/// the same thing at both sizes, and it is also the quantity with the direct reading —
/// the fraction of the column one inversion recovers.
///
/// Calibrated on the pre-run path, which is the hard case: the post-run report measures
/// every row and the share is exact, but the preview measures a 100-value sample, so the
/// share is an estimate with real variance. Fire rate of each candidate threshold, 4000
/// independent 100-value samples per configuration, given as the range over every column
/// size tested — 200/1000/5000 labels for the Zipf rows, 1000/5000 tail values for the
/// dominant ones, and 1000/5000/100000 rows for both:
///
/// | true dominant share | T=0.25 | T=0.30 | **T=1/3** | T=0.35 | T=0.40 | T=0.50 |
/// | --- | --- | --- | --- | --- | --- | --- |
/// | Zipf s=1.0 (0.11–0.17) | .000–.031 | .000–.001 | **.000** | .000 | .000 | .000 |
/// | Zipf s=1.1 (0.16–0.21) | .010–.212 | .000–.028 | **.000–.002** | .000–.001 | .000 | .000 |
/// | Zipf s=1.2 (0.21–0.26) | .215–.624 | .025–.210 | **.001–.047** | .001–.031 | .000–.002 | .000 |
/// | one value over 50% | 1.000 | 1.000 | **.999–1.000** | .999–1.000 | .979–.983 | .537–.542 |
/// | one value over 60% | 1.000 | 1.000 | **1.000** | 1.000 | 1.000 | .980–.985 |
///
/// Two requirements pick the constant from that table. A Zipf column with exponent up to
/// 1.1 must stay silent: Zipf with s near 1 is the ordinary shape of real categorical
/// data, its top value takes a fifth of the rows at most, and a warning that fires there
/// fires on most text columns in most files — the same noise argument that keeps
/// singleton counts out of the predicate entirely. A column where one value genuinely
/// covers half the rows must be caught. The data pins the constant to the interval
/// **[1/3, 0.35]**: at 0.30 a Zipf-1.1 column false-fires on 2.8% of samples, and at 0.40
/// a truly 50%-dominant column is missed on 2% of them. 1/3 rather than 0.35 because the
/// two are indistinguishable on every measurement here and 1/3 states the rule the
/// warning is making — one value in every three rows.
///
/// The interval is narrow because both requirements are strict, and it should be read as
/// what the measurements happen to admit rather than as a discovered boundary. Relaxing
/// the second requirement to "caught on 95% of samples" moves its upper end past 0.40,
/// where the rate is .979, but not as far as 0.45, where it is .859. Tightening either
/// requirement — ten times the replicates, or demanding the false-positive rate hold at
/// Zipf s=1.2 as well — would narrow the interval or empty it; that is an extrapolation
/// from the trend across the table, not a measurement.
///
/// The honest summary: a 100-value sample cannot reliably separate a 26%-dominant column
/// from a 40%-dominant one — at 1/3 the first fires on up to 4.7% of samples and the
/// second on 90% of them — and no choice of constant makes it able to. What the
/// measurements do establish is that 1/3 separates the shapes at the two *ends* — ordinary
/// skew and one-value dominance — with a false-positive rate at or below 0.2% and a miss
/// rate at or below 0.1%.
///
/// Not tested: real production columns, tails that are not uniform, columns whose second
/// value is nearly as common as the first (where inverting the top pseudonym is a coin
/// flip rather than a certainty, so this term over-warns by construction), samples larger
/// than 100 values — the "Sample rows" setting can only raise that figure, which shrinks
/// the variance above and so can only move the fire rates toward the exact post-run
/// answer — and the interaction with a column that is *also* low-cardinality, which the
/// absolute term answers first.
const MIN_INVERTIBLE_DOMINANT_SHARE: f64 = 1.0 / 3.0;

/// Which of the three tests judged a distribution frequency-invertible.
///
/// Exists so a warning can name the evidence it actually has. The three terms catch
/// genuinely different shapes — a handful of values, one value dominating a diverse
/// column, and many small groups across a large one — and a single wording cannot
/// describe all three without describing at least two of them wrongly. A column of
/// 101 values where one covers half the rows was previously reported as holding
/// "only 101 distinct value(s)", which is true, reads as reassuring, and names a risk
/// the column does not have while staying silent about the one it does.
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
    /// the one statistic here a small sample can actually estimate: over 2400 draws, a
    /// 100-value spread sample of a column whose top value covers half the rows reported a
    /// share between 0.36 and 0.69, while the same samples' distinct counts under-reported
    /// the columns' by one to two orders of magnitude. That asymmetry is why the
    /// distinct-count term needs both a population figure and a coverage gate while this
    /// one needs neither.
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
        // columns it was added for. See [`MIN_SAMPLE_COVERAGE`] for the figures.
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformContext<'a> {
    pub column_name: &'a str,
    pub column_index: usize,
    pub row_index: usize,
    pub empty_format: EmptyFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadersData {
    pub file_path: PathBuf,
    pub row_count: usize,
    pub row_count_is_complete: bool,
    pub default_output_path: PathBuf,
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
    pub columns: Vec<ColumnMetadata>,
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
