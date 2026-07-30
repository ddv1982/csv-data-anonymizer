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
            // Only the overwrite branch adopts a mode. It is the one that replaces a file
            // the user already placed; on the other branch a destination existing is the
            // error, so there is no mode there to inherit and reading one would describe a
            // file publication is about to refuse to touch.
            let publish_result = if overwrite {
                adopt_destination_permissions(&temporary_output_path, output_path).and_then(|()| {
                    fs::rename(&temporary_output_path, output_path).map_err(AnonymizerError::from)
                })
            } else {
                fs::hard_link(&temporary_output_path, output_path)
                    .map_err(|error| no_clobber_publish_error(error, output_path))
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

/// Explains a no-clobber publish that failed for a reason other than the
/// destination existing.
///
/// `hard_link` is what makes no-clobber atomic — it is the one publish that cannot
/// overwrite — but not every filesystem has hard links, and on those (FAT32 and
/// exFAT removable media, some network mounts) it fails for a reason that has
/// nothing to do with the user's file. Reported as its own message because the
/// generic I/O error names a call the user never made and offers no way forward,
/// while the way forward is exactly one setting.
///
/// `Unsupported` alone does not catch that case. Linux's `vfs_link` returns EPERM
/// when a filesystem defines no `.link` inode operation — neither vfat nor exfat
/// does — so the removable media this message exists for arrives here as
/// `PermissionDenied`, and Windows returns `ERROR_ACCESS_DENIED` for the same
/// situation. Only macOS reports `Unsupported`, so keying on it alone means the
/// guidance never reaches the users most likely to need it.
///
/// `PermissionDenied` is ambiguous in general — a read-only directory returns it
/// too, and telling those users their filesystem is at fault is worse than saying
/// nothing. What disambiguates it here is the caller: this only runs after
/// `reserve_temporary_output_path` created a file in `output_path`'s own parent
/// directory, which is the directory the link would be created in. So the
/// directory is writable, its search bits are ours, and the link source is a
/// regular file this process owns — the reasons EPERM/EACCES would otherwise
/// carry are all already refuted, and "this filesystem has no hard links" is what
/// is left. Only sound at this one call site; do not reuse for a link elsewhere.
///
/// Not disambiguated: a mandatory access control policy (SELinux, AppArmor) that
/// denies `link` specifically while permitting `open`. That user is told to enable
/// overwrite, which is still an action that resolves their failure.
fn no_clobber_publish_error(error: std::io::Error, output_path: &Path) -> AnonymizerError {
    match error.kind() {
        std::io::ErrorKind::AlreadyExists => {
            AnonymizerError::OutputExists(output_path.to_path_buf())
        }
        std::io::ErrorKind::Unsupported | std::io::ErrorKind::PermissionDenied => {
            AnonymizerError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!(
                    "{} is on a filesystem that cannot publish output without overwriting. \
                     Enable overwrite for this destination, or write to a folder on a \
                     filesystem with hard links, such as your home folder.",
                    output_path.display()
                ),
            ))
        }
        _ => AnonymizerError::from(error),
    }
}

/// Gives the temporary file the permissions the destination should end up with.
///
/// The temporary file is created owner-only, so a run in progress — and anything a
/// failed run leaves behind — is never readable by other users of the machine. That
/// mode would otherwise survive publication, because both publish paths carry the
/// temporary file's own permissions to the destination: `rename` moves the inode,
/// and `hard_link` makes the destination a second name for it.
///
/// So an overwriting publish restores the mode the destination is entitled to: it
/// keeps exactly the permissions it already had, because silently tightening a file
/// the user had already placed and shared is a surprise this has no business
/// springing.
///
/// Called only from that branch. The no-clobber branch publishes a destination that
/// did not exist a moment ago — one that does is its error, not its input — so there
/// is no mode there to inherit, and it keeps the owner-only one, which is the right
/// default for output derived from data the user brought here to protect. Calling
/// this there returned `Ok(())` from the first line anyway; the point of not calling
/// it is that the reader no longer has to work that out.
///
/// Only a regular file the invoking user owns is treated as "already placed and
/// shared". `symlink_metadata` deliberately does not follow links, because
/// `rename` and `hard_link` replace the *link*, not its target: reading the mode
/// through the link would adopt a mode that belongs to a file publication never
/// touches. In a directory other users can write — a team share, `/tmp`, a synced
/// folder — that is a way for someone else to choose the anonymized output's
/// permissions by leaving a symlink at the destination first. The ownership check
/// closes the same hole for a plain file they created there. Neither is a file the
/// user placed, so neither earns an exception to the owner-only default.
///
/// Only the permission bits are copied. `mode()` returns the whole `st_mode`,
/// including the file-type bits and setuid/setgid/sticky; passing that through
/// `from_mode` would rely on masking POSIX leaves unspecified, and a setgid bit is
/// a grant nobody chose for a CSV of anonymized data.
///
/// Unix only. Windows has no mode to carry and no umask to reason about, so there
/// is nothing here for it to do.
#[cfg(unix)]
fn adopt_destination_permissions(temporary_path: &Path, output_path: &Path) -> Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let Ok(existing) = fs::symlink_metadata(output_path) else {
        return Ok(());
    };
    if !existing.is_file() {
        return Ok(());
    }
    // The temporary file stands in for the effective uid: this process created it a
    // moment ago, so its owner is who we are running as. std exposes no `geteuid`,
    // and a file we already hold answers the same question without a dependency.
    let Ok(temporary) = fs::metadata(temporary_path) else {
        return Ok(());
    };
    if existing.uid() != temporary.uid() {
        return Ok(());
    }
    fs::set_permissions(
        temporary_path,
        fs::Permissions::from_mode(existing.permissions().mode() & 0o777),
    )?;
    Ok(())
}

