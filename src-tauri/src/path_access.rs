use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct PathAccess {
    inner: Mutex<AllowedPaths>,
}

#[derive(Debug, Default)]
struct AllowedPaths {
    input_files: HashSet<PathBuf>,
    output_files: HashSet<PathBuf>,
}

impl PathAccess {
    pub fn grant_input_file(&self, path: impl AsRef<Path>) -> Result<PathBuf, String> {
        let path = canonical_input_file(path.as_ref())?;
        let mut inner = self.lock()?;
        inner.input_files.insert(path.clone());
        Ok(path)
    }

    pub fn grant_output_file(&self, path: impl AsRef<Path>) -> Result<PathBuf, String> {
        let path = normalize_output_file(path.as_ref())?;
        let mut inner = self.lock()?;
        inner.output_files.insert(path.clone());
        Ok(path)
    }

    pub fn authorize_input_file(&self, path: impl AsRef<Path>) -> Result<PathBuf, String> {
        let path = canonical_input_file(path.as_ref())?;
        let inner = self.lock()?;
        if inner.input_files.contains(&path) {
            Ok(path)
        } else {
            Err(format!(
                "File access has not been granted for {}. Use Browse to select the CSV file.",
                path.display()
            ))
        }
    }

    pub fn authorize_output_file(&self, path: impl AsRef<Path>) -> Result<PathBuf, String> {
        let path = normalize_output_file(path.as_ref())?;
        let inner = self.lock()?;
        if inner.output_files.contains(&path) {
            Ok(path)
        } else {
            Err(format!(
                "Output access has not been granted for {}. Use Browse to choose the output file.",
                path.display()
            ))
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, AllowedPaths>, String> {
        self.inner
            .lock()
            .map_err(|_| "Path access state is unavailable.".to_string())
    }
}

fn canonical_input_file(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("CSV path is empty.".to_string());
    }

    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("Could not access {}: {error}", path.display()))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("Could not inspect {}: {error}", canonical.display()))?;
    if !metadata.is_file() {
        return Err(format!("{} is not a file.", canonical.display()));
    }

    Ok(canonical)
}

fn normalize_output_file(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("Output path is empty.".to_string());
    }

    let file_name = path
        .file_name()
        .ok_or_else(|| format!("{} is not a valid output file path.", path.display()))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        format!(
            "Could not access output directory {}: {error}",
            parent.display()
        )
    })?;
    let metadata = fs::metadata(&canonical_parent).map_err(|error| {
        format!(
            "Could not inspect output directory {}: {error}",
            canonical_parent.display()
        )
    })?;
    if !metadata.is_dir() {
        return Err(format!(
            "{} is not a directory.",
            canonical_parent.display()
        ));
    }

    Ok(canonical_parent.join(file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grants_and_authorizes_input_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input = temp_dir.path().join("data.csv");
        fs::write(&input, "id,email\n1,a@example.com\n").unwrap();
        let access = PathAccess::default();

        let granted = access.grant_input_file(&input).unwrap();
        let authorized = access.authorize_input_file(&input).unwrap();

        assert_eq!(authorized, granted);
    }

    #[test]
    fn rejects_ungranted_input_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let input = temp_dir.path().join("data.csv");
        fs::write(&input, "id,email\n1,a@example.com\n").unwrap();
        let access = PathAccess::default();

        assert!(access.authorize_input_file(&input).is_err());
    }

    #[test]
    fn grants_output_files_by_canonical_parent() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output = temp_dir.path().join("out.csv");
        let access = PathAccess::default();

        let granted = access.grant_output_file(&output).unwrap();
        let authorized = access.authorize_output_file(&output).unwrap();

        assert_eq!(authorized, granted);
    }
}
