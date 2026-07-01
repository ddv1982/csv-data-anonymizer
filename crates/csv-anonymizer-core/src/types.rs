use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
                PrivacyFindingKind::PrivateAddress,
                "Column type indicates postal address context.",
            )),
            DataType::TaxId => Some((
                PrivacyFindingKind::GovernmentId,
                "Column type indicates government or tax identifier data.",
            )),
            DataType::NumericId => Some((
                PrivacyFindingKind::AccountOrFinancialId,
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
            DataType::NumericId | DataType::Uuid => Some(RedactionPlaceholder::AccountId),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedactionPlaceholder {
    Email,
    Phone,
    Person,
    Address,
    Date,
    AccountId,
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
    PrivateDate,
    AccountOrFinancialId,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetadata {
    pub name: String,
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
    pub is_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessOptions<'a> {
    pub smart_replacements: Option<&'a crate::smart::SmartReplacementMap>,
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
    pub sample_count: usize,
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
    pub sample_count: usize,
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
}
