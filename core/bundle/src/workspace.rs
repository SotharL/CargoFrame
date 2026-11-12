//! The folder one capture lives in while its window is open.
//!
//! A capture gets its own folder under CargoFrame's data folder. The screenshot and the files
//! the window offers are copied into it, so what is sent, zipped or saved is what was there at
//! the moment of the capture (a log keeps growing; a share must not catch it half way), and so a
//! destination that needs real files on disk (a chat that takes pasted files) has them.
//!
//! The window owns the folder:
//!
//! * [`CaptureDir`] deletes its folder when it is dropped, which is when the window shows another
//!   capture, is closed, or CargoFrame quits.
//! * If CargoFrame is killed, the folder is left behind. [`Workspace::sweep`] deletes every
//!   folder that no open window owns; it runs at start-up and every hour.
//! * Only folders that carry CargoFrame's marker file are ever deleted, so a mistaken path can
//!   never take anything else with it.

use crate::build::{open_project_file, BundleError};
use crate::entry::safe_name;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The file that says "this folder belongs to a capture". Nothing without it is deleted.
pub const MARKER: &str = ".cargoframe-capture";
const PREFIX: &str = "capture-";
const FILES: &str = "files";
const SCREENSHOT: &str = "screenshot.png";

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// All capture folders live under one root.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

/// What a sweep did.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Sweep {
    pub removed: usize,
    pub kept: usize,
    pub failed: usize,
}

