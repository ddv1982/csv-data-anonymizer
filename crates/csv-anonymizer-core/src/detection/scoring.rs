use std::cmp::Reverse;

use crate::types::{Confidence, DataType, DetectionResult, DetectionTrace, DetectionTraceItem};

use super::candidate::DetectorCandidate;

/// Ranked tie-breakers for candidate selection: confidence tier, evidence tier,
/// entity specificity, how many sampled values matched, and finally the
/// candidate's position in the priority list so that ties resolve to the
/// earlier, more conservative detector.
type DecisionKey = (u8, u8, u8, usize, Reverse<usize>);

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
        Reverse(index),
    )
}

/// Raise a confidence one tier, capped at High. Used when a header signal
/// agrees with an already-final validator selection: the header may lift
/// confidence by exactly one tier but can never suppress or replace it.
pub(in crate::detection) fn raise_one_tier(confidence: Confidence) -> Confidence {
    match confidence {
        Confidence::Low => Confidence::Medium,
        Confidence::Medium | Confidence::High => Confidence::High,
    }
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
    ) -> DetectorCandidate {
        DetectorCandidate::from_spec(DetectorCandidateSpec {
            data_type,
            reason: "test candidate".to_string(),
            match_count: 2,
            total_considered: 2,
            confidence: Confidence::High,
            evidence,
            specificity,
        })
    }

    #[test]
    fn validator_backed_candidates_win_ties() {
        let decision = DetectorDecision::select(vec![
            candidate(DataType::NumericId, DetectorEvidence::Pattern, 60),
            candidate(DataType::TaxId, DetectorEvidence::Validator, 95),
        ]);

        assert_eq!(decision.selected.unwrap().data_type, DataType::TaxId);
    }

    #[test]
    fn specific_entities_win_with_equal_evidence() {
        let decision = DetectorDecision::select(vec![
            candidate(DataType::Phone, DetectorEvidence::Pattern, 50),
            candidate(DataType::MacAddress, DetectorEvidence::Pattern, 80),
        ]);

        assert_eq!(decision.selected.unwrap().data_type, DataType::MacAddress);
    }

    /// Fully tied candidates resolve to the one listed first, so the priority
    /// order in `detection_priority()` stays the final word.
    #[test]
    fn earlier_candidate_wins_when_everything_else_ties() {
        let decision = DetectorDecision::select(vec![
            candidate(DataType::PostalCode, DetectorEvidence::Pattern, 70),
            candidate(DataType::NumericId, DetectorEvidence::Pattern, 70),
        ]);

        assert_eq!(decision.selected.unwrap().data_type, DataType::PostalCode);
    }
}
