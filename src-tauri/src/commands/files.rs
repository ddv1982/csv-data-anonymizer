use super::shared::{pick_file_path, selected_dialog_path};
use crate::command_error::CommandError;
use crate::path_access::PathAccess;
use std::path::{Path, PathBuf};
use tauri::State;
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn pick_input_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    initial_directory: Option<PathBuf>,
) -> Result<Option<PathBuf>, CommandError> {
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
        .map_err(Into::into)
}

#[tauri::command]
pub async fn pick_output_csv(
    app: tauri::AppHandle,
    path_access: State<'_, PathAccess>,
    suggested_output_path: Option<PathBuf>,
) -> Result<Option<PathBuf>, CommandError> {
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
        .map_err(Into::into)
}

#[tauri::command]
pub fn open_output_location(
    path_access: State<'_, PathAccess>,
    output_path: PathBuf,
) -> Result<(), CommandError> {
    let output_path = path_access.authorize_output_file(output_path)?;
    let location = output_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or(output_path);
    open::that_detached(&location).map_err(|error| {
        CommandError::from(format!("Could not open {}: {error}", location.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// The three commands in this file are the app's only source of file access: each picker
    /// grants what the user chose, and `open_output_location` refuses anything else. The
    /// commands themselves need a Tauri `AppHandle` and `State` and cannot be called here, so
    /// these tests pin the grant/refuse contract they are built out of.
    fn temp_csv(directory: &Path, name: &str) -> PathBuf {
        let path = directory.join(name);
        fs::write(&path, "id,email\n1,a@example.com\n").expect("write fixture");
        path
    }

    /// A file the user never picked is refused, and the refusal points at the picker.
    ///
    /// Without this a caller could name any path on disk in a command payload and have the
    /// app read it, which is exactly the access the picker exists to bound.
    #[test]
    fn a_file_the_picker_never_granted_is_refused() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let input = temp_csv(temp_dir.path(), "data.csv");
        let path_access = PathAccess::default();

        let error = path_access.authorize_input_file(&input).unwrap_err();

        assert!(error.contains("has not been granted"));
        assert!(error.contains("Browse"));
    }

    /// Picking an input file grants exactly that file and nothing beside it.
    ///
    /// `pick_input_csv` grants the chosen path; a grant that spread to the directory would
    /// hand the app every other file the user keeps next to their data.
    #[test]
    fn picking_an_input_file_grants_that_file_alone() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let picked = temp_csv(temp_dir.path(), "picked.csv");
        let sibling = temp_csv(temp_dir.path(), "sibling.csv");
        let path_access = PathAccess::default();

        let granted = path_access.grant_input_file(&picked).expect("grant");

        assert_eq!(
            path_access
                .authorize_input_file(&picked)
                .expect("authorize"),
            granted
        );
        assert!(path_access.authorize_input_file(&sibling).is_err());
    }

    /// Read access to a file is never also write access to it.
    ///
    /// `open_output_location` and the anonymize job both authorize against the output grants;
    /// if an input grant satisfied them, choosing a file to read would silently make it a
    /// legal destination to overwrite.
    #[test]
    fn granting_a_file_for_reading_does_not_make_it_writable() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let input = temp_csv(temp_dir.path(), "data.csv");
        let path_access = PathAccess::default();

        path_access.grant_input_file(&input).expect("input grant");

        assert!(path_access.authorize_output_file(&input).is_err());
    }

    /// Write access to a destination is never also read access to it.
    #[test]
    fn granting_a_destination_for_writing_does_not_make_it_readable() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let output = temp_csv(temp_dir.path(), "out.csv");
        let path_access = PathAccess::default();

        path_access
            .grant_output_file(&output)
            .expect("output grant");

        assert!(path_access.authorize_input_file(&output).is_err());
    }

    /// Choosing one destination does not authorize its neighbours.
    ///
    /// `open_output_location` reveals the containing directory, but authorization stays keyed
    /// to the single file the save dialog returned.
    #[test]
    fn choosing_one_destination_does_not_authorize_its_neighbours() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let chosen = temp_dir.path().join("chosen.csv");
        let neighbour = temp_dir.path().join("neighbour.csv");
        let path_access = PathAccess::default();

        path_access.grant_output_file(&chosen).expect("grant");

        assert!(path_access.authorize_output_file(&chosen).is_ok());
        assert!(path_access.authorize_output_file(&neighbour).is_err());
    }

    /// A granted destination stays granted when spelled with `.` and `..` segments.
    ///
    /// The save dialog and the later job can hand back the same file written differently;
    /// normalization is what stops that from reading as an ungranted path.
    #[test]
    fn a_granted_destination_is_recognised_through_relative_segments() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let nested = temp_dir.path().join("nested");
        fs::create_dir(&nested).expect("nested dir");
        let output = nested.join("out.csv");
        let path_access = PathAccess::default();

        let granted = path_access.grant_output_file(&output).expect("grant");
        let detoured = nested.join("..").join("nested").join("out.csv");

        assert_eq!(
            path_access
                .authorize_output_file(&detoured)
                .expect("authorize"),
            granted
        );
    }

    /// A directory is never accepted as a destination file.
    ///
    /// `pick_output_csv` treats a directory-shaped suggestion as a starting folder, and the
    /// grant behind it must refuse to treat one as the file to write.
    #[test]
    fn a_directory_is_never_granted_as_a_destination_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let directory = temp_dir.path().join("out.csv");
        fs::create_dir(&directory).expect("directory");
        let path_access = PathAccess::default();

        let error = path_access.grant_output_file(&directory).unwrap_err();

        assert!(error.contains("not a regular output file"));
    }
}
