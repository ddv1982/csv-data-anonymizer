use crate::error::{AnonymizerError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY_OUTPUT_FILE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn replace_file_atomically<T>(
    output_path: &Path,
    overwrite: bool,
    write_temporary: impl FnOnce(&Path) -> Result<T>,
) -> Result<T> {
    let temporary_output_path = reserve_temporary_output_path(output_path)?;
    match write_temporary(&temporary_output_path) {
        Ok(result) => {
            let publish_result = if overwrite {
                fs::rename(&temporary_output_path, output_path).map_err(AnonymizerError::from)
            } else {
                fs::hard_link(&temporary_output_path, output_path).map_err(|error| {
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        AnonymizerError::OutputExists(output_path.to_path_buf())
                    } else {
                        AnonymizerError::from(error)
                    }
                })
            };
            match publish_result {
                Ok(()) => {
                    if !overwrite {
                        let _ = fs::remove_file(&temporary_output_path);
                    }
                    Ok(result)
                }
                Err(error) => {
                    let _ = fs::remove_file(&temporary_output_path);
                    Err(error)
                }
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary_output_path);
            Err(error)
        }
    }
}

fn reserve_temporary_output_path(output_path: &Path) -> Result<PathBuf> {
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = output_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output.csv");
    loop {
        let sequence = NEXT_TEMPORARY_OUTPUT_FILE.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".{file_name}.{}-{sequence}.tmp",
            std::process::id()
        ));
        match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(_) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_clobber_publish_preserves_a_destination_created_during_the_write() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.csv");

        let error = replace_file_atomically(&output_path, false, |temporary_path| {
            fs::write(temporary_path, "new").unwrap();
            fs::write(&output_path, "existing").unwrap();
            Ok(())
        })
        .unwrap_err();

        assert!(matches!(error, AnonymizerError::OutputExists(_)));
        assert_eq!(fs::read_to_string(output_path).unwrap(), "existing");
        assert!(fs::read_dir(temp_dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn failed_publish_removes_the_temporary_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.csv");
        fs::create_dir(&output_path).unwrap();

        replace_file_atomically(&output_path, true, |temporary_path| {
            fs::write(temporary_path, "new").unwrap();
            Ok(())
        })
        .unwrap_err();

        assert!(fs::read_dir(temp_dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn publication_matrix_preserves_overwrite_contracts() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("output.csv");
        let absent_output_path = temp_dir.path().join("absent-output.csv");

        replace_file_atomically(&absent_output_path, true, |temporary_path| {
            fs::write(temporary_path, "created").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read_to_string(absent_output_path).unwrap(), "created");

        replace_file_atomically(&output_path, false, |temporary_path| {
            fs::write(temporary_path, "first").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "first");

        let error = replace_file_atomically(&output_path, false, |temporary_path| {
            fs::write(temporary_path, "blocked").unwrap();
            Ok(())
        })
        .unwrap_err();
        assert!(matches!(error, AnonymizerError::OutputExists(_)));
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "first");

        replace_file_atomically(&output_path, true, |temporary_path| {
            fs::write(temporary_path, "replacement").unwrap();
            Ok(())
        })
        .unwrap();
        assert_eq!(fs::read_to_string(output_path).unwrap(), "replacement");
        assert!(fs::read_dir(temp_dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn concurrent_temporary_paths_are_reserved_uniquely() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = std::sync::Arc::new(temp_dir.path().join("output.csv"));
        let threads = (0..16)
            .map(|_| {
                let output_path = output_path.clone();
                std::thread::spawn(move || reserve_temporary_output_path(&output_path).unwrap())
            })
            .collect::<Vec<_>>();
        let paths = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<std::collections::HashSet<_>>();

        assert_eq!(paths.len(), 16);
        for path in paths {
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn concurrent_no_clobber_publishers_produce_one_complete_winner() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = std::sync::Arc::new(temp_dir.path().join("output.csv"));
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads = (0..8)
            .map(|index| {
                let output_path = output_path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    replace_file_atomically(&output_path, false, |temporary_path| {
                        fs::write(temporary_path, format!("payload-{index}")).unwrap();
                        barrier.wait();
                        Ok(())
                    })
                })
            })
            .collect::<Vec<_>>();
        let results = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(result, Err(AnonymizerError::OutputExists(_))))
                .count(),
            7
        );
        let output = fs::read_to_string(&*output_path).unwrap();
        assert!((0..8).any(|index| output == format!("payload-{index}")));
        assert!(fs::read_dir(temp_dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn concurrent_overwrite_publishers_leave_one_complete_output() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = std::sync::Arc::new(temp_dir.path().join("output.csv"));
        fs::write(&*output_path, "existing").unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let threads = (0..8)
            .map(|index| {
                let output_path = output_path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    replace_file_atomically(&output_path, true, |temporary_path| {
                        fs::write(temporary_path, format!("replacement-{index}")).unwrap();
                        barrier.wait();
                        Ok(())
                    })
                })
            })
            .collect::<Vec<_>>();

        for thread in threads {
            thread.join().unwrap().unwrap();
        }
        let output = fs::read_to_string(&*output_path).unwrap();
        assert!((0..8).any(|index| output == format!("replacement-{index}")));
        assert!(fs::read_dir(temp_dir.path()).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }
}