#[cfg(not(unix))]
fn adopt_destination_permissions(_temporary_path: &Path, _output_path: &Path) -> Result<()> {
    Ok(())
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
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        // Owner-only from the moment it exists, not set afterwards: between creation
        // and a later chmod the file would be readable at the directory's default
        // mode, and it is holding transformed source data for the whole of that
        // window. `create_new` above means this mode applies to a file this call just
        // made, never to one it found.
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
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

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;

        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn temporary_output_is_owner_only_while_the_run_is_in_flight() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("in-flight.csv");
        let mut observed = None;

        replace_file_atomically(&output_path, true, |temporary_path| {
            fs::write(temporary_path, "sensitive").unwrap();
            observed = Some(mode_of(temporary_path));
            Ok(())
        })
        .unwrap();

        assert_eq!(observed, Some(0o600));
    }

    #[cfg(unix)]
    #[test]
    fn a_new_destination_is_published_owner_only() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("new-output.csv");

        replace_file_atomically(&output_path, true, |temporary_path| {
            fs::write(temporary_path, "created").unwrap();
            Ok(())
        })
        .unwrap();

        assert_eq!(mode_of(&output_path), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn an_existing_destination_keeps_the_permissions_it_had() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("existing-output.csv");
        fs::write(&output_path, "existing").unwrap();
        fs::set_permissions(&output_path, fs::Permissions::from_mode(0o644)).unwrap();

        replace_file_atomically(&output_path, true, |temporary_path| {
            fs::write(temporary_path, "replacement").unwrap();
            Ok(())
        })
        .unwrap();

        // Overwriting a file the user already placed must not silently change who
        // can read it.
        assert_eq!(mode_of(&output_path), 0o644);
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "replacement");
    }

    /// The no-clobber path adopts nothing, and now cannot: it does not call
    /// `adopt_destination_permissions` at all.
    ///
    /// The outcome is unchanged — that function returned early on a destination that does
    /// not exist, and on this path one that does is the error rather than the input — so
    /// this pins the mode rather than the call. It matters because the umask is the thing
    /// that would take over if the owner-only creation mode were ever lost here, and a
    /// default umask publishes anonymized output group- and world-readable.
    #[cfg(unix)]
    #[test]
    fn a_no_clobber_publish_is_owner_only() {
        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("no-clobber-output.csv");

        replace_file_atomically(&output_path, false, |temporary_path| {
            fs::write(temporary_path, "created").unwrap();
            Ok(())
        })
        .unwrap();

        assert_eq!(mode_of(&output_path), 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn an_existing_destination_gains_no_bits_outside_the_permission_bits() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let output_path = temp_dir.path().join("setgid-output.csv");
        fs::write(&output_path, "existing").unwrap();
        fs::set_permissions(&output_path, fs::Permissions::from_mode(0o2644)).unwrap();

        replace_file_atomically(&output_path, true, |temporary_path| {
            fs::write(temporary_path, "replacement").unwrap();
            Ok(())
        })
        .unwrap();

        // Unmasked on purpose: `mode_of` masks with 0o777, which is what hid a
        // publish that carried setgid and the file-type bits into `from_mode`.
        assert_eq!(
            fs::metadata(&output_path).unwrap().permissions().mode(),
            0o100644
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_destination_is_published_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let target_path = temp_dir.path().join("someone-elses.csv");
        let output_path = temp_dir.path().join("output.csv");
        fs::write(&target_path, "target").unwrap();
        fs::set_permissions(&target_path, fs::Permissions::from_mode(0o666)).unwrap();
        std::os::unix::fs::symlink(&target_path, &output_path).unwrap();

        replace_file_atomically(&output_path, true, |temporary_path| {
            fs::write(temporary_path, "anonymized").unwrap();
            Ok(())
        })
        .unwrap();

        // A symlink planted at the destination must not choose who may read the
        // output: `rename` replaced the link, so the target's 0o666 was never the
        // published file's mode to inherit.
        assert_eq!(mode_of(&output_path), 0o600);
        assert!(!fs::symlink_metadata(&output_path).unwrap().is_symlink());
        assert_eq!(fs::read_to_string(&output_path).unwrap(), "anonymized");
        assert_eq!(fs::read_to_string(&target_path).unwrap(), "target");
    }

    #[test]
    fn no_clobber_publish_maps_a_missing_hard_link_to_the_overwrite_remedy() {
        let output_path = Path::new("/media/stick/output.csv");

        let existing = no_clobber_publish_error(
            std::io::Error::from(std::io::ErrorKind::AlreadyExists),
            output_path,
        );
        // Linux reports a filesystem without `.link` as EPERM, not ENOSYS, so the
        // FAT32/exFAT case this message exists for only reaches users if
        // PermissionDenied maps here too.
        let no_hard_links = [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::Unsupported,
        ]
        .map(|kind| no_clobber_publish_error(std::io::Error::from(kind), output_path));
        let unrelated = no_clobber_publish_error(
            std::io::Error::from(std::io::ErrorKind::StorageFull),
            output_path,
        );

        assert!(matches!(existing, AnonymizerError::OutputExists(_)));
        for error in &no_hard_links {
            assert!(
                error.to_string().contains("Enable overwrite"),
                "error was {error}"
            );
            assert!(!error.to_string().contains("different filesystem"));
        }
        assert!(matches!(unrelated, AnonymizerError::Io(_)));
        assert!(!unrelated.to_string().contains("Enable overwrite"));
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
