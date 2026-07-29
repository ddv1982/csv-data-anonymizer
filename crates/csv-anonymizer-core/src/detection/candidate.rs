use crate::types::{Confidence, DataType, DetectionTraceItem};

/// How a candidate was established, strongest first. Column classification is
/// value-first, so header agreement is not an evidence tier here — it adjusts a
/// value-backed selection elsewhere rather than competing with one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::detection) enum DetectorEvidence {
    Validator,
    Pattern,
    Shape,
}

impl DetectorEvidence {
    pub(in crate::detection) fn rank(self) -> u8 {
        match self {
            DetectorEvidence::Validator => 3,
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
}

pub(in crate::detection) struct DetectorCandidateSpec {
    pub data_type: DataType,
    pub reason: String,
    pub match_count: usize,
    pub total_considered: usize,
    pub confidence: Confidence,
    pub evidence: DetectorEvidence,
    pub specificity: u8,
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
        }
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
