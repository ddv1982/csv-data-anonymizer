use crate::settings::{AppSettings, SettingsStore, sanitize_settings};
use csv_anonymizer_core::{
    AnonymizeData, AnonymizeParams, AnonymizerService, ColumnMetadata, HeadersData, PiiRisk,
    PreviewData, PreviewParams,
};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::{DialogExt, FilePath};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    pub headers: HeadersData,
    pub selected_columns: Vec<usize>,
    pub suggested_output_path: PathBuf,
}

#[tauri::command]
pub fn load_settings() -> Result<AppSettings, String> {
    SettingsStore::default()
        .load()
        .map_err(|error| format!("Could not load settings: {error}"))
}

#[tauri::command]
pub fn save_settings(mut settings: AppSettings) -> Result<(), String> {
    sanitize_settings(&mut settings);
    SettingsStore::default()
        .save(&settings)
        .map_err(|error| format!("Could not save settings: {error}"))
}

#[tauri::command]
pub async fn pick_input_csv(
    app: tauri::AppHandle,
    initial_directory: Option<PathBuf>,
) -> Result<Option<PathBuf>, String> {
    pick_file_path(
        &app,
        "Select CSV file",
        "CSV files",
        &["csv", "tsv", "txt"],
        "input CSV",
        initial_directory.as_deref(),
    )
}

#[tauri::command]
pub async fn pick_output_csv(
    app: tauri::AppHandle,
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
        .transpose()
}

#[tauri::command]
pub async fn analyze_csv(
    file_path: PathBuf,
    sample_row_count: usize,
    output_suffix: String,
) -> Result<AnalyzeResponse, String> {
    run_blocking(move || {
        let service = service();
        let headers = service
            .analyze_csv_sampled(&file_path, sample_row_count)
            .map_err(|error| error.to_string())?;
        let selected_columns = headers
            .columns
            .iter()
            .filter(|column| should_auto_select(column))
            .map(|column| column.index)
            .collect::<Vec<_>>();
        let suggested_output_path =
            default_output_path_with_suffix(&headers.file_path, &output_suffix);

        Ok(AnalyzeResponse {
            headers,
            selected_columns,
            suggested_output_path,
        })
    })
    .await
}

#[tauri::command]
pub async fn preview_anonymization(
    file_path: PathBuf,
    columns: Vec<usize>,
    deterministic: bool,
    seed: String,
    sample_count: usize,
) -> Result<PreviewData, String> {
    run_blocking(move || {
        service()
            .preview_anonymization(PreviewParams {
                file_path,
                columns,
                deterministic,
                seed,
                sample_count,
            })
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn count_csv_rows(file_path: PathBuf) -> Result<usize, String> {
    run_blocking(move || {
        service()
            .count_csv_rows(&file_path)
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub async fn anonymize_csv(
    file_path: PathBuf,
    output_path: PathBuf,
    columns: Vec<usize>,
    deterministic: bool,
    seed: String,
    force: bool,
    sample_row_count: usize,
) -> Result<AnonymizeData, String> {
    run_blocking(move || {
        service()
            .anonymize_csv_with_sample_rows(
                AnonymizeParams {
                    file_path,
                    output_path,
                    columns,
                    deterministic,
                    seed,
                    force,
                },
                sample_row_count,
            )
            .map_err(|error| error.to_string())
    })
    .await
}

#[tauri::command]
pub fn open_output_location(output_path: PathBuf) -> Result<(), String> {
    let location = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(output_path);
    open::that_detached(&location)
        .map_err(|error| format!("Could not open {}: {error}", location.display()))
}

async fn run_blocking<T>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| format!("Background task failed: {error}"))?
}

fn service() -> AnonymizerService {
    AnonymizerService::new(env!("CARGO_PKG_VERSION"))
}

fn should_auto_select(column: &ColumnMetadata) -> bool {
    !column.sample_values.is_empty() && matches!(column.pii_risk, PiiRisk::High | PiiRisk::Medium)
}

fn default_output_path_with_suffix(input_path: &Path, suffix: &str) -> PathBuf {
    let suffix = if suffix.trim().is_empty() {
        "_anonymized"
    } else {
        suffix.trim()
    };
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let file_name = match input_path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => format!("{stem}{suffix}.{extension}"),
        _ => format!("{stem}{suffix}"),
    };
    input_path.with_file_name(file_name)
}

fn selected_dialog_path(path: FilePath, file_kind: &str) -> Result<PathBuf, String> {
    path.into_path()
        .map_err(|error| format!("Unsupported {file_kind} path: {error}"))
}

fn pick_file_path(
    app: &tauri::AppHandle,
    title: &str,
    filter_name: &str,
    extensions: &[&str],
    file_kind: &str,
    initial_directory: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, extensions);

    if let Some(directory) = initial_directory.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(directory);
    }

    dialog
        .blocking_pick_file()
        .map(|path| selected_dialog_path(path, file_kind))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_output_path_with_custom_suffix() {
        assert_eq!(
            default_output_path_with_suffix(Path::new("/tmp/data.csv"), "_private"),
            PathBuf::from("/tmp/data_private.csv")
        );
    }
}
