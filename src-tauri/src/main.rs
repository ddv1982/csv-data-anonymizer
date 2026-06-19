mod commands;
mod jobs;
mod local_ai;
mod path_access;
mod settings;

use commands::{
    analyze_csv, anonymize_csv, cancel_anonymize_job, cancel_local_ai_model_download,
    count_csv_rows, get_anonymize_job_status, get_local_ai_model_download_status,
    get_local_ai_status, load_settings, open_local_ai_setup_url, open_output_location,
    pick_input_csv, pick_output_csv, preview_anonymization, save_settings, start_anonymize_job,
    start_local_ai_model_download,
};

fn main() {
    tauri::Builder::default()
        .manage(jobs::AnonymizeJobStore::default())
        .manage(local_ai::LocalAiDownloadStore::default())
        .manage(path_access::PathAccess::default())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            pick_input_csv,
            pick_output_csv,
            analyze_csv,
            count_csv_rows,
            preview_anonymization,
            anonymize_csv,
            start_anonymize_job,
            get_anonymize_job_status,
            cancel_anonymize_job,
            get_local_ai_status,
            start_local_ai_model_download,
            get_local_ai_model_download_status,
            cancel_local_ai_model_download,
            open_local_ai_setup_url,
            open_output_location,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CSV Anonymizer");
}
