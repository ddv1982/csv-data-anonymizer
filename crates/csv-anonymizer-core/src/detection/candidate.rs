use crate::types::{Confidence, DataType, DetectionTraceItem};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::detection) enum DetectorEvidence {
    Header,
    Validator,
    Pattern,
    Shape,
}

impl DetectorEvidence {
    pub(in crate::detection) fn rank(self) -> u8 {
        match self {
            DetectorEvidence::Validator => 4,
            DetectorEvidence::Header => 3,
            DetectorEvidence::Pattern => 2,
            DetectorEvidence::Shape => 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::detection) struct DetectorCandidate {
    pub data_type: DataType,
    pub reason: String,
    pub match_count: usize,
    pub total_considered: usize,
    pub confidence: Confidence,
    pub accepted: bool,
    pub evidence: DetectorEvidence,
    pub specificity: u8,
    pub order: usize,
    pub span_start: usize,
    pub span_len: usize,
}

pub(in crate::detection) struct DetectorCandidateSpec {
    pub data_type: DataType,
    pub reason: String,
    pub match_count: usize,
    pub total_considered: usize,
    pub confidence: Confidence,
    pub evidence: DetectorEvidence,
    pub specificity: u8,
    pub order: usize,
}

impl DetectorCandidate {
    pub(in crate::detection) fn from_spec(spec: DetectorCandidateSpec) -> Self {
        Self {
            data_type: spec.data_type,
            reason: spec.reason,
            match_count: spec.match_count,
            total_considered: spec.total_considered,
            confidence: spec.confidence,
            accepted: spec.confidence != Confidence::Low,
            evidence: spec.evidence,
            specificity: spec.specificity,
            order: spec.order,
            span_start: usize::MAX,
            span_len: 0,
        }
    }

    #[cfg(test)]
    pub(in crate::detection) fn with_span(mut self, span_start: usize, span_len: usize) -> Self {
        self.span_start = span_start;
        self.span_len = span_len;
        self
    }

    pub(in crate::detection) fn trace_item(&self) -> DetectionTraceItem {
        DetectionTraceItem {
            data_type: self.data_type,
            reason: self.reason.clone(),
            match_count: self.match_count,
            total_considered: self.total_considered,
            confidence: self.confidence,
            accepted: self.accepted,
        }
    }
}
