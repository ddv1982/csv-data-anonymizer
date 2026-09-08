mod column;
mod processing;
mod report;
mod sampling;
mod workflow;

pub use column::{
    AnonymizationStrategy, ColumnControl, ColumnEvidenceProfile, ColumnMetadata,
    ColumnReviewReason, Confidence, DataType, DetectionResult, DetectionRunSummary, DetectionTrace,
    DetectionTraceItem, DeterministicDetectionStatus, EmptyFormat, EvidenceDisposition,
    FormatEvidence, FormatEvidenceBasis, LocalNerRunStatus, PiiRisk, PrivacyDecision,
    PrivacyEvidenceSummary, PrivacyFinding, PrivacyFindingKind, RedactionDecision,
    RedactionPlaceholderSource, SemanticDecision, SemanticSpecificity, SemanticStatus,
};
pub(crate) use column::{
    DetectionReviewReason, GENERIC_STRING_STRUCTURE_DISCLOSURE, RedactionPlaceholder,
    ReportIdentifierClass,
};
pub use processing::{
    ProcessControl, ProcessOptions, ProcessProgress, ProcessResult, SmartReplacementEntry,
    SmartReplacementRejectionCount, SmartReplacementRejectionReason, TransformContext,
    TransformReport,
};
pub(crate) use report::FrequencyInversionRisk;
pub use report::{
    ColumnReleaseReport, ColumnValueDistribution, DropColumnEffect, MatchedColumn, MatchedPart,
    PrivacyReport, ReleaseEvidenceItem, ReleaseEvidenceStatus, ReleaseReadiness,
    ReleaseReadinessStatus, RowUniquenessSummary, UtilityMetric,
};
pub(crate) use sampling::{DETECTION_SAMPLE_ROW_FLOOR, DetectionCoverage};
pub use sampling::{
    DetectionCoverageSummary, DetectionCoverageUnit, MAX_PREVIEW_SAMPLE_COUNT,
    MAX_SAMPLE_ROW_COUNT, ParsedSample,
};
pub use workflow::{
    AnonymizeData, AnonymizeParams, ColumnPreview, HeadersData, PasteAnalyzeData,
    PasteAnalyzeParams, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PreflightData, PreflightMode, PreflightParams, PreviewData,
    PreviewParams, PreviewWarning, QuickGenerateParams, QuickTransformData, QuickTransformParams,
    SampleTransform, WarningSeverity,
};

#[cfg(test)]
mod tests;
