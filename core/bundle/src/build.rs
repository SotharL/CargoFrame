//! Build the zip and its manifest.

use crate::entry::{safe_name, Names};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    #[error("\"{0}\" cannot be put in a zip (unsafe name)")]
    UnsafeName(String),
    #[error("cannot read {path}: {reason}")]
    Unreadable { path: String, reason: String },
    #[error("{path} is a link that leads outside the project folder")]
    Escapes { path: String },
    #[error("{count} files would make the zip larger than {limit_mb} MB")]
    TooBig { count: usize, limit_mb: u64 },
    #[error("too many files ({0})")]
    TooMany(usize),
    #[error("writing the zip failed: {0}")]
    Write(String),
}

/// Where an item's bytes come from.
#[derive(Debug, Clone)]
pub enum Source {
    Bytes(Vec<u8>),
    /// A file in the project folder. It is read only if, with links followed, it is still
    /// inside `root`.
    ProjectFile {
        root: PathBuf,
        relative: String,
    },
}

#[derive(Debug, Clone)]
pub struct Item {
    /// Name inside the zip, for example `screenshot.png` or `files/logs/app.log`.
    pub name: String,
    pub source: Source,
    /// What it is, for the manifest: `screenshot`, `text`, `file`.
    pub kind: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_files: usize,
    pub max_total_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_files: 500,
            max_total_bytes: 200 * 1024 * 1024,
        }
    }
}

/// Local date and time of creation, from the caller (this crate reads no clock).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Created {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Created {
    pub fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub tool: String,
    pub version: String,
    pub created: String,
    /// The trigger that made the capture.
    pub trigger: String,
    /// Where it was meant to go ("Jira", "Claude", ...).
    pub service: String,
    pub files: Vec<ManifestFile>,
    /// Anything that did not go as planned, in words.
    pub notes: Vec<String>,
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn options(created: Created) -> SimpleFileOptions {
    let when = DateTime::from_date_and_time(
        created.year,
        created.month,
        created.day,
        created.hour,
        created.minute,
        created.second,
    )
    .unwrap_or_default();
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(when)
        .unix_permissions(0o644)
}

fn unreadable(path: &str, e: impl std::fmt::Display) -> BundleError {
    BundleError::Unreadable {
        path: path.to_string(),
        reason: e.to_string(),
    }
}

/// Check a project file and open it.
fn open_project_file(root: &Path, relative: &str) -> Result<(fs::File, u64), BundleError> {
    if safe_name(relative).is_none() {
        return Err(BundleError::UnsafeName(relative.to_string()));
    }
    let root = root.canonicalize().map_err(|e| unreadable(relative, e))?;
    let real = root
        .join(relative)
        .canonicalize()
        .map_err(|e| unreadable(relative, e))?;
    if !real.starts_with(&root) {
        return Err(BundleError::Escapes {
            path: relative.to_string(),
        });
    }
    let meta = fs::metadata(&real).map_err(|e| unreadable(relative, e))?;
    if !meta.is_file() {
        return Err(unreadable(relative, "not a regular file"));
    }
    let file = fs::File::open(&real).map_err(|e| unreadable(relative, e))?;
    Ok((file, meta.len()))
}

