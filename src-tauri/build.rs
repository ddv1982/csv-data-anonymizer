fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "load_settings",
            "save_settings",
            "reset_dp_budget_ledger",
            "pick_input_csv",
            "pick_output_csv",
            "analyze_csv",
            "count_csv_rows",
            "preview_anonymization",
            "anonymize_csv",
            "start_anonymize_job",
            "get_anonymize_job_status",
            "cancel_anonymize_job",
            "get_local_ai_status",
            "start_local_ai_model_download",
            "get_local_ai_model_download_status",
            "cancel_local_ai_model_download",
            "open_local_ai_setup_url",
            "open_output_location",
        ]),
    ))
    .expect("failed to build Tauri app permissions")
}
