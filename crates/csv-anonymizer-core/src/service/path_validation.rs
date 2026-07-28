use crate::error::{AnonymizerError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_WRITE_PROBE: AtomicU64 = AtomicU64::new(0);

pub fn generate_default_output_path(input_path: &Path) -> PathBuf {
    let extension = input_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("csv");
    let stem = input_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let file_name = format!("{stem}_private_output.{extension}");
    input_path.with_file_name(file_name)
}

pub(super) fn normalize_path(path: &Path) -> Result<PathBuf> {
    if path.as_os_str().is_empty() {
        return Err(AnonymizerError::FileNotFound(path.to_path_buf()));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

// The transform streams input to a temp file and atomically renames it over the
// output; if output == input the original data would be destroyed. Compare on
// canonicalized paths so symlinks and relative spellings cannot bypass the guard.
pub(super) fn ensure_output_differs_from_input(
    input_path: &Path,
    output_path: &Path,
) -> Result<()> {
    let canonical_input = fs::canonicalize(input_path).unwrap_or_else(|_| input_path.to_path_buf());
    let output_path = normalize_path(output_path)?;
    let canonical_output = match (output_path.parent(), output_path.file_name()) {
        (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => fs::canonicalize(parent)
            .map(|parent| parent.join(name))
            .unwrap_or_else(|_| output_path.to_path_buf()),
        _ => output_path.to_path_buf(),
    };
    if canonical_input == canonical_output {
        return Err(AnonymizerError::OutputSameAsInput(canonical_output));
    }
    Ok(())
}

pub(super) fn validate_output_path(output_path: &Path, force: bool) -> Result<PathBuf> {
    let normalized = normalize_path(output_path)?;
    if normalized.exists() && !force {
        return Err(AnonymizerError::OutputExists(normalized));
    }

    let output_dir = normalized
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if !output_dir.is_dir() {
        return Err(AnonymizerError::OutputDirectoryNotWritable(output_dir));
    }

    loop {
        let probe_sequence = NEXT_WRITE_PROBE.fetch_add(1, Ordering::Relaxed);
        let probe = write_probe_path(&output_dir, probe_sequence);
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe)
        {
            Ok(_) => {
                let _ = fs::remove_file(probe);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(AnonymizerError::OutputDirectoryNotWritable(output_dir)),
        }
    }

    Ok(normalized)
}

fn write_probe_path(output_dir: &Path, sequence: u64) -> PathBuf {
    output_dir.join(format!(
        ".csv-anonymizer-write-test-{}-{sequence}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_probe_collisions_are_retried_and_cleaned_up() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.csv");
        let first_sequence = NEXT_WRITE_PROBE.load(Ordering::Relaxed);
        let stale_paths = (first_sequence..first_sequence + 1024)
            .map(|sequence| write_probe_path(temp_dir.path(), sequence))
            .collect::<Vec<_>>();
        for path in &stale_paths {
            fs::write(path, "stale").unwrap();
        }

        assert_eq!(
            validate_output_path(&output_path, false).unwrap(),
            output_path
        );

        for path in stale_paths {
            fs::remove_file(path).unwrap();
        }
        assert!(fs::read_dir(temp_dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn concurrent_output_validations_do_not_collide_or_leave_probes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_dir = std::sync::Arc::new(temp_dir.path().to_path_buf());
        let threads = (0..16)
            .map(|index| {
                let output_dir = output_dir.clone();
                std::thread::spawn(move || {
                    let output_path = output_dir.join(format!("output-{index}.csv"));
                    validate_output_path(&output_path, false)
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().unwrap().unwrap();
        }
        assert!(fs::read_dir(temp_dir.path()).unwrap().next().is_none());
    }
}
