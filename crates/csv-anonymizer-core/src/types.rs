use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DataType {
    Email,
    Uuid,
    Timestamp,
    NumericId,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedSample {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessOptions<'a> {
    pub deterministic: bool,
    pub seed: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessResult {
    pub row_count: usize,
    pub success: bool,
    pub output_path: PathBuf,
    pub duration_ms: u128,
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
pub struct PreviewData {
    pub previews: Vec<ColumnPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewParams {
    pub file_path: PathBuf,
    pub columns: Vec<usize>,
    pub deterministic: bool,
    pub seed: String,
    pub sample_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnonymizeParams {
    pub file_path: PathBuf,
    pub output_path: PathBuf,
    pub columns: Vec<usize>,
    pub deterministic: bool,
    pub seed: String,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnonymizeData {
    pub output_path: PathBuf,
    pub row_count: usize,
    pub columns_anonymized: usize,
    pub duration_ms: u128,
}
