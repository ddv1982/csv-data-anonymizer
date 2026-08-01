//! Anonymizes tabular data, and reports what the result still exposes.
//!
//! The crate's surface is the re-export list below, not its module tree. Every module here is
//! `pub(crate)` except the two an outside caller genuinely reaches into, so adding a `pub fn`
//! inside one does not widen the API by accident — and, more usefully, so `dead_code` can see
//! the whole crate. A `pub` item in a library crate is never reported as dead, which over
//! roughly 2,100 lines of module surface meant the lint had nothing to say about any of it.
//!
//! The two exceptions:
//!
//! - [`direct_input`], reached by `src-tauri` for the paste and quick-generate paths.
//! - [`detection`], reached by `benches/detector_matrix.rs` and nothing else. It stays public
//!   for the benchmark alone; no application code uses it.

pub(crate) mod csv_io;
pub mod detection;
pub mod direct_input;
pub mod error;
mod file_ops;
pub(crate) mod metadata;
mod prepared_snapshot;
mod preview;
mod process_control;
mod random;
mod release_report;
mod report_notes;
mod sampling;
pub(crate) mod service;
pub(crate) mod smart;
pub(crate) mod strategies;
#[cfg(test)]
mod test_support;
pub(crate) mod types;
mod uniqueness;

pub use error::{AnonymizerError, Result};
pub use metadata::should_auto_select_column;
pub use prepared_snapshot::{
    PREPARED_ANALYSIS_VERSION, PreparedAnalysisSnapshot, PreparedCandidateEvidence,
    PreparedDetectorIdentity, PreparedSnapshotError, SourceFingerprint, ValidatedPreparedAnalysis,
};
pub use service::AnonymizerService;
pub use smart::{SmartReplacement, SmartReplacementProvider, SmartReplacementRequest};
pub use strategies::TokenizationKey;
pub use types::{
    AnonymizationStrategy, AnonymizeData, AnonymizeParams, ColumnControl, ColumnEvidenceProfile,
    ColumnMetadata, ColumnPreview, ColumnReleaseReport, ColumnReviewReason,
    ColumnValueDistribution, Confidence, DataType, DetectionCoverageSummary, DetectionCoverageUnit,
    DetectionResult, DetectionRunSummary, DetectionTrace, DetectionTraceItem,
    DeterministicDetectionStatus, DropColumnEffect, EmptyFormat, EvidenceDisposition,
    FormatEvidence, FormatEvidenceBasis, HeadersData, LocalNerRunStatus, MAX_PREVIEW_SAMPLE_COUNT,
    MAX_SAMPLE_ROW_COUNT, MatchedColumn, MatchedPart, ParsedSample, PasteAnalyzeData,
    PasteAnalyzeParams, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PiiRisk, PreflightData, PreflightMode, PreflightParams, PreviewData,
    PreviewParams, PreviewWarning, PrivacyDecision, PrivacyEvidenceSummary, PrivacyFinding,
    PrivacyFindingKind, PrivacyReport, ProcessControl, ProcessOptions, ProcessProgress,
    ProcessResult, QuickGenerateParams, QuickTransformData, QuickTransformParams,
    RedactionDecision, RedactionPlaceholderSource, ReleaseEvidenceItem, ReleaseEvidenceStatus,
    ReleaseReadiness, ReleaseReadinessStatus, RowUniquenessSummary, SampleTransform,
    SemanticDecision, SemanticSpecificity, SemanticStatus, SmartReplacementEntry,
    SmartReplacementRejectionCount, SmartReplacementRejectionReason, TransformContext,
    TransformReport, UtilityMetric, WarningSeverity,
};
