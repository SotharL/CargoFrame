//! Put a finished zip into a folder without ever leaving a half-written file that looks done.

use cargoframe_safe::ladder::WriteFailure;
use cargoframe_safe::naming::{part_name, unique_name};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn classify(e: &io::Error) -> WriteFailure {
    match e.kind() {
        io::ErrorKind::PermissionDenied
        | io::ErrorKind::NotFound
        | io::ErrorKind::ReadOnlyFilesystem => WriteFailure::NotWritable,
        io::ErrorKind::StorageFull | io::ErrorKind::QuotaExceeded => WriteFailure::DiskFull,
        _ => match e.raw_os_error() {
            // ENOSPC on Unix, ERROR_DISK_FULL / ERROR_HANDLE_DISK_FULL on Windows.
            Some(28) | Some(112) | Some(39) => WriteFailure::DiskFull,
            _ => WriteFailure::Other(e.to_string()),
        },
    }
}

/// Copy `source` to `dir/<file_name>` through `<file_name>.part`, choosing a free name if the
/// file already exists (`x-2.zip`). Returns the final path. The folder must already exist: a
/// mistyped path is reported, never silently created.
pub fn copy_atomic(source: &Path, dir: &Path, file_name: &str) -> Result<PathBuf, WriteFailure> {
    if !dir.is_dir() {
        return Err(WriteFailure::NotWritable);
    }
    let final_name = unique_name(file_name, |n| dir.join(n).exists());
    let part = dir.join(part_name(&final_name));
    let target = dir.join(&final_name);
    let result = fs::copy(source, &part).and_then(|_| {
        // Flush to disk before the rename, so a power cut cannot leave a short file.
        fs::File::open(&part).and_then(|f| f.sync_all())?;
        fs::rename(&part, &target)
    });
    match result {
        Ok(()) => Ok(target),
        Err(e) => {
            let _ = fs::remove_file(&part);
            Err(classify(&e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_and_leaves_no_part_file() {
        let src = tempfile::NamedTempFile::new().unwrap();
        fs::write(src.path(), b"zipdata").unwrap();
        let dir = tempfile::tempdir().unwrap();
        let out = copy_atomic(src.path(), dir.path(), "a.zip").unwrap();
        assert_eq!(out, dir.path().join("a.zip"));
        assert_eq!(fs::read(&out).unwrap(), b"zipdata");
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
    }

    #[test]
    fn never_overwrites_an_existing_zip() {
        let src = tempfile::NamedTempFile::new().unwrap();
        fs::write(src.path(), b"new").unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.zip"), b"old").unwrap();
        let out = copy_atomic(src.path(), dir.path(), "a.zip").unwrap();
        assert_eq!(out, dir.path().join("a-2.zip"));
        assert_eq!(fs::read(dir.path().join("a.zip")).unwrap(), b"old");
    }

    #[test]
    fn a_missing_folder_is_reported_not_created() {
        let src = tempfile::NamedTempFile::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        assert_eq!(
            copy_atomic(src.path(), &missing, "a.zip"),
            Err(WriteFailure::NotWritable)
        );
        assert!(!missing.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_folder_is_not_writable() {
        use std::os::unix::fs::PermissionsExt;
        // Root ignores permission bits, so this check only means something for other users.
        if running_as_root() {
            return;
        }
        let src = tempfile::NamedTempFile::new().unwrap();
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
        let r = copy_atomic(src.path(), dir.path(), "a.zip");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(r, Err(WriteFailure::NotWritable));
    }

    #[cfg(unix)]
    fn running_as_root() -> bool {
        std::env::var("USER").map(|u| u == "root").unwrap_or(false)
            || fs::metadata("/proc/self")
                .map(|m| {
                    use std::os::unix::fs::MetadataExt;
                    m.uid() == 0
                })
                .unwrap_or(false)
    }
}
