use crate::TokenizationKey;
use crate::detection::CandidateDetector;
use crate::smart::SmartReplacementProvider;
use crate::types::{DETECTION_SAMPLE_ROW_FLOOR, ProcessControl};

pub struct CsvAnalysisOptions<'detector> {
    pub sample_rows: usize,
    pub candidate_detector: Option<&'detector mut dyn CandidateDetector>,
}

impl Default for CsvAnalysisOptions<'_> {
    fn default() -> Self {
        Self {
            sample_rows: DETECTION_SAMPLE_ROW_FLOOR,
            candidate_detector: None,
        }
    }
}

#[derive(Default)]
pub struct TransformRuntime<'provider, 'key> {
    pub provider: Option<&'provider mut dyn SmartReplacementProvider>,
    pub tokenization_key: Option<&'key TokenizationKey>,
}

pub struct CsvRunOptions<'control, 'callbacks, 'provider, 'key> {
    pub sample_rows: usize,
    pub control: Option<&'control mut ProcessControl<'callbacks>>,
    pub transform: TransformRuntime<'provider, 'key>,
}

impl Default for CsvRunOptions<'_, '_, '_, '_> {
    fn default() -> Self {
        Self {
            sample_rows: DETECTION_SAMPLE_ROW_FLOOR,
            control: None,
            transform: TransformRuntime::default(),
        }
    }
}
