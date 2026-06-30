mod commands;
mod job_registry;
mod jobs;
mod local_ai;
mod path_access;
mod settings;
include!("tauri_command_list.rs");

use commands::*;
use std::sync::Arc;

macro_rules! generate_tauri_handler {
    ($($command:ident),+ $(,)?) => {
        tauri::generate_handler![$($command),+]
    };
}

fn main() {
    tauri::Builder::default()
        .manage(jobs::AnonymizeJobStore::default())
        .manage(local_ai::LocalAiDownloadStore::default())
        .manage(path_access::PathAccess::default())
        .manage(Arc::new(settings::SettingsStore::default()))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri_command_list!(generate_tauri_handler))
        .run(tauri::generate_context!())
        .expect("error while running CSV Anonymizer");
}

#[cfg(test)]
mod tests {
    macro_rules! command_names {
        ($($command:ident),+ $(,)?) => {
            &[$(stringify!($command)),+]
        };
    }

    #[test]
    fn tauri_command_list_has_no_duplicate_names() {
        let names = tauri_command_list!(command_names);
        let unique_names = names
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(unique_names.len(), names.len());
    }
}
