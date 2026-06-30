use crate::error::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn replace_file_atomically<T>(
    output_path: &Path,
    write_temporary: impl FnOnce(&Path) -> Result<T>,
) -> Result<T> {
    let temporary_output_path = temporary_output_path(output_path);
    match write_temporary(&temporary_output_path) {
        Ok(result) => {
            fs::rename(&temporary_output_path, output_path)?;
            Ok(result)
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary_output_path);
            Err(error)
        }
    }
}

fn temporary_output_path(output_path: &Path) -> PathBuf {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output.csv");
    let suffix = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    );
    parent.join(format!(".{file_name}.{suffix}.tmp"))
}
