pub mod csv_io;
pub mod detection;
pub mod direct_input;
pub mod error;
mod file_ops;
mod hash;
pub mod metadata;
mod preview;
mod process_control;
mod release_report;
mod report_notes;
pub mod service;
pub mod smart;
pub mod strategies;
pub mod types;

pub use error::{AnonymizerError, Result};
pub use service::AnonymizerService;
pub use smart::{
    SmartReplacement, SmartReplacementMap, SmartReplacementProvider, SmartReplacementRequest,
};
pub use types::{
    AnonymizationStrategy, AnonymizeData, AnonymizeParams, ColumnControl, ColumnMetadata,
    ColumnPreview, ColumnReleaseReport, Confidence, DataType, DetectionResult, DetectionTrace,
    DetectionTraceItem, EmptyFormat, HeadersData, ParsedSample, PasteAnalyzeData,
    PasteAnalyzeParams, PasteDataFormat, PastePreviewParams, PasteTransformData,
    PasteTransformParams, PiiRisk, PreflightData, PreflightMode, PreflightParams, PreviewData,
    PreviewParams, PreviewWarning, PrivacyEvidenceSummary, PrivacyFinding, PrivacyFindingKind,
    PrivacyReport, ProcessControl, ProcessOptions, ProcessProgress, ProcessResult,
    QuickGenerateParams, QuickTransformData, QuickTransformParams, ReleaseEvidenceItem,
    ReleaseEvidenceStatus, ReleaseReadiness, ReleaseReadinessStatus, SampleTransform,
    SmartReplacementEntry, SmartReplacementRejectionCount, SmartReplacementRejectionReason,
    TransformReport, UtilityMetric, WarningSeverity,
};
