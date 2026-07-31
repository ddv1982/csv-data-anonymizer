use crate::local_ai::DEFAULT_OLLAMA_MODEL;
use csv_anonymizer_core::{
    DetectionRunSummary, LocalNerRunStatus, PasteDataFormat, PreparedAnalysisSnapshot,
    SourceFingerprint,
};
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

const PREPARED_ANALYSIS_CACHE_LIMIT: usize = 16;

pub(super) fn selected_candidate_ids(
    snapshot: &PreparedAnalysisSnapshot,
    selected_columns: &[usize],
) -> Vec<String> {
    snapshot
        .candidate_evidence
        .iter()
        .filter(|evidence| selected_columns.contains(&evidence.column_index))
        .map(|evidence| evidence.id.clone())
        .collect()
}

fn prepared_analysis_cache() -> &'static Mutex<VecDeque<PreparedAnalysisSnapshot>> {
    static CACHE: OnceLock<Mutex<VecDeque<PreparedAnalysisSnapshot>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(VecDeque::new()))
}

pub(super) fn register_prepared_analysis(
    snapshot: &PreparedAnalysisSnapshot,
) -> Result<(), String> {
    let mut cache = prepared_analysis_cache()
        .lock()
        .map_err(|_| "Prepared analysis cache is unavailable.".to_string())?;
    if !cache.iter().any(|issued| issued == snapshot) {
        cache.push_back(snapshot.clone());
    }
    while cache.len() > PREPARED_ANALYSIS_CACHE_LIMIT {
        cache.pop_front();
    }
    Ok(())
}

fn verify_backend_issued_snapshot(snapshot: &PreparedAnalysisSnapshot) -> Result<(), String> {
    let cache = prepared_analysis_cache()
        .lock()
        .map_err(|_| "Prepared analysis cache is unavailable.".to_string())?;
    if cache.iter().any(|issued| issued == snapshot) {
        Ok(())
    } else {
        Err(
            "Analyze the source again: the prepared analysis was not issued by this app session."
                .to_string(),
        )
    }
}

pub(crate) fn snapshot_detection_summary(
    snapshot: &PreparedAnalysisSnapshot,
) -> DetectionRunSummary {
    snapshot.detection_run_summary.clone()
}

pub(crate) fn require_prepared_analysis(
    local_ner_enabled: bool,
    snapshot: Option<&PreparedAnalysisSnapshot>,
) -> Result<(), String> {
    match (local_ner_enabled, snapshot) {
        (true, None) => {
            Err("Analyze the source again before using Local AI detection results.".to_string())
        }
        (false, Some(_)) => Err(
            "Analyze the source again after changing the Local AI detection setting.".to_string(),
        ),
        _ => Ok(()),
    }
}

pub(crate) fn require_snapshot_model(
    snapshot: Option<&PreparedAnalysisSnapshot>,
    configured_model: &str,
) -> Result<(), String> {
    let Some(snapshot) = snapshot else {
        return Ok(());
    };
    let configured_model = if configured_model.trim().is_empty() {
        DEFAULT_OLLAMA_MODEL
    } else {
        configured_model.trim()
    };
    if matches!(
        snapshot.detector.status,
        LocalNerRunStatus::Completed | LocalNerRunStatus::Incomplete
    ) && snapshot.detector.model_version.as_deref() != Some(configured_model)
    {
        Err("Analyze the source again after changing the Local AI model.".to_string())
    } else {
        Ok(())
    }
}

fn validate_file_snapshot_fingerprint(
    snapshot: &PreparedAnalysisSnapshot,
    file_path: &std::path::Path,
    sample_row_count: usize,
    selected_columns: &[usize],
    source_fingerprint: &str,
) -> Result<(), String> {
    let source_identity = file_path.to_string_lossy();
    let confirmed = selected_candidate_ids(snapshot, selected_columns);
    snapshot
        .validate_source_fingerprint(
            &source_identity,
            "csv",
            source_fingerprint,
            sample_row_count,
            &confirmed,
        )
        .map_err(|error| format!("Analyze the source again: {error}"))?;
    Ok(())
}

