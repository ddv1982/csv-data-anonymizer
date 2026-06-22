use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetadata {
    pub name: String,
    pub index: usize,
    pub detected_type: DataType,
    pub confidence: Confidence,
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
    pub deterministic: bool,
    pub seed: &'a str,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransformReport {
    pub unique_pseudonym_values: usize,
    pub reused_pseudonym_values: usize,
    pub collisions_avoided: usize,
    pub exhausted_pseudonym_pools: usize,
    pub opaque_token_values: usize,
    pub smart_replacement_values: usize,
    pub smart_replacement_fallbacks: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransformContext<'a> {
    pub column_name: &'a str,
    pub column_index: usize,
    pub row_index: usize,
    pub seed: &'a str,
    pub deterministic: bool,
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
    pub deterministic: bool,
    pub seed: String,
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
    pub deterministic: bool,
    pub seed: String,
    pub force: bool,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
    #[serde(default)]
    pub privacy_config: Option<PrivacyConfig>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyReport {
    pub release_mode: ReleaseMode,
    pub direct_identifiers: usize,
    pub quasi_identifiers: usize,
    pub sensitive_columns: usize,
    pub pseudonymized_columns: usize,
    pub smart_replacement_columns: usize,
    pub opaque_token_columns: usize,
    pub masked_columns: usize,
    pub generalized_columns: usize,
    pub pass_through_columns: usize,
    pub suppressed_rows: usize,
    pub synthetic_rows: usize,
    pub dp_epsilon: Option<String>,
    pub unique_pseudonym_values: usize,
    pub reused_pseudonym_values: usize,
    pub collisions_avoided: usize,
    pub exhausted_pseudonym_pools: usize,
    pub opaque_token_values: usize,
    pub smart_replacement_values: usize,
    pub smart_replacement_fallbacks: usize,
    pub formal_models: Vec<PrivacyModelReport>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseMode {
    #[default]
    Standard,
    FormalTabular,
    DifferentialPrivacyAggregate,
    SyntheticData,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyConfig {
    #[serde(default)]
    pub release_mode: ReleaseMode,
    #[serde(default)]
    pub column_roles: Vec<PrivacyColumnRole>,
    #[serde(default)]
    pub formal: FormalPrivacyConfig,
    #[serde(default)]
    pub differential_privacy: DifferentialPrivacyConfig,
    #[serde(default)]
    pub synthetic: SyntheticDataConfig,
}

impl PrivacyConfig {
    pub fn standard() -> Self {
        Self::default()
    }
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            release_mode: ReleaseMode::Standard,
            column_roles: Vec::new(),
            formal: FormalPrivacyConfig::default(),
            differential_privacy: DifferentialPrivacyConfig::default(),
            synthetic: SyntheticDataConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColumnRole {
    #[default]
    Auto,
    DirectIdentifier,
    QuasiIdentifier,
    Sensitive,
    Attribute,
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyColumnRole {
    pub column_index: usize,
    #[serde(default)]
    pub role: ColumnRole,
    #[serde(default)]
    pub generalization_level: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormalPrivacyConfig {
    #[serde(default = "default_k_anonymity")]
    pub k: usize,
    #[serde(default)]
    pub l_diversity: Option<usize>,
    #[serde(default)]
    pub t_closeness: Option<f64>,
    #[serde(default = "default_suppress_small_classes")]
    pub suppress_small_classes: bool,
}

impl Default for FormalPrivacyConfig {
    fn default() -> Self {
        Self {
            k: default_k_anonymity(),
            l_diversity: None,
            t_closeness: None,
            suppress_small_classes: default_suppress_small_classes(),
        }
    }
}

fn default_k_anonymity() -> usize {
    5
}

fn default_suppress_small_classes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifferentialPrivacyConfig {
    #[serde(default = "default_epsilon")]
    pub epsilon: f64,
    #[serde(default)]
    pub aggregate: DpAggregate,
    #[serde(default)]
    pub group_by_column: Option<usize>,
    #[serde(default)]
    pub value_column: Option<usize>,
    #[serde(default)]
    pub lower_bound: Option<f64>,
    #[serde(default)]
    pub upper_bound: Option<f64>,
}

impl Default for DifferentialPrivacyConfig {
    fn default() -> Self {
        Self {
            epsilon: default_epsilon(),
            aggregate: DpAggregate::Count,
            group_by_column: None,
            value_column: None,
            lower_bound: None,
            upper_bound: None,
        }
    }
}

fn default_epsilon() -> f64 {
    1.0
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DpAggregate {
    #[default]
    Count,
    Sum,
    Mean,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyntheticDataConfig {
    #[serde(default)]
    pub row_count: Option<usize>,
    #[serde(default)]
    pub epsilon: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyModelReport {
    pub model: PrivacyModel,
    pub satisfied: bool,
    pub actual: String,
    pub threshold: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrivacyModel {
    KAnonymity,
    LDiversity,
    TCloseness,
    DifferentialPrivacy,
    SyntheticData,
}
