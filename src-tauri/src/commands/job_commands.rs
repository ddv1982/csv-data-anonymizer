use super::csv::{
    ValidatedFileInput, require_prepared_analysis, require_snapshot_model,
    snapshot_detection_summary,
};
use super::shared::authorize_or_confirm_output_file;
use crate::jobs::{AnonymizeJobStatus, AnonymizeJobStore, run_anonymize_job};
use crate::local_ai::LocalAiRequest;
use crate::path_access::PathAccess;
use crate::settings::{MAX_SAMPLE_ROW_COUNT, SettingsStore, validate_sample_count};
use csv_anonymizer_core::{
    AnonymizeParams, ColumnControl, PreparedAnalysisSnapshot, SmartReplacementEntry,
};
use serde::Deserialize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartAnonymizeJobRequest {
    pub file_path: PathBuf,
    pub output_path: PathBuf,
    pub columns: Vec<usize>,
    #[serde(default)]
    pub controls: Vec<ColumnControl>,
    pub force: bool,
    pub sample_row_count: usize,
    pub total_row_count: Option<usize>,
    #[serde(default)]
    pub preview_smart_replacements: Vec<SmartReplacementEntry>,
    pub local_ai: Option<LocalAiRequest>,
    pub prepared_analysis: Option<PreparedAnalysisSnapshot>,
}

#[tauri::command]
pub async fn start_anonymize_job(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    settings: State<'_, Arc<SettingsStore>>,
    jobs: State<'_, AnonymizeJobStore>,
    request: StartAnonymizeJobRequest,
) -> Result<AnonymizeJobStatus, String> {
    validate_sample_count(
        request.sample_row_count,
        MAX_SAMPLE_ROW_COUNT,
        "Sample row count",
    )?;
    // Refuse a busy app before asking the user anything. `authorize_or_confirm_output_file`
    // can open a blocking native dialog, and running it first walked a user through
    // confirming a destination only to be told afterwards that another job holds the only
    // slot. This check does not reserve the slot: taking the lease here would leak it on
    // every early return below — a denied authorization, or the user cancelling that same
    // dialog — wedging admission for the life of the process. `create_job_for_output` below
    // is still the authority that decides, so a race past this point is refused there.
    jobs.admission_available()?;
    let file_path = path_access.authorize_input_file(request.file_path)?;
    let (local_ner_enabled, local_ner_model) = settings
        .load_settings()
        .map(|settings| (settings.local_ner_enabled, settings.local_ai_model))
        .map_err(|error| format!("Could not load settings: {error}"))?;
    require_prepared_analysis(local_ner_enabled, request.prepared_analysis.as_ref())?;
    require_snapshot_model(request.prepared_analysis.as_ref(), &local_ner_model)?;
    let validated_input = ValidatedFileInput::prepare(
        request.prepared_analysis.as_ref(),
        file_path,
        request.sample_row_count,
        &request.columns,
    )?;
    let output_path = authorize_or_confirm_output_file(&app, &path_access, request.output_path)?;
    if validated_input.original_path() == output_path {
        return Err("Output path must differ from the input path.".to_string());
    }
    let local_ai_enabled = settings
        .load_settings()
        .map(|settings| settings.local_ai_enabled)
        .map_err(|error| format!("Could not load settings: {error}"))?;
    let job = jobs.create_job_for_output(request.total_row_count)?;
    let initial_status = job.snapshot()?;
    let worker_job = job.clone();
    let panic_job = job.clone();
    let detection_run_summary = request
        .prepared_analysis
        .as_ref()
        .map(snapshot_detection_summary);

    let _job_handle = tauri::async_runtime::spawn_blocking(move || {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let processing_path = validated_input.processing_path();
            run_anonymize_job(
                worker_job,
                AnonymizeParams {
                    file_path: processing_path,
                    output_path,
                    columns: request.columns,
                    controls: request.controls,
                    force: request.force,
                    preview_smart_replacements: request.preview_smart_replacements,
                },
                request.sample_row_count,
                request.local_ai,
                local_ai_enabled,
                detection_run_summary,
            );
        }));
        if result.is_err() {
            panic_job.finish_panic();
        }
    });

    Ok(initial_status)
}