/// Write the zip to `out` and return its manifest. The manifest is also stored in the zip as
/// `manifest.json`, last, once every checksum is known.
///
/// Files that cannot be read are skipped and named in `notes` instead of failing the whole
/// share, except an item that tries to escape its folder, which is always an error.
pub fn build_zip<W: Write + Seek>(
    out: W,
    items: Vec<Item>,
    info: ManifestInfo,
    limits: Limits,
) -> Result<Manifest, BundleError> {
    if items.len() > limits.max_files {
        return Err(BundleError::TooMany(items.len()));
    }
    let mut zip = ZipWriter::new(out);
    let opts = options(info.created);
    let mut names = Names::default();
    let mut files = Vec::new();
    let mut notes = info.notes;
    let mut total: u64 = 0;

    for item in items {
        let name =
            safe_name(&item.name).ok_or_else(|| BundleError::UnsafeName(item.name.clone()))?;
        let name = names.unique(&name);
        let mut hasher = Sha256::new();
        let mut size: u64 = 0;
        match item.source {
            Source::Bytes(bytes) => {
                total += bytes.len() as u64;
                if total > limits.max_total_bytes {
                    return Err(BundleError::TooBig {
                        count: files.len() + 1,
                        limit_mb: limits.max_total_bytes / (1024 * 1024),
                    });
                }
                zip.start_file(&name, opts)
                    .map_err(|e| BundleError::Write(e.to_string()))?;
                zip.write_all(&bytes)
                    .map_err(|e| BundleError::Write(e.to_string()))?;
                hasher.update(&bytes);
                size = bytes.len() as u64;
            }
            Source::ProjectFile { root, relative } => {
                let (mut file, len) = match open_project_file(&root, &relative) {
                    Ok(f) => f,
                    Err(e @ BundleError::Escapes { .. }) | Err(e @ BundleError::UnsafeName(_)) => {
                        return Err(e)
                    }
                    Err(e) => {
                        notes.push(format!("skipped {relative}: {e}"));
                        continue;
                    }
                };
                total += len;
                if total > limits.max_total_bytes {
                    return Err(BundleError::TooBig {
                        count: files.len() + 1,
                        limit_mb: limits.max_total_bytes / (1024 * 1024),
                    });
                }
                zip.start_file(&name, opts)
                    .map_err(|e| BundleError::Write(e.to_string()))?;
                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    let n = file.read(&mut buf).map_err(|e| unreadable(&relative, e))?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buf[..n]);
                    zip.write_all(&buf[..n])
                        .map_err(|e| BundleError::Write(e.to_string()))?;
                    size += n as u64;
                }
            }
        }
        files.push(ManifestFile {
            name,
            kind: item.kind.to_string(),
            size,
            sha256: hex(&hasher.finalize()),
        });
    }

    let manifest = Manifest {
        tool: "CargoFrame".into(),
        version: info.version,
        created: info.created.iso(),
        trigger: info.trigger,
        service: info.service,
        files,
        notes,
    };
    let json =
        serde_json::to_vec_pretty(&manifest).map_err(|e| BundleError::Write(e.to_string()))?;
    zip.start_file("manifest.json", opts)
        .map_err(|e| BundleError::Write(e.to_string()))?;
    zip.write_all(&json)
        .map_err(|e| BundleError::Write(e.to_string()))?;
    zip.finish()
        .map_err(|e| BundleError::Write(e.to_string()))?;
    Ok(manifest)
}

/// The parts of the manifest that come from the caller.
#[derive(Debug, Clone)]
pub struct ManifestInfo {
    pub version: String,
    pub created: Created,
    pub trigger: String,
    pub service: String,
    pub notes: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn info() -> ManifestInfo {
        ManifestInfo {
            version: "0.1.0".into(),
            created: Created {
                year: 2026,
                month: 10,
                day: 23,
                hour: 15,
                minute: 30,
                second: 45,
            },
            trigger: "Bug report".into(),
            service: "Jira".into(),
            notes: vec![],
        }
    }