pub(crate) struct ValidatedFileInput {
    original_path: PathBuf,
    staged_path: Option<tempfile::TempPath>,
}

impl ValidatedFileInput {
    pub(crate) fn prepare(
        snapshot: Option<&PreparedAnalysisSnapshot>,
        original_path: PathBuf,
        sample_row_count: usize,
        selected_columns: &[usize],
    ) -> Result<Self, String> {
        let staged_path = snapshot
            .map(|snapshot| {
                stage_validated_file_snapshot(
                    snapshot,
                    &original_path,
                    sample_row_count,
                    selected_columns,
                )
            })
            .transpose()?;
        Ok(Self {
            original_path,
            staged_path,
        })
    }

    pub(crate) fn original_path(&self) -> &std::path::Path {
        &self.original_path
    }

    pub(crate) fn processing_path(&self) -> PathBuf {
        self.staged_path
            .as_deref()
            .unwrap_or(&self.original_path)
            .to_path_buf()
    }
}

/// Copies the exact snapshot-validated bytes to a private file for a background job.
///
/// Keeping the returned path alive pins the staged source until processing finishes,
/// closing the gap between validation and the service opening the input.
pub(crate) fn stage_validated_file_snapshot(
    snapshot: &PreparedAnalysisSnapshot,
    file_path: &std::path::Path,
    sample_row_count: usize,
    selected_columns: &[usize],
) -> Result<tempfile::TempPath, String> {
    verify_backend_issued_snapshot(snapshot)?;
    let (staged, source_fingerprint) = stage_private_csv_file(file_path)?;
    validate_file_snapshot_fingerprint(
        snapshot,
        file_path,
        sample_row_count,
        selected_columns,
        &source_fingerprint,
    )?;
    Ok(staged)
}

pub(super) fn validate_paste_snapshot(
    snapshot: &PreparedAnalysisSnapshot,
    content: &str,
    format: PasteDataFormat,
    sample_row_count: usize,
    selected_columns: &[usize],
) -> Result<(), String> {
    verify_backend_issued_snapshot(snapshot)?;
    let confirmed = selected_candidate_ids(snapshot, selected_columns);
    let requested_format = if format == PasteDataFormat::Auto {
        snapshot.format.as_str().to_string()
    } else {
        paste_format_name(format).to_string()
    };
    snapshot
        .validate(
            "paste",
            &requested_format,
            content.as_bytes(),
            sample_row_count,
            &confirmed,
        )
        .map_err(|error| format!("Analyze the pasted data again: {error}"))?;
    Ok(())
}

pub(super) fn paste_format_name(format: PasteDataFormat) -> &'static str {
    match format {
        PasteDataFormat::Auto => "auto",
        PasteDataFormat::Csv => "csv",
        PasteDataFormat::Json => "json",
        PasteDataFormat::Xml => "xml",
        PasteDataFormat::Yaml => "yaml",
        PasteDataFormat::PlainText => "plainText",
        PasteDataFormat::Logs => "logs",
    }
}

pub(super) fn stage_private_csv_file(
    source_path: &std::path::Path,
) -> Result<(tempfile::TempPath, String), String> {
    let mut source = std::fs::File::open(source_path)
        .map_err(|error| format!("Could not open source for private staging: {error}"))?;
    let mut staged = tempfile::Builder::new()
        .prefix("csv-anonymizer-validated-")
        .suffix(".csv")
        .tempfile()
        .map_err(|error| format!("Could not create private staged source: {error}"))?;
    let mut fingerprint = SourceFingerprint::default();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|error| format!("Could not read source for private staging: {error}"))?;
        if read == 0 {
            break;
        }
        fingerprint.update(&buffer[..read]);
        staged
            .write_all(&buffer[..read])
            .map_err(|error| format!("Could not write private staged source: {error}"))?;
    }
    staged
        .as_file_mut()
        .sync_all()
        .map_err(|error| format!("Could not write private staged source: {error}"))?;
    Ok((staged.into_temp_path(), fingerprint.finish()))
}
