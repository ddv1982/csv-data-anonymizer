use crate::path_access::PathAccess;
use csv_anonymizer_core::AnonymizerService;
use std::path::{Path, PathBuf};
use tauri_plugin_dialog::{DialogExt, FilePath, MessageDialogButtons, MessageDialogKind};

pub(super) fn authorize_or_confirm_input_file(
    app: &tauri::AppHandle,
    path_access: &PathAccess,
    file_path: PathBuf,
) -> Result<PathBuf, String> {
    match path_access.authorize_input_file(&file_path) {
        Ok(path) => Ok(path),
        Err(_) => {
            if confirm_path_access(
                app,
                "Allow CSV file access?",
                &format!(
                    "Allow CSV Anonymizer to read this file?\n\n{}",
                    file_path.display()
                ),
                "Allow",
            ) {
                path_access.grant_input_file(file_path)
            } else {
                Err("CSV file access was not granted.".to_string())
            }
        }
    }
}

pub(super) fn authorize_or_confirm_output_file(
    app: &tauri::AppHandle,
    path_access: &PathAccess,
    output_path: PathBuf,
) -> Result<PathBuf, String> {
    match path_access.authorize_output_file(&output_path) {
        Ok(path) => Ok(path),
        Err(_) => {
            if confirm_path_access(
                app,
                "Allow output file access?",
                &format!(
                    "Allow CSV Anonymizer to write this output file?\n\n{}",
                    output_path.display()
                ),
                "Allow",
            ) {
                path_access.grant_output_file(output_path)
            } else {
                Err("Output file access was not granted.".to_string())
            }
        }
    }
}

fn confirm_path_access(app: &tauri::AppHandle, title: &str, message: &str, ok_label: &str) -> bool {
    app.dialog()
        .message(message)
        .title(title)
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            ok_label.to_string(),
            "Cancel".to_string(),
        ))
        .blocking_show()
}

pub(super) async fn run_blocking<T>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| format!("Background task failed: {error}"))?
}

pub(super) fn service() -> AnonymizerService {
    AnonymizerService::new(env!("CARGO_PKG_VERSION"))
}

pub(super) fn default_output_path_with_suffix(
    input_path: &Path,
    suffix: &str,
) -> Result<PathBuf, String> {
    if suffix.chars().any(char::is_control) {
        return Err("Output suffix must be plain filename text without path separators or control characters.".to_string());
    }
    let suffix = if suffix.trim().is_empty() {
        "_private_output"
    } else {
        suffix.trim()
    };
    validate_output_suffix(suffix)?;
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let file_name = match input_path.extension().and_then(|value| value.to_str()) {
        Some(extension) if !extension.is_empty() => format!("{stem}{suffix}.{extension}"),
        _ => format!("{stem}{suffix}"),
    };
    Ok(input_path.with_file_name(file_name))
}

fn validate_output_suffix(suffix: &str) -> Result<(), String> {
    if suffix.contains('/') || suffix.contains('\\') || suffix.chars().any(char::is_control) {
        Err("Output suffix must be plain filename text without path separators or control characters.".to_string())
    } else {
        Ok(())
    }
}

pub(super) fn selected_dialog_path(path: FilePath, file_kind: &str) -> Result<PathBuf, String> {
    path.into_path()
        .map_err(|error| format!("Unsupported {file_kind} path: {error}"))
}

pub(super) fn pick_file_path(
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
            default_output_path_with_suffix(Path::new("/tmp/data.csv"), "_private").unwrap(),
            PathBuf::from("/tmp/data_private.csv")
        );
    }

    #[test]
    fn rejects_path_like_output_suffixes() {
        assert!(default_output_path_with_suffix(Path::new("/tmp/data.csv"), "../private").is_err());
        assert!(
            default_output_path_with_suffix(Path::new("/tmp/data.csv"), "..\\private").is_err()
        );
        assert!(default_output_path_with_suffix(Path::new("/tmp/data.csv"), "_private\n").is_err());
    }
}