    fn read_back(bytes: Vec<u8>) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        zip::ZipArchive::new(Cursor::new(bytes)).unwrap()
    }

    fn entry_text(z: &mut zip::ZipArchive<Cursor<Vec<u8>>>, name: &str) -> String {
        let mut s = String::new();
        z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
        s
    }

    #[test]
    fn builds_a_zip_with_checksums_and_a_manifest_that_matches() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("logs")).unwrap();
        fs::write(dir.path().join("logs/app.log"), "hello log").unwrap();
        let items = vec![
            Item {
                name: "screenshot.png".into(),
                source: Source::Bytes(vec![0x89, b'P', b'N', b'G']),
                kind: "screenshot",
            },
            Item {
                name: "prompt.txt".into(),
                source: Source::Bytes(b"why?".to_vec()),
                kind: "text",
            },
            Item {
                name: "files/logs/app.log".into(),
                source: Source::ProjectFile {
                    root: dir.path().into(),
                    relative: "logs/app.log".into(),
                },
                kind: "file",
            },
        ];
        let mut out = Cursor::new(Vec::new());
        let manifest = build_zip(&mut out, items, info(), Limits::default()).unwrap();
        assert_eq!(manifest.files.len(), 3);
        assert_eq!(manifest.created, "2026-10-23T15:30:45");

        let mut z = read_back(out.into_inner());
        assert_eq!(entry_text(&mut z, "prompt.txt"), "why?");
        assert_eq!(entry_text(&mut z, "files/logs/app.log"), "hello log");
        // The manifest inside the zip equals the one returned, and its hashes are right.
        let inside: Manifest = serde_json::from_str(&entry_text(&mut z, "manifest.json")).unwrap();
        assert_eq!(inside, manifest);
        let log = manifest
            .files
            .iter()
            .find(|f| f.name == "files/logs/app.log")
            .unwrap();
        assert_eq!(log.size, 9);
        assert_eq!(log.sha256, hex(&Sha256::digest(b"hello log")));
    }

    #[test]
    fn duplicate_names_are_kept_apart() {
        let item = |text: &str| Item {
            name: "note.txt".into(),
            source: Source::Bytes(text.as_bytes().to_vec()),
            kind: "text",
        };
        let mut out = Cursor::new(Vec::new());
        let m = build_zip(
            &mut out,
            vec![item("one"), item("two")],
            info(),
            Limits::default(),
        )
        .unwrap();
        let names: Vec<_> = m.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["note.txt", "note-2.txt"]);
        let mut z = read_back(out.into_inner());
        assert_eq!(entry_text(&mut z, "note-2.txt"), "two");
    }

    #[test]
    fn an_unreadable_file_is_noted_and_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let items = vec![Item {
            name: "files/gone.log".into(),
            source: Source::ProjectFile {
                root: dir.path().into(),
                relative: "gone.log".into(),
            },
            kind: "file",
        }];
        let mut out = Cursor::new(Vec::new());
        let m = build_zip(&mut out, items, info(), Limits::default()).unwrap();
        assert!(m.files.is_empty());
        assert!(m.notes[0].contains("skipped gone.log"), "{:?}", m.notes);
    }

    #[test]
    fn unsafe_names_and_escaping_paths_are_errors_not_skips() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = Cursor::new(Vec::new());
        let bad_name = vec![Item {
            name: "../evil".into(),
            source: Source::Bytes(vec![]),
            kind: "text",
        }];
        assert!(matches!(
            build_zip(&mut out, bad_name, info(), Limits::default()),
            Err(BundleError::UnsafeName(_))
        ));
        let escaping = vec![Item {
            name: "files/x".into(),
            source: Source::ProjectFile {
                root: dir.path().into(),
                relative: "../x".into(),
            },
            kind: "file",
        }];
        assert!(matches!(
            build_zip(&mut out, escaping, info(), Limits::default()),
            Err(BundleError::UnsafeName(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_leaving_the_project_is_refused() {
        let project = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), "s3cret").unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), project.path().join("l.txt"))
            .unwrap();
        let items = vec![Item {
            name: "files/l.txt".into(),
            source: Source::ProjectFile {
                root: project.path().into(),
                relative: "l.txt".into(),
            },
            kind: "file",
        }];
        let mut out = Cursor::new(Vec::new());
        assert!(matches!(
            build_zip(&mut out, items, info(), Limits::default()),
            Err(BundleError::Escapes { .. })
        ));
    }

    #[test]
    fn size_and_count_limits_apply() {
        let big = |n: usize| Item {
            name: format!("f{n}.bin"),
            source: Source::Bytes(vec![0u8; 600]),
            kind: "file",
        };
        let mut out = Cursor::new(Vec::new());
        let tight = Limits {
            max_files: 3,
            max_total_bytes: 1000,
        };
        assert!(matches!(
            build_zip(&mut out, vec![big(1), big(2)], info(), tight),
            Err(BundleError::TooBig { .. })
        ));
        assert!(matches!(
            build_zip(&mut out, (0..4).map(big).collect(), info(), tight),
            Err(BundleError::TooMany(4))
        ));
    }
}
