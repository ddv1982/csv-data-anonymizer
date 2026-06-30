macro_rules! tauri_command_list {
    ($macro:ident) => {
        $macro!(
            load_settings,
            save_settings,
            pick_input_csv,
            pick_output_csv,
            analyze_csv,
            analyze_pasted_data,
            count_csv_rows,
            preflight_anonymization,
            preview_anonymization,
            preview_pasted_data,
            anonymize_pasted_data,
            generate_quick_values,
            start_anonymize_job,
            get_anonymize_job_status,
            cancel_anonymize_job,
            get_local_ai_status,
            start_local_ai_model_download,
            get_local_ai_model_download_status,
            cancel_local_ai_model_download,
            open_local_ai_setup_url,
            open_output_location,
        )
    };
}
