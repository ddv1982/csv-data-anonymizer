use crate::types::{Confidence, DataType, PrivacyFinding, PrivacyFindingKind};

use super::utf16_index_for_byte;

const MIN_ACCEPTED_SCORE_BASIS_POINTS: u16 = 8_000;
const MAX_CANDIDATES: usize = 10_000;

#[derive(Debug, Clone, Copy)]
pub struct CandidateCell<'a> {
    pub column_index: usize,
    pub row_index: usize,
    pub column_name: &'a str,
    pub text: &'a str,
}

#[derive(Debug)]
pub struct CandidateBatch<'a> {
    pub cells: Vec<CandidateCell<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateKind {
    PersonName,
    PrivateAddress,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub column_index: usize,
    pub row_index: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub kind: CandidateKind,
    /// Model score in basis points (`0..=10_000`).
    pub score_basis_points: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateBatchResult {
    pub model_version: Option<String>,
    pub coverage: CandidateDetectionCoverage,
    pub candidates: Vec<Candidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateDetectionCoverage {
    pub total_cells: usize,
    pub examined_cells: usize,
    pub skipped_oversized_cells: usize,
}

impl CandidateDetectionCoverage {
    pub fn complete(total_cells: usize) -> Self {
        Self {
            total_cells,
            examined_cells: total_cells,
            skipped_oversized_cells: 0,
        }
    }

    pub fn is_incomplete(self) -> bool {
        self.examined_cells < self.total_cells
    }
}

pub trait CandidateDetector {
    fn detector_id(&self) -> &str;

    fn detect(
        &mut self,
        batch: &CandidateBatch<'_>,
    ) -> std::result::Result<CandidateBatchResult, String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateRejectionReason {
    TooManyCandidates,
    UnknownCell,
    InvalidSpan,
    ScoreOutOfRange,
    BelowThreshold,
    OverlapsDeterministicEvidence,
    OverlapsCandidateEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateRejection {
    pub reason: CandidateRejectionReason,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateDetectorRunStatus {
    Disabled,
    Completed {
        detector_id: String,
        model_version: Option<String>,
        examined_cells: usize,
        accepted_candidates: usize,
        rejections: Vec<CandidateRejection>,
    },
    Incomplete {
        detector_id: String,
        model_version: Option<String>,
        total_cells: usize,
        examined_cells: usize,
        skipped_oversized_cells: usize,
        accepted_candidates: usize,
        rejections: Vec<CandidateRejection>,
    },
    Failed {
        detector_id: String,
        examined_cells: usize,
        message: String,
    },
}

pub(crate) struct ValidatedCandidates {
    pub(crate) findings_by_column: Vec<Vec<PrivacyFinding>>,
    pub(crate) accepted: usize,
    pub(crate) rejections: Vec<CandidateRejection>,
}

pub(crate) fn validate_candidates(
    batch: &CandidateBatch<'_>,
    result: CandidateBatchResult,
    deterministic_findings: &[Vec<PrivacyFinding>],
    detector_id: &str,
    column_count: usize,
) -> ValidatedCandidates {
    let mut findings_by_column: Vec<Vec<PrivacyFinding>> = vec![Vec::new(); column_count];
    let mut rejection_counts = [0usize; 7];
    let candidate_count = result.candidates.len();
    let mut candidates = result.candidates;
    candidates.sort_by(|left, right| {
        right
            .score_basis_points
            .cmp(&left.score_basis_points)
            .then_with(|| {
                right
                    .end_byte
                    .saturating_sub(right.start_byte)
                    .cmp(&left.end_byte.saturating_sub(left.start_byte))
            })
            .then_with(|| left.column_index.cmp(&right.column_index))
            .then_with(|| left.row_index.cmp(&right.row_index))
            .then_with(|| left.start_byte.cmp(&right.start_byte))
            .then_with(|| left.end_byte.cmp(&right.end_byte))
            .then_with(|| candidate_kind_order(left.kind).cmp(&candidate_kind_order(right.kind)))
    });

    for candidate in candidates.into_iter().take(MAX_CANDIDATES) {
        let Some(cell) = batch.cells.iter().find(|cell| {
            cell.column_index == candidate.column_index && cell.row_index == candidate.row_index
        }) else {
            rejection_counts[CandidateRejectionReason::UnknownCell as usize] += 1;
            continue;
        };
        if candidate.score_basis_points > 10_000 {
            rejection_counts[CandidateRejectionReason::ScoreOutOfRange as usize] += 1;
            continue;
        }
        if candidate.score_basis_points < MIN_ACCEPTED_SCORE_BASIS_POINTS {
            rejection_counts[CandidateRejectionReason::BelowThreshold as usize] += 1;
            continue;
        }
        if candidate.start_byte >= candidate.end_byte
            || candidate.end_byte > cell.text.len()
            || !cell.text.is_char_boundary(candidate.start_byte)
            || !cell.text.is_char_boundary(candidate.end_byte)
        {
            rejection_counts[CandidateRejectionReason::InvalidSpan as usize] += 1;
            continue;
        }

        let start = utf16_index_for_byte(cell.text, candidate.start_byte);
        let end = utf16_index_for_byte(cell.text, candidate.end_byte);
        let overlaps_deterministic = deterministic_findings
            .get(candidate.column_index)
            .into_iter()
            .flatten()
            .any(|finding| {
                finding.row_index == candidate.row_index
                    && start < finding.end
                    && finding.start < end
            });
        if overlaps_deterministic {
            rejection_counts[CandidateRejectionReason::OverlapsDeterministicEvidence as usize] += 1;
            continue;
        }
        let overlaps_candidate = findings_by_column[candidate.column_index]
            .iter()
            .any(|finding| {
                finding.row_index == candidate.row_index
                    && start < finding.end
                    && finding.start < end
            });
        if overlaps_candidate {
            rejection_counts[CandidateRejectionReason::OverlapsCandidateEvidence as usize] += 1;
            continue;
        }

        let (kind, data_type) = match candidate.kind {
            CandidateKind::PersonName => (PrivacyFindingKind::Person, DataType::FullName),
            CandidateKind::PrivateAddress => {
                (PrivacyFindingKind::PrivateAddress, DataType::Address)
            }
        };
        findings_by_column[candidate.column_index].push(PrivacyFinding {
            kind,
            data_type,
            row_index: candidate.row_index,
            start,
            end,
            match_value: cell.text[candidate.start_byte..candidate.end_byte].to_string(),
            sample_value: cell.text.to_string(),
            // Model output is additive evidence, but never validator-grade evidence.
            confidence: Confidence::Medium,
            score: 72,
            detector: format!("local-ner:{detector_id}"),
            reason: "Optional local named-entity detector identified sensitive text.".to_string(),
        });
    }

    if candidate_count > MAX_CANDIDATES {
        rejection_counts[CandidateRejectionReason::TooManyCandidates as usize] +=
            candidate_count - MAX_CANDIDATES;
    }
    let accepted = findings_by_column.iter().map(Vec::len).sum();
    let reasons = [
        CandidateRejectionReason::TooManyCandidates,
        CandidateRejectionReason::UnknownCell,
        CandidateRejectionReason::InvalidSpan,
        CandidateRejectionReason::ScoreOutOfRange,
        CandidateRejectionReason::BelowThreshold,
        CandidateRejectionReason::OverlapsDeterministicEvidence,
        CandidateRejectionReason::OverlapsCandidateEvidence,
    ];
    let rejections = reasons
        .into_iter()
        .enumerate()
        .filter_map(|(index, reason)| {
            let count = rejection_counts[index];
            (count > 0).then_some(CandidateRejection { reason, count })
        })
        .collect();

    ValidatedCandidates {
        findings_by_column,
        accepted,
        rejections,
    }
}

fn candidate_kind_order(kind: CandidateKind) -> u8 {
    match kind {
        CandidateKind::PersonName => 0,
        CandidateKind::PrivateAddress => 1,
    }
}

pub(crate) fn candidate_batch<'a>(
    headers: &'a [String],
    samples: &'a [Vec<String>],
) -> CandidateBatch<'a> {
    let cells = samples
        .iter()
        .enumerate()
        .flat_map(|(row_index, row)| {
            row.iter()
                .enumerate()
                .filter(|(_, text)| !text.is_empty() && !text.eq_ignore_ascii_case("null"))
                .filter_map(move |(column_index, text)| {
                    headers.get(column_index).map(|column_name| CandidateCell {
                        column_index,
                        row_index,
                        column_name,
                        text,
                    })
                })
        })
        .collect();
    CandidateBatch { cells }
}
