use super::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