impl Workspace {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A fresh folder for a capture, owned by the returned [`CaptureDir`].
    pub fn create(&self, session_id: u64) -> io::Result<CaptureDir> {
        fs::create_dir_all(&self.root)?;
        let secs = now_secs();
        loop {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = self.root.join(format!(
                "{PREFIX}{secs}-{session_id}-{}-{n}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => {
                    let mut marker = fs::File::create(path.join(MARKER))?;
                    writeln!(
                        marker,
                        "CargoFrame capture folder. Safe to delete when its window is closed.\nsession={session_id}\nprocess={}\ncreated={secs}",
                        std::process::id()
                    )?;
                    return Ok(CaptureDir { path });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }

    /// Delete every capture folder that is not one of `keep` and is older than `older_than`
    /// (a folder created a moment ago may belong to a window that is still being opened).
    /// Anything without the marker file is left alone.
    pub fn sweep(&self, keep: &[PathBuf], older_than: Duration) -> Sweep {
        let mut report = Sweep::default();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return report;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let is_ours = name.to_string_lossy().starts_with(PREFIX)
                && path.is_dir()
                && path.join(MARKER).is_file();
            if !is_ours {
                continue;
            }
            if keep.iter().any(|k| same_path(k, &path)) {
                report.kept += 1;
                continue;
            }
            let age = created_secs(&path).map(|c| now_secs().saturating_sub(c));
            let too_new = age.map_or(true, |a| Duration::from_secs(a) < older_than);
            if too_new {
                report.kept += 1;
                continue;
            }
            match fs::remove_dir_all(&path) {
                Ok(()) => report.removed += 1,
                Err(_) => report.failed += 1,
            }
        }
        report
    }
}

/// One capture's folder. Dropping it deletes the folder and everything in it.
#[derive(Debug)]
pub struct CaptureDir {
    path: PathBuf,
}

impl CaptureDir {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Where staged project files are kept.
    pub fn files_dir(&self) -> PathBuf {
        self.path.join(FILES)
    }

    pub fn screenshot_path(&self) -> PathBuf {
        self.path.join(SCREENSHOT)
    }

    /// Save (or replace) the screenshot. The old one stays whole until the new one is complete.
    pub fn write_screenshot(&self, png: &[u8]) -> io::Result<PathBuf> {
        let target = self.screenshot_path();
        let temp = self.path.join(format!("{SCREENSHOT}.part"));
        fs::write(&temp, png)?;
        fs::rename(&temp, &target)?;
        Ok(target)
    }

    /// Copy a project file (`relative` to `project_root`) into this folder, once. Returns the
    /// copy. The same rules as for a zip apply: no way out of the project folder, regular files
    /// only, and nothing larger than `max_bytes`.
    pub fn stage(
        &self,
        project_root: &Path,
        relative: &str,
        max_bytes: u64,
    ) -> Result<PathBuf, BundleError> {
        let name = safe_name(relative).ok_or_else(|| BundleError::UnsafeName(relative.into()))?;
        let dest = self.files_dir().join(&name);
        if dest.is_file() {
            return Ok(dest);
        }
        let (source, len) = open_project_file(project_root, relative)?;
        copy_in(source, len, &dest, relative, max_bytes)
    }

    /// Copy a file the person chose themselves (outside any project folder) into this folder,
    /// under `relative`, once. Regular files only, nothing larger than `max_bytes`.
    pub fn adopt(
        &self,
        from: &Path,
        relative: &str,
        max_bytes: u64,
    ) -> Result<PathBuf, BundleError> {
        let name = safe_name(relative).ok_or_else(|| BundleError::UnsafeName(relative.into()))?;
        let dest = self.files_dir().join(&name);
        if dest.is_file() {
            return Ok(dest);
        }
        let unreadable = |e: &dyn std::fmt::Display| BundleError::Unreadable {
            path: relative.into(),
            reason: e.to_string(),
        };
        let meta = fs::metadata(from).map_err(|e| unreadable(&e))?;
        if !meta.is_file() {
            return Err(unreadable(&"not a regular file"));
        }
        let source = fs::File::open(from).map_err(|e| unreadable(&e))?;
        copy_in(source, meta.len(), &dest, relative, max_bytes)
    }

    /// The staged copy of a project file, if it was staged.
    pub fn staged(&self, relative: &str) -> Option<PathBuf> {
        let name = safe_name(relative)?;
        let p = self.files_dir().join(name);
        p.is_file().then_some(p)
    }
}

/// Copy `source` (`len` bytes) to `dest` through a partial name, refusing more than `max_bytes`.
fn copy_in(
    mut source: fs::File,
    len: u64,
    dest: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<PathBuf, BundleError> {
    if len > max_bytes {
        return Err(BundleError::Unreadable {
            path: relative.into(),
            reason: format!("larger than {} MB", max_bytes / (1024 * 1024)),
        });
    }
    let write_err = |e: io::Error| BundleError::Write(e.to_string());
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(write_err)?;
    }
    // Copy to a partial name, then rename, so a half-copied file is never mistaken for a
    // staged one. Read at most `max_bytes + 1` in case the file grew while it was copied.
    let part = dest.with_file_name(format!(
        "{}.part",
        dest.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    let mut out = fs::File::create(&part).map_err(write_err)?;
    let copied = io::copy(&mut (&mut source).take(max_bytes + 1), &mut out).map_err(write_err)?;
    drop(out);
    if copied > max_bytes {
        let _ = fs::remove_file(&part);
        return Err(BundleError::Unreadable {
            path: relative.into(),
            reason: format!("larger than {} MB", max_bytes / (1024 * 1024)),
        });
    }
    fs::rename(&part, dest).map_err(write_err)?;
    Ok(dest.to_path_buf())
}

impl Drop for CaptureDir {
    fn drop(&mut self) {
        remove_marked(&self.path);
    }
}

/// Delete a capture folder, but only one that carries the marker.
fn remove_marked(path: &Path) {
    if path.join(MARKER).is_file() {
        let _ = fs::remove_dir_all(path);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// The `created=` line of the marker, in seconds since 1970.
fn created_secs(folder: &Path) -> Option<u64> {
    let text = fs::read_to_string(folder.join(MARKER)).ok()?;
    text.lines()
        .find_map(|l| l.strip_prefix("created="))
        .and_then(|v| v.trim().parse().ok())
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> (tempfile::TempDir, Workspace) {
        let dir = tempfile::tempdir().unwrap();
        let w = Workspace::new(dir.path().join("captures"));
        (dir, w)
    }

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, text) in files {
            let p = dir.path().join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, text).unwrap();
        }
        dir
    }

    /// Make a folder look like it was created `secs` seconds ago.
    fn age(folder: &Path, secs: u64) {
        let text = fs::read_to_string(folder.join(MARKER)).unwrap();
        let rewritten: Vec<String> = text
            .lines()
            .map(|l| {
                if l.starts_with("created=") {
                    format!("created={}", now_secs() - secs)
                } else {
                    l.to_string()
                }
            })
            .collect();
        fs::write(folder.join(MARKER), rewritten.join("\n")).unwrap();
    }

    #[test]
    fn a_capture_gets_its_own_folder_that_goes_when_the_window_does() {
        let (_keep, w) = ws();
        let a = w.create(1).unwrap();
        let b = w.create(1).unwrap();
        assert_ne!(a.path(), b.path());
        assert!(a.path().join(MARKER).is_file());
        let shot = a.write_screenshot(b"png").unwrap();
        assert_eq!(fs::read(&shot).unwrap(), b"png");
        let gone = a.path().to_path_buf();
        drop(a);
        assert!(!gone.exists(), "dropping the capture deletes its folder");
        assert!(b.path().exists(), "another capture's folder stays");
    }

    #[test]
    fn replacing_the_screenshot_keeps_one_whole_file() {
        let (_keep, w) = ws();
        let c = w.create(1).unwrap();
        c.write_screenshot(b"first").unwrap();
        c.write_screenshot(b"second").unwrap();
        assert_eq!(fs::read(c.screenshot_path()).unwrap(), b"second");
        let names: Vec<_> = fs::read_dir(c.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!names.iter().any(|n| n.ends_with(".part")), "{names:?}");
    }

    #[test]
    fn a_sweep_deletes_what_no_window_owns_and_keeps_what_one_does() {
        let (_keep, w) = ws();
        let live = w.create(1).unwrap();
        // Two folders left by a crash: old enough, and owned by nobody.
        let orphan_a = w.create(2).unwrap();
        let orphan_b = w.create(3).unwrap();
        let (pa, pb) = (orphan_a.path().to_path_buf(), orphan_b.path().to_path_buf());
        // The crash: the owners never got to drop.
        std::mem::forget(orphan_a);
        std::mem::forget(orphan_b);
        age(&pa, 3600);
        age(&pb, 3600);
        age(live.path(), 3600);

        let report = w.sweep(&[live.path().to_path_buf()], Duration::from_secs(120));
        assert_eq!(
            report,
            Sweep {
                removed: 2,
                kept: 1,
                failed: 0
            }
        );
        assert!(!pa.exists() && !pb.exists());
        assert!(
            live.path().exists(),
            "the open window's folder is never swept"
        );
    }

    #[test]
    fn a_folder_created_a_moment_ago_is_not_swept() {
        let (_keep, w) = ws();
        let fresh = w.create(1).unwrap();
        let path = fresh.path().to_path_buf();
        std::mem::forget(fresh);
        let report = w.sweep(&[], Duration::from_secs(120));
        assert_eq!(report.removed, 0);
        assert!(path.exists());
        // Once it is old enough it goes.
        age(&path, 600);
        assert_eq!(w.sweep(&[], Duration::from_secs(120)).removed, 1);
        assert!(!path.exists());
    }

    #[test]
    fn a_sweep_never_touches_what_is_not_a_capture_folder() {
        let (_keep, w) = ws();
        fs::create_dir_all(w.root().join("capture-looks-ours-but-has-no-marker")).unwrap();
        fs::create_dir_all(w.root().join("notes")).unwrap();
        fs::write(w.root().join("capture-file.txt"), "x").unwrap();
        fs::create_dir_all(w.root().join("notes").join(PREFIX)).unwrap();
        let report = w.sweep(&[], Duration::ZERO);
        assert_eq!(report, Sweep::default());
        assert!(w
            .root()
            .join("capture-looks-ours-but-has-no-marker")
            .is_dir());
        assert!(w.root().join("notes").is_dir());
        assert!(w.root().join("capture-file.txt").is_file());
    }

    #[test]
    fn sweeping_a_folder_that_does_not_exist_is_fine() {
        let (_keep, w) = ws();
        assert_eq!(w.sweep(&[], Duration::ZERO), Sweep::default());
    }

    #[test]
    fn a_dropped_capture_whose_marker_was_removed_deletes_nothing() {
        let (_keep, w) = ws();
        let c = w.create(1).unwrap();
        let path = c.path().to_path_buf();
        fs::write(path.join("precious.txt"), "keep me").unwrap();
        fs::remove_file(path.join(MARKER)).unwrap();
        drop(c);
        assert!(path.join("precious.txt").is_file());
    }

    #[test]
    fn staging_copies_the_file_once_as_it_was_at_that_moment() {
        let (_keep, w) = ws();
        let p = project(&[("logs/app.log", "line 1\n")]);
        let c = w.create(1).unwrap();
        let first = c.stage(p.path(), "logs/app.log", 1024).unwrap();
        assert_eq!(fs::read_to_string(&first).unwrap(), "line 1\n");
        assert_eq!(first, c.files_dir().join("logs/app.log"));
        // The log grows afterwards: the staged copy is unchanged, and staging again is a no-op.
        fs::write(p.path().join("logs/app.log"), "line 1\nline 2\n").unwrap();
        let again = c.stage(p.path(), "logs/app.log", 1024).unwrap();
        assert_eq!(fs::read_to_string(again).unwrap(), "line 1\n");
        assert_eq!(c.staged("logs/app.log"), Some(first));
        assert_eq!(c.staged("logs/other.log"), None);
    }

    #[test]
    fn staging_refuses_a_way_out_of_the_project_and_files_that_are_too_big_or_missing() {
        let (_keep, w) = ws();
        let p = project(&[("a.txt", "0123456789")]);
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "s").unwrap();
        let c = w.create(1).unwrap();
        for bad in ["../secret.txt", "a/../../secret.txt", "C:\\secret.txt", ""] {
            assert!(
                matches!(
                    c.stage(p.path(), bad, 1024),
                    Err(BundleError::UnsafeName(_))
                ),
                "{bad:?}"
            );
        }
        assert!(matches!(
            c.stage(p.path(), "a.txt", 5),
            Err(BundleError::Unreadable { .. })
        ));
        assert!(
            c.staged("a.txt").is_none(),
            "a refused file leaves nothing behind"
        );
        assert!(matches!(
            c.stage(p.path(), "missing.txt", 1024),
            Err(BundleError::Unreadable { .. })
        ));
        // Nothing half-copied remains.
        let mut leftovers = Vec::new();
        collect(&c.files_dir(), &mut leftovers);
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leads_outside_the_project_is_not_followed() {
        let (_keep, w) = ws();
        let p = project(&[("a.txt", "x")]);
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "s").unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), p.path().join("link.txt"))
            .unwrap();
        let c = w.create(1).unwrap();
        assert!(matches!(
            c.stage(p.path(), "link.txt", 1024),
            Err(BundleError::Escapes { .. })
        ));
        assert!(c.staged("link.txt").is_none());
    }

    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect(&p, out);
                } else {
                    out.push(p);
                }
            }
        }
    }
}
