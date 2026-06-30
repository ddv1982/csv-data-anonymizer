use std::cmp::Reverse;

use crate::types::{Confidence, DataType, DetectionResult, DetectionTrace, DetectionTraceItem};

use super::candidate::DetectorCandidate;

type DecisionKey = (
    u8,
    u8,
    u8,
    usize,
    usize,
    Reverse<usize>,
    Reverse<usize>,
    Reverse<usize>,
);

pub(in crate::detection) struct DetectorDecision {
    pub selected: Option<DetectorCandidate>,
    pub candidates: Vec<DetectorCandidate>,
}

impl DetectorDecision {
    pub(in crate::detection) fn select(candidates: Vec<DetectorCandidate>) -> Self {
        let selected = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.accepted)
            .max_by_key(|(index, candidate)| decision_key(candidate, *index))
            .map(|(_, candidate)| candidate.clone());

        Self {
            selected,
            candidates,
        }
    }

    pub(in crate::detection) fn trace_items(&self) -> Vec<DetectionTraceItem> {
        self.candidates
            .iter()
            .map(DetectorCandidate::trace_item)
            .collect()
    }
}

fn decision_key(candidate: &DetectorCandidate, index: usize) -> DecisionKey {
    (
        confidence_rank(candidate.confidence),
        candidate.evidence.rank(),
        candidate.specificity,
        candidate.match_count,
        candidate.span_len,
        Reverse(candidate.span_start),
        Reverse(candidate.order),
        Reverse(index),
    )
}

pub(in crate::detection) fn calculate_confidence(
    match_count: usize,
    total_non_empty: usize,
) -> Confidence {
    if total_non_empty == 0 {
        return Confidence::Low;
    }

    let percentage = match_count as f64 / total_non_empty as f64;
    if percentage >= 0.8 {
        Confidence::High
    } else if percentage >= 0.5 {
        Confidence::Medium
    } else {
        Confidence::Low
    }
}

fn confidence_rank(confidence: Confidence) -> u8 {
    match confidence {
        Confidence::High => 3,
        Confidence::Medium => 2,
        Confidence::Low => 1,
    }
}

pub(in crate::detection) fn detection_result(
    data_type: DataType,
    confidence: Confidence,
    sample_matches: usize,
    total_samples: usize,
    total_non_empty: usize,
    selected_reason: impl Into<String>,
    candidates: Vec<DetectionTraceItem>,
) -> DetectionResult {
    DetectionResult {
        data_type,
        confidence,
        sample_matches,
        total_samples,
        trace: Some(DetectionTrace {
            summary: detection_summary(data_type, confidence, sample_matches, total_non_empty),
            selected_reason: selected_reason.into(),
            total_non_empty,
            candidates,
        }),
    }
}

pub(in crate::detection) fn attach_single_trace(
    mut result: DetectionResult,
    total_non_empty: usize,
    selected_reason: impl Into<String>,
    reason: impl Into<String>,
) -> DetectionResult {
    let reason = reason.into();
    result.trace = Some(DetectionTrace {
        summary: detection_summary(
            result.data_type,
            result.confidence,
            result.sample_matches,
            total_non_empty,
        ),
        selected_reason: selected_reason.into(),
        total_non_empty,
        candidates: vec![trace_item(
            result.data_type,
            reason,
            result.sample_matches,
            total_non_empty,
            result.confidence,
            true,
        )],
    });
    result
}

pub(in crate::detection) fn trace_item(
    data_type: DataType,
    reason: impl Into<String>,
    match_count: usize,
    total_considered: usize,
    confidence: Confidence,
    accepted: bool,
) -> DetectionTraceItem {
    DetectionTraceItem {
        data_type,
        reason: reason.into(),
        match_count,
        total_considered,
        confidence,
        accepted,
    }
}

fn detection_summary(
    data_type: DataType,
    confidence: Confidence,
    sample_matches: usize,
    total_non_empty: usize,
) -> String {
    format!(
        "{data_type:?} selected with {confidence:?} confidence from {sample_matches}/{total_non_empty} non-empty sample value(s)."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::candidate::{DetectorCandidate, DetectorCandidateSpec, DetectorEvidence};

    fn candidate(
        data_type: DataType,
        evidence: DetectorEvidence,
        specificity: u8,
        order: usize,
    ) -> DetectorCandidate {
        DetectorCandidate::from_spec(DetectorCandidateSpec {
            data_type,
            reason: "test candidate".to_string(),
            match_count: 2,
            total_considered: 2,
            confidence: Confidence::High,
            evidence,
            specificity,
            order,
        })
    }

    #[test]
    fn validator_backed_candidates_win_ties() {
        let decision = DetectorDecision::select(vec![
            candidate(DataType::NumericId, DetectorEvidence::Pattern, 60, 0),
            candidate(DataType::TaxId, DetectorEvidence::Validator, 95, 1),
        ]);

        assert_eq!(decision.selected.unwrap().data_type, DataType::TaxId);
    }

    #[test]
    fn specific_entities_win_with_equal_evidence() {
        let decision = DetectorDecision::select(vec![
            candidate(DataType::Phone, DetectorEvidence::Pattern, 50, 0),
            candidate(DataType::MacAddress, DetectorEvidence::Pattern, 80, 1),
        ]);

        assert_eq!(decision.selected.unwrap().data_type, DataType::MacAddress);
    }

    #[test]
    fn longer_then_earlier_span_wins_with_equal_scores() {
        let decision = DetectorDecision::select(vec![
            candidate(DataType::Url, DetectorEvidence::Pattern, 70, 0).with_span(8, 10),
            candidate(DataType::Url, DetectorEvidence::Pattern, 70, 1).with_span(4, 10),
            candidate(DataType::Url, DetectorEvidence::Pattern, 70, 2).with_span(0, 8),
        ]);

        let selected = decision.selected.unwrap();
        assert_eq!(selected.span_start, 4);
        assert_eq!(selected.span_len, 10);
    }
}
