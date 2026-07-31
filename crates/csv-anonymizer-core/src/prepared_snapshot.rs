use crate::types::{
    ColumnMetadata, DataType, DetectionRunSummary, LocalNerRunStatus, PrivacyFindingKind,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

pub const PREPARED_ANALYSIS_VERSION: u16 = 2;

/// Incremental form of the snapshot fingerprint, suitable for streaming files.
#[derive(Debug, Clone)]
pub struct SourceFingerprint {
    first: u64,
    second: u64,
}

impl Default for SourceFingerprint {
    fn default() -> Self {
        Self {
            first: 0xcbf29ce484222325,
            second: 0x84222325cbf29ce4,
        }
    }
}

impl SourceFingerprint {
    pub fn update(&mut self, content: &[u8]) {
        for byte in content {
            self.first ^= u64::from(*byte);
            self.first = self.first.wrapping_mul(0x100000001b3);
            self.second ^= u64::from(*byte);
            self.second = self.second.wrapping_mul(0x100000001b3);
        }
    }

    pub fn finish(self) -> String {
        format!("fnv128:{:016x}{:016x}", self.first, self.second)
    }
}

/// Immutable handoff between analysis and a later preview or transform.
///
/// The checksum is an accidental-tamper guard, not a signature. A trust boundary
/// that accepts snapshots from an untrusted process must authenticate the serialized
/// snapshot outside this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedAnalysisSnapshot {
    pub version: u16,
    pub source_identity: String,
    pub source_fingerprint: String,
    pub format: String,
    pub sample_row_count: usize,
    pub columns: Vec<ColumnMetadata>,
    pub detector: PreparedDetectorIdentity,
    pub detection_run_summary: DetectionRunSummary,
    pub candidate_evidence: Vec<PreparedCandidateEvidence>,
    pub integrity_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedDetectorIdentity {
    pub status: LocalNerRunStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detector_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedCandidateEvidence {
    pub id: String,
    pub column_index: usize,
    pub row_index: usize,
    /// UTF-16 offsets, matching `PrivacyFinding`.
    pub start: usize,
    pub end: usize,
    pub kind: PrivacyFindingKind,
    pub data_type: DataType,
    pub match_value: String,
    pub sample_value: String,
    pub detector: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedSnapshotError {
    VersionMismatch,
    SourceIdentityMismatch,
    SourceContentMismatch,
    FormatMismatch,
    SampleRowCountMismatch,
    IntegrityMismatch,
    InvalidSchema,
    UnknownColumn,
    InvalidEvidence,
    DuplicateEvidence,
    UnconfirmedEvidence,
}

impl fmt::Display for PreparedSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::VersionMismatch => "prepared analysis version is not supported",
            Self::SourceIdentityMismatch => "prepared analysis belongs to a different source",
            Self::SourceContentMismatch => "source content changed after analysis",
            Self::FormatMismatch => "prepared analysis uses a different input format",
            Self::SampleRowCountMismatch => {
                "prepared analysis uses a different detection sample size"
            }
            Self::IntegrityMismatch => "prepared analysis payload was modified",
            Self::InvalidSchema => "prepared analysis column schema is invalid",
            Self::UnknownColumn => "prepared analysis evidence refers to an unknown column",
            Self::InvalidEvidence => "prepared analysis contains invalid candidate evidence",
            Self::DuplicateEvidence => "prepared analysis contains duplicate candidate evidence",
            Self::UnconfirmedEvidence => "confirmed candidate evidence is missing or unknown",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for PreparedSnapshotError {}

/// A validated replay. Candidate evidence remains inert unless its id appears in
/// `confirmed_candidate_ids`; selecting a column remains a separate explicit act.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedPreparedAnalysis {
    columns: Vec<ColumnMetadata>,
    confirmed_candidate_ids: HashSet<String>,
}

impl ValidatedPreparedAnalysis {
    pub fn columns(&self) -> &[ColumnMetadata] {
        &self.columns
    }

    pub fn confirmed_candidate_ids(&self) -> &HashSet<String> {
        &self.confirmed_candidate_ids
    }

    /// Returns snapshot metadata with only caller-selected columns activated.
    ///
    /// Candidate evidence never auto-selects a column. A candidate-only column can
    /// therefore affect a transform only after both snapshot validation and explicit
    /// column selection; confirmation is retained for the caller's audit/report path.
    pub fn columns_for_selection(
        &self,
        selected_columns: &[usize],
    ) -> Result<Vec<ColumnMetadata>, PreparedSnapshotError> {
        let selected: HashSet<_> = selected_columns.iter().copied().collect();
        if selected
            .iter()
            .any(|index| !self.columns.iter().any(|column| column.index == *index))
        {
            return Err(PreparedSnapshotError::UnknownColumn);
        }
        Ok(self
            .columns
            .iter()
            .cloned()
            .map(|mut column| {
                column.is_selected = selected.contains(&column.index);
                column
            })
            .collect())
    }
}

impl PreparedAnalysisSnapshot {
    pub fn new(
        source_identity: impl Into<String>,
        format: impl Into<String>,
        source_content: &[u8],
        sample_row_count: usize,
        columns: Vec<ColumnMetadata>,
        run: &DetectionRunSummary,
    ) -> Result<Self, PreparedSnapshotError> {
        let mut fingerprint = SourceFingerprint::default();
        fingerprint.update(source_content);
        Self::new_with_source_fingerprint(
            source_identity,
            format,
            fingerprint.finish(),
            sample_row_count,
            columns,
            run,
        )
    }

    pub fn new_with_source_fingerprint(
        source_identity: impl Into<String>,
        format: impl Into<String>,
        source_fingerprint: String,
        sample_row_count: usize,
        columns: Vec<ColumnMetadata>,
        run: &DetectionRunSummary,
    ) -> Result<Self, PreparedSnapshotError> {
        let detector = PreparedDetectorIdentity {
            status: run.local_ner,
            detector_id: run.detector_id.clone(),
            model_version: run.model_version.clone(),
        };
        let candidate_evidence = collect_candidate_evidence(&columns);
        let mut snapshot = Self {
            version: PREPARED_ANALYSIS_VERSION,
            source_identity: source_identity.into(),
            source_fingerprint,
            format: format.into(),
            sample_row_count,
            columns,
            detector,
            detection_run_summary: run.clone(),
            candidate_evidence,
            integrity_checksum: String::new(),
        };
        snapshot.validate_structure()?;
        snapshot.integrity_checksum = snapshot.compute_integrity();
        Ok(snapshot)
    }

    pub fn validate(
        &self,
        source_identity: &str,
        format: &str,
        source_content: &[u8],
        sample_row_count: usize,
        confirmed_candidate_ids: &[String],
    ) -> Result<ValidatedPreparedAnalysis, PreparedSnapshotError> {
        let mut fingerprint = SourceFingerprint::default();
        fingerprint.update(source_content);
        self.validate_source_fingerprint(
            source_identity,
            format,
            &fingerprint.finish(),
            sample_row_count,
            confirmed_candidate_ids,
        )
    }

    pub fn validate_source_fingerprint(
        &self,
        source_identity: &str,
        format: &str,
        source_fingerprint: &str,
        sample_row_count: usize,
        confirmed_candidate_ids: &[String],
    ) -> Result<ValidatedPreparedAnalysis, PreparedSnapshotError> {
        if self.version != PREPARED_ANALYSIS_VERSION {
            return Err(PreparedSnapshotError::VersionMismatch);
        }
        if self.source_identity != source_identity {
            return Err(PreparedSnapshotError::SourceIdentityMismatch);
        }
        if self.format != format {
            return Err(PreparedSnapshotError::FormatMismatch);
        }
        if self.sample_row_count != sample_row_count {
            return Err(PreparedSnapshotError::SampleRowCountMismatch);
        }
        if self.source_fingerprint != source_fingerprint {
            return Err(PreparedSnapshotError::SourceContentMismatch);
        }
        if self.integrity_checksum != self.compute_integrity() {
            return Err(PreparedSnapshotError::IntegrityMismatch);
        }
        self.validate_structure()?;

        let known: HashSet<_> = self
            .candidate_evidence
            .iter()
            .map(|evidence| evidence.id.as_str())
            .collect();
        let confirmed: HashSet<_> = confirmed_candidate_ids.iter().cloned().collect();
        if confirmed.len() != confirmed_candidate_ids.len()
            || confirmed.iter().any(|id| !known.contains(id.as_str()))
        {
            return Err(PreparedSnapshotError::UnconfirmedEvidence);
        }
        Ok(ValidatedPreparedAnalysis {
            columns: self.columns.clone(),
            confirmed_candidate_ids: confirmed,
        })
    }

    fn validate_structure(&self) -> Result<(), PreparedSnapshotError> {
        let mut column_indices = HashSet::new();
        for (position, column) in self.columns.iter().enumerate() {
            if column.index != position || !column_indices.insert(column.index) {
                return Err(PreparedSnapshotError::InvalidSchema);
            }
        }

        let detector_prefix = self
            .detector
            .detector_id
            .as_deref()
            .map(|id| format!("local-ner:{id}"));
        let detector_produced_evidence = matches!(
            self.detector.status,
            LocalNerRunStatus::Completed | LocalNerRunStatus::Incomplete
        );
        if detector_produced_evidence && detector_prefix.is_none() {
            return Err(PreparedSnapshotError::InvalidEvidence);
        }
        if self.detection_run_summary.local_ner != self.detector.status
            || self.detection_run_summary.detector_id != self.detector.detector_id
            || self.detection_run_summary.model_version != self.detector.model_version
        {
            return Err(PreparedSnapshotError::InvalidEvidence);
        }
        if !detector_produced_evidence && !self.candidate_evidence.is_empty() {
            return Err(PreparedSnapshotError::InvalidEvidence);
        }

        let mut ids = HashSet::new();
        let mut coordinates = HashSet::new();
        let mut accepted_spans = Vec::new();
        for evidence in &self.candidate_evidence {
            if !ids.insert(evidence.id.as_str())
                || !coordinates.insert((
                    evidence.column_index,
                    evidence.row_index,
                    evidence.start,
                    evidence.end,
                    evidence.kind,
                ))
            {
                return Err(PreparedSnapshotError::DuplicateEvidence);
            }
            let Some(column) = self.columns.get(evidence.column_index) else {
                return Err(PreparedSnapshotError::UnknownColumn);
            };
            if column.index != evidence.column_index
                || evidence.detector != detector_prefix.as_deref().unwrap_or_default()
                || evidence.start >= evidence.end
                || utf16_slice(&evidence.sample_value, evidence.start, evidence.end)
                    != Some(evidence.match_value.as_str())
            {
                return Err(PreparedSnapshotError::InvalidEvidence);
            }
            let matching_finding = column.privacy_findings.iter().any(|finding| {
                finding.row_index == evidence.row_index
                    && finding.start == evidence.start
                    && finding.end == evidence.end
                    && finding.kind == evidence.kind
                    && finding.data_type == evidence.data_type
                    && finding.match_value == evidence.match_value
                    && finding.sample_value == evidence.sample_value
                    && finding.detector == evidence.detector
            });
            if !matching_finding {
                return Err(PreparedSnapshotError::InvalidEvidence);
            }
            if accepted_spans
                .iter()
                .any(|&(column_index, row_index, start, end)| {
                    column_index == evidence.column_index
                        && row_index == evidence.row_index
                        && evidence.start < end
                        && start < evidence.end
                })
            {
                return Err(PreparedSnapshotError::InvalidEvidence);
            }
            accepted_spans.push((
                evidence.column_index,
                evidence.row_index,
                evidence.start,
                evidence.end,
            ));
        }
        Ok(())
    }

    fn compute_integrity(&self) -> String {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.version.to_le_bytes());
        append(&mut bytes, self.source_identity.as_bytes());
        append(&mut bytes, self.source_fingerprint.as_bytes());
        append(&mut bytes, self.format.as_bytes());
        bytes.extend_from_slice(&(self.sample_row_count as u64).to_le_bytes());
        append(
            &mut bytes,
            &serde_json::to_vec(&self.columns).expect("serializing columns cannot fail"),
        );
        append(
            &mut bytes,
            &serde_json::to_vec(&self.detector).expect("serializing detector cannot fail"),
        );
        append(
            &mut bytes,
            &serde_json::to_vec(&self.detection_run_summary)
                .expect("serializing detection run summary cannot fail"),
        );
        append(
            &mut bytes,
            &serde_json::to_vec(&self.candidate_evidence)
                .expect("serializing candidate evidence cannot fail"),
        );
        let mut fingerprint = SourceFingerprint::default();
        fingerprint.update(&bytes);
        fingerprint.finish()
    }
}

fn collect_candidate_evidence(columns: &[ColumnMetadata]) -> Vec<PreparedCandidateEvidence> {
    let mut evidence = Vec::new();
    for column in columns {
        for finding in &column.privacy_findings {
            if !finding.detector.starts_with("local-ner:") {
                continue;
            }
            let id = format!(
                "c{}-r{}-{}-{}-{}",
                column.index,
                finding.row_index,
                finding.start,
                finding.end,
                evidence.len()
            );
            evidence.push(PreparedCandidateEvidence {
                id,
                column_index: column.index,
                row_index: finding.row_index,
                start: finding.start,
                end: finding.end,
                kind: finding.kind,
                data_type: finding.data_type,
                match_value: finding.match_value.clone(),
                sample_value: finding.sample_value.clone(),
                detector: finding.detector.clone(),
            });
        }
    }
    evidence
}

fn append(destination: &mut Vec<u8>, value: &[u8]) {
    destination.extend_from_slice(&(value.len() as u64).to_le_bytes());
    destination.extend_from_slice(value);
}

fn utf16_slice(value: &str, start: usize, end: usize) -> Option<&str> {
    let mut start_byte = None;
    let mut end_byte = None;
    let mut utf16_offset = 0;
    for (byte, character) in value.char_indices() {
        if utf16_offset == start {
            start_byte = Some(byte);
        }
        if utf16_offset == end {
            end_byte = Some(byte);
            break;
        }
        utf16_offset += character.len_utf16();
    }
    if utf16_offset == start {
        start_byte.get_or_insert(value.len());
    }
    if utf16_offset == end {
        end_byte.get_or_insert(value.len());
    }
    Some(&value[start_byte?..end_byte?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        AnonymizationStrategy, ColumnReviewReason, ColumnValueDistribution, Confidence,
        DetectionRunSummary, EmptyFormat, PiiRisk, PrivacyFinding,
    };

    fn snapshot() -> PreparedAnalysisSnapshot {
        let finding = PrivacyFinding {
            kind: PrivacyFindingKind::Person,
            data_type: DataType::FullName,
            row_index: 0,
            start: 0,
            end: 4,
            match_value: "José".into(),
            sample_value: "José lives here".into(),
            confidence: Confidence::Medium,
            score: 72,
            detector: "local-ner:test".into(),
            reason: "test".into(),
        };
        let column = ColumnMetadata {
            name: "notes".into(),
            header_label_is_ambiguous: false,
            source_path: None,
            index: 0,
            detected_type: DataType::String,
            confidence: Confidence::Low,
            detection_trace: None,
            privacy_findings: vec![finding],
            privacy_evidence: vec![],
            review_reasons: vec![ColumnReviewReason::AmbiguousContext],
            pii_risk: PiiRisk::Low,
            sample_values: vec!["José lives here".into()],
            sample_value_distribution: ColumnValueDistribution::default(),
            empty_format: EmptyFormat::EmptyString,
            is_selected: false,
            strategy: AnonymizationStrategy::Auto,
        };
        PreparedAnalysisSnapshot::new(
            "/tmp/input.csv",
            "csv",
            b"notes\nJose lives here\n",
            100,
            vec![column],
            &DetectionRunSummary {
                local_ner: LocalNerRunStatus::Completed,
                detector_id: Some("test".into()),
                model_version: Some("1".into()),
                accepted_candidates: 1,
                ..DetectionRunSummary::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn incremental_fingerprint_is_independent_of_chunk_boundaries() {
        let content = b"a source large enough to split several different ways";
        let mut whole = SourceFingerprint::default();
        whole.update(content);
        let mut chunked = SourceFingerprint::default();
        for chunk in content.chunks(3) {
            chunked.update(chunk);
        }

        assert_eq!(whole.finish(), chunked.finish());
    }

    #[test]
    fn snapshot_rejects_overlapping_candidate_evidence() {
        let mut first = snapshot();
        let mut overlapping = first.columns[0].privacy_findings[0].clone();
        overlapping.kind = PrivacyFindingKind::PrivateAddress;
        overlapping.data_type = DataType::Address;
        overlapping.start = 1;
        overlapping.match_value = "osé".into();
        first.columns[0].privacy_findings.push(overlapping);

        assert!(matches!(
            PreparedAnalysisSnapshot::new_with_source_fingerprint(
                first.source_identity,
                first.format,
                first.source_fingerprint,
                first.sample_row_count,
                first.columns,
                &first.detection_run_summary,
            ),
            Err(PreparedSnapshotError::InvalidEvidence)
        ));
    }

    #[test]
    fn validates_and_requires_explicit_selection() {
        let snapshot = snapshot();
        let id = snapshot.candidate_evidence[0].id.clone();
        let replay = snapshot
            .validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[id],
            )
            .unwrap();
        assert!(!replay.columns()[0].is_selected);
        assert!(
            replay
                .columns_for_selection(&[0])
                .unwrap()
                .first()
                .unwrap()
                .is_selected
        );
    }

    #[test]
    fn rejects_changed_content_and_source_identity() {
        let snapshot = snapshot();
        assert_eq!(
            snapshot.validate(
                "/tmp/other.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::SourceIdentityMismatch)
        );
        assert_eq!(
            snapshot.validate("/tmp/input.csv", "csv", b"changed", 100, &[]),
            Err(PreparedSnapshotError::SourceContentMismatch)
        );
    }

    #[test]
    fn rejects_schema_version_and_format_mismatch() {
        let snapshot = snapshot();
        let mut version = snapshot.clone();
        version.version += 1;
        assert_eq!(
            version.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::VersionMismatch)
        );
        assert_eq!(
            snapshot.validate(
                "/tmp/input.csv",
                "json",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::FormatMismatch)
        );
        assert_eq!(
            snapshot.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                101,
                &[],
            ),
            Err(PreparedSnapshotError::SampleRowCountMismatch)
        );
        let mut schema = snapshot.clone();
        schema.columns[0].index = 2;
        schema.integrity_checksum = schema.compute_integrity();
        assert_eq!(
            schema.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::InvalidSchema)
        );
    }

    #[test]
    fn rejects_unknown_duplicate_and_invalid_evidence() {
        let base = snapshot();
        let mut unknown = base.clone();
        unknown.candidate_evidence[0].column_index = 5;
        unknown.integrity_checksum = unknown.compute_integrity();
        assert_eq!(
            unknown.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::UnknownColumn)
        );

        let mut duplicate = base.clone();
        duplicate
            .candidate_evidence
            .push(duplicate.candidate_evidence[0].clone());
        duplicate.integrity_checksum = duplicate.compute_integrity();
        assert_eq!(
            duplicate.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::DuplicateEvidence)
        );

        let mut invalid = base;
        invalid.candidate_evidence[0].end = 3;
        invalid.integrity_checksum = invalid.compute_integrity();
        assert_eq!(
            invalid.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::InvalidEvidence)
        );
    }

    #[test]
    fn detects_payload_tampering_and_unknown_confirmation() {
        let mut tampered = snapshot();
        tampered.columns[0].name = "tampered".into();
        assert_eq!(
            tampered.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &[],
            ),
            Err(PreparedSnapshotError::IntegrityMismatch)
        );

        let snapshot = snapshot();
        assert_eq!(
            snapshot.validate(
                "/tmp/input.csv",
                "csv",
                b"notes\nJose lives here\n",
                100,
                &["missing".into()]
            ),
            Err(PreparedSnapshotError::UnconfirmedEvidence)
        );
    }
}
