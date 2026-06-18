mod commands;
mod settings;

use commands::{
    analyze_csv, anonymize_csv, load_settings, open_output_location, pick_input_csv,
    pick_output_csv, preview_anonymization, save_settings,
};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            load_settings,
            save_settings,
            pick_input_csv,
            pick_output_csv,
            analyze_csv,
            preview_anonymization,
            anonymize_csv,
            open_output_location,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CSV Anonymizer");
}
