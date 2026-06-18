use super::shared::{pick_file_path, selected_dialog_path};
use crate::path_access::PathAccess;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn pick_input_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    initial_directory: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let picked = pick_file_path(
        &app,
        "Select CSV file",
        "CSV files",
        &["csv", "tsv", "txt"],
        "input CSV",
        initial_directory.as_deref(),
    )?;

    picked
        .map(|path| path_access.grant_input_file(path))
        .transpose()
}

#[tauri::command]
pub async fn pick_output_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    suggested_output_path: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    let suggested_output_file = suggested_output_path.as_ref().filter(|path| !path.is_dir());
    let default_name = suggested_output_file
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .unwrap_or("anonymized.csv");

    let mut dialog = app
        .dialog()
        .file()
        .set_file_name(default_name)
        .add_filter("CSV files", &["csv"]);

    if let Some(directory) = suggested_output_path.as_ref().and_then(|path| {
        if path.is_dir() {
            Some(path.as_path())
        } else {
            path.parent()
        }
    }) {
        dialog = dialog.set_directory(directory);
    }

    dialog
        .blocking_save_file()
        .map(|path| selected_dialog_path(path, "output CSV"))
        .transpose()?
        .map(|path| path_access.grant_output_file(path))
        .transpose()
}

#[tauri::command]
pub fn open_output_location(
    path_access: State<'_, PathAccess>,
    output_path: PathBuf,
) -> Result<(), String> {
    let output_path = path_access.authorize_output_file(output_path)?;
    let location = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(output_path);
    open::that_detached(&location)
        .map_err(|error| format!("Could not open {}: {error}", location.display()))
}