#[tauri::command]
pub fn get_anonymize_job_status(
    jobs: State<'_, AnonymizeJobStore>,
    job_id: String,
) -> Result<AnonymizeJobStatus, String> {
    jobs.snapshot_job(&job_id)
}

#[tauri::command]
pub fn cancel_anonymize_job(
    jobs: State<'_, AnonymizeJobStore>,
    job_id: String,
) -> Result<AnonymizeJobStatus, String> {
    let job = jobs.get_job(&job_id)?;
    let status = job.snapshot()?;
    if status.state.is_terminal() {
        return Ok(status);
    }
    job.request_cancel()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::AnonymizeJobState;
    use csv_anonymizer_core::{AnonymizationStrategy, AnonymizerError};
    use serde_json::{Value, json};

    fn start_payload() -> Value {
        json!({
            "filePath": "/tmp/data.csv",
            "outputPath": "/tmp/data_private.csv",
            "columns": [0, 1],
            "controls": [
                { "columnIndex": 0, "typeOverride": "fullName", "strategy": "localAi" },
            ],
            "force": false,
            "sampleRowCount": 200,
            "totalRowCount": 1000,
            "previewSmartReplacements": [
                { "columnIndex": 0, "original": "Alice Smith", "replacement": "Preview Alice" },
            ],
            "localAi": { "enabled": true, "model": "gemma3:4b" },
        })
    }

    fn parse(payload: Value) -> StartAnonymizeJobRequest {
        serde_json::from_value(payload).expect("start anonymize job request")
    }

    /// The start command accepts the payload `startAnonymizeJob` sends, field for field.
    ///
    /// The frontend suite mocks this boundary, so a rename on either side shows up only when
    /// a real run is attempted — after the user has already picked a destination.
    #[test]
    fn start_request_accepts_the_payload_the_frontend_sends() {
        let request = parse(start_payload());

        assert_eq!(request.file_path, PathBuf::from("/tmp/data.csv"));
        assert_eq!(request.output_path, PathBuf::from("/tmp/data_private.csv"));
        assert_eq!(request.columns, vec![0, 1]);
        assert_eq!(request.controls[0].strategy, AnonymizationStrategy::LocalAi);
        assert!(!request.force);
        assert_eq!(request.sample_row_count, 200);
        assert_eq!(request.total_row_count, Some(1000));
        assert_eq!(request.preview_smart_replacements.len(), 1);
        assert!(request.local_ai.expect("local ai request").enabled);
    }

    /// Overwriting an existing output file always takes an explicit `force`.
    ///
    /// A missing flag is refused rather than assumed, so no payload shape can arrive at the
    /// job with permission to overwrite a file the user never named.
    #[test]
    fn start_request_refuses_a_payload_with_no_force_flag() {
        let mut payload = start_payload();
        payload
            .as_object_mut()
            .expect("payload object")
            .remove("force");

        assert!(serde_json::from_value::<StartAnonymizeJobRequest>(payload).is_err());
    }

    /// A run with no strategy overrides and no preview values is still a valid request.
    ///
    /// Both lists default to empty, which is what a plain auto-strategy run looks like.
    #[test]
    fn start_request_treats_omitted_controls_and_preview_values_as_empty() {
        let mut payload = start_payload();
        let object = payload.as_object_mut().expect("payload object");
        object.remove("controls");
        object.remove("previewSmartReplacements");

        let request = parse(payload);

        assert!(request.controls.is_empty());
        assert!(request.preview_smart_replacements.is_empty());
    }

    /// An unknown total row count is carried as unknown, not as zero.
    ///
    /// Zero would make the progress bar read as complete from the first row.
    #[test]
    fn start_request_carries_an_unknown_total_row_count_as_absent() {
        let mut payload = start_payload();
        payload["totalRowCount"] = Value::Null;

        assert!(parse(payload).total_row_count.is_none());
    }

    /// The start command refuses an out-of-range sample row count before doing anything else.
    ///
    /// This is the first check in `start_anonymize_job`; it runs before admission, before path
    /// authorization and before any dialog, so the same refusal is what the command produces.
    #[test]
    fn oversized_sample_row_counts_are_refused_before_any_work_starts() {
        let request = parse(start_payload());

        assert!(
            validate_sample_count(
                request.sample_row_count,
                MAX_SAMPLE_ROW_COUNT,
                "Sample row count"
            )
            .is_ok()
        );
        assert!(
            validate_sample_count(
                MAX_SAMPLE_ROW_COUNT + 1,
                MAX_SAMPLE_ROW_COUNT,
                "Sample row count"
            )
            .is_err()
        );
        assert!(validate_sample_count(0, MAX_SAMPLE_ROW_COUNT, "Sample row count").is_err());
    }

    /// The job status response serializes under the names the frontend polls for.
    ///
    /// `ProcessingStatus` reads the state, the cancel flag and the row counters straight off
    /// this object; a rename leaves a running job looking stalled.
    #[test]
    fn job_status_serializes_the_names_the_frontend_polls_for() {
        let jobs = AnonymizeJobStore::default();
        let job = jobs.create_job_for_output(Some(1000)).expect("job");
        let status = job.request_cancel().expect("cancel");

        let value = serde_json::to_value(&status).expect("job status");

        assert_eq!(value["state"], json!("running"));
        assert_eq!(value["rowsProcessed"], json!(0));
        assert_eq!(value["totalRows"], json!(1000));
        assert_eq!(value["cancelRequested"], json!(true));
        assert!(value["jobId"].as_str().is_some_and(|id| !id.is_empty()));
    }

    /// Polling or cancelling an id this app never issued fails instead of reporting a run.
    ///
    /// Both commands do nothing but forward the id to the store, so this is the whole of
    /// their error handling: an unknown id must not resolve to another user's job or to a
    /// blank "running" status.
    #[test]
    fn status_and_cancel_refuse_a_job_id_the_store_never_issued() {
        let jobs = AnonymizeJobStore::default();
        let _live_job = jobs.create_job_for_output(None).expect("job");

        let status_error = jobs.snapshot_job("job-0-999").unwrap_err();
        let cancel_error = jobs.get_job("job-0-999").unwrap_err();

        assert!(status_error.contains("anonymization job"));
        assert!(status_error.contains("job-0-999"));
        assert_eq!(status_error, cancel_error);
    }

    /// A finished job reports the outcome it reached, and reports it more than once.
    ///
    /// The status the command returns is read straight from the store, so a run that failed
    /// has to keep saying so across retried polls; a run that quietly stopped being findable
    /// would leave the UI unable to tell a failure from a success.
    #[test]
    fn a_finished_job_keeps_reporting_the_outcome_it_reached() {
        let jobs = AnonymizeJobStore::default();
        let job = jobs.create_job_for_output(None).expect("job");
        job.finish(Err(AnonymizerError::SmartReplacement(
            "provider unavailable".to_string(),
        )));
        let job_id = job.snapshot().expect("status").job_id;

        let first = jobs.snapshot_job(&job_id).expect("first poll");
        let repeat = jobs.snapshot_job(&job_id).expect("repeat poll");

        assert_eq!(first.state, AnonymizeJobState::Failed);
        assert!(first.result.is_none());
        assert!(
            first
                .error
                .as_deref()
                .is_some_and(|error| error.contains("provider unavailable"))
        );
        assert_eq!(repeat.state, AnonymizeJobState::Failed);
    }
}
