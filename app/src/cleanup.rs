//! Clearing what CargoFrame keeps on this computer (Settings, General, Your data).
//!
//! Everything here stays inside the two folders CargoFrame owns: the data folder (work in
//! progress, kept zips, logs, downloaded updates) and the settings folder. A file that cannot be
//! removed (the log being written, say) is left and counted, never an error.

use std::fs;
use std::path::Path;

use serde::Serialize;

/// One kind of data: its name, what it is, and the folders of the data folder that hold it.
struct Kind {
    id: &'static str,
    label: &'static str,
    what: &'static str,
    folders: &'static [&'static str],
}

const KINDS: &[Kind] = &[
    Kind {
        id: "work",
        label: "Captures and recordings in progress",
        what: "Kept for a little while so a window can use them, then removed by themselves.",
        folders: &["captures", "video-recording", "video-test", "tmp"],
    },
    Kind {
        id: "kept",
        label: "Zips kept here",
        what: "Saved here when the folder for saved zips could not be used.",
        folders: &["kept"],
    },
    Kind {
        id: "logs",
        label: "Log files",
        what: "What CargoFrame wrote about itself, for finding a problem.",
        folders: &["logs"],
    },
    Kind {
        id: "updates",
        label: "Downloaded updates",
        what: "Installers downloaded by an update, removed after the install.",
        folders: &["updates"],
    },
];

/// What one kind of data takes on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Area {
    pub id: &'static str,
    pub label: &'static str,
    pub what: &'static str,
    pub files: u64,
    pub bytes: u64,
}

/// What a clean-up did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Cleaned {
    pub files: u64,
    pub bytes: u64,
    /// Files that could not be removed (in use): they are still there.
    pub left: u64,
}

/// The files and bytes inside `path`; links are counted, never followed.
fn size_of(path: &Path) -> (u64, u64) {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return (0, 0);
    };
    if !meta.is_dir() {
        return (1, meta.len());
    }
    let Ok(entries) = fs::read_dir(path) else {
        return (0, 0);
    };
    entries
        .flatten()
        .map(|e| size_of(&e.path()))
        .fold((0, 0), |(f, b), (more_f, more_b)| (f + more_f, b + more_b))
}

/// What each kind of data takes now.
pub fn report(data: &Path) -> Vec<Area> {
    KINDS
        .iter()
        .map(|k| {
            let (files, bytes) = k
                .folders
                .iter()
                .map(|f| size_of(&data.join(f)))
                .fold((0, 0), |(a, b), (c, d)| (a + c, b + d));
            Area {
                id: k.id,
                label: k.label,
                what: k.what,
                files,
                bytes,
            }
        })
        .collect()
}

/// Remove what is inside `dir` (the folder itself stays), counting what went and what stayed.
fn empty(dir: &Path, into: &mut Cleaned) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            empty(&path, into);
            // a folder with something left in it stays
            let _ = fs::remove_dir(&path);
        } else if fs::remove_file(&path).is_ok() {
            into.files += 1;
            into.bytes += meta.len();
        } else {
            into.left += 1;
        }
    }
}

/// Remove the kinds of data named by `ids` (unknown names are ignored).
pub fn clean(data: &Path, ids: &[String]) -> Cleaned {
    let mut done = Cleaned::default();
    for k in KINDS.iter().filter(|k| ids.iter().any(|i| i == k.id)) {
        for folder in k.folders {
            empty(&data.join(folder), &mut done);
        }
    }
    done
}

/// Remove every kind of data.
pub fn clean_all(data: &Path) -> Cleaned {
    let ids: Vec<String> = KINDS.iter().map(|k| k.id.to_string()).collect();
    clean(data, &ids)
}

/// Remove every settings file: triggers, layouts, values from files, connections, General and the
/// rest. The next start finds an empty folder, as a new install does.
pub fn reset_settings(config: &Path) -> Cleaned {
    let mut done = Cleaned::default();
    empty(config, &mut done);
    done
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, bytes: usize) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn it_counts_what_each_kind_of_data_takes() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "captures/a/shot.png", 100);
        write(d.path(), "tmp/zip.part", 50);
        write(d.path(), "logs/cargoframe.log", 10);
        let areas = report(d.path());
        let by = |id: &str| areas.iter().find(|a| a.id == id).unwrap().clone();
        assert_eq!((by("work").files, by("work").bytes), (2, 150));
        assert_eq!((by("logs").files, by("logs").bytes), (1, 10));
        assert_eq!((by("kept").files, by("updates").files), (0, 0));
    }

    #[test]
    fn it_removes_only_the_kinds_asked_for_and_keeps_the_folders() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "kept/z.zip", 20);
        write(d.path(), "logs/a.log", 5);
        write(d.path(), "captures/x/y/z.png", 7);
        let done = clean(d.path(), &ids(&["kept", "work", "nonsense"]));
        assert_eq!((done.files, done.bytes, done.left), (2, 27, 0));
        assert!(!d.path().join("kept/z.zip").exists());
        assert!(!d.path().join("captures/x").exists());
        assert!(d.path().join("kept").is_dir(), "the folder itself stays");
        // the logs were not asked for
        assert!(d.path().join("logs/a.log").exists());
    }

    #[test]
    fn it_never_touches_a_folder_that_is_not_one_of_its_own() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "somewhere-else/keep.txt", 3);
        write(d.path(), "kept/z.zip", 1);
        clean_all(d.path());
        assert!(d.path().join("somewhere-else/keep.txt").exists());
    }

    #[test]
    fn a_folder_that_is_not_there_is_not_an_error() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(clean_all(d.path()), Cleaned::default());
        assert!(report(d.path()).iter().all(|a| a.files == 0 && a.bytes == 0));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_removed_but_what_it_points_to_is_not() {
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        write(outside.path(), "precious.txt", 9);
        fs::create_dir_all(d.path().join("kept")).unwrap();
        std::os::unix::fs::symlink(outside.path(), d.path().join("kept/link")).unwrap();
        clean(d.path(), &ids(&["kept"]));
        assert!(outside.path().join("precious.txt").exists());
    }

    #[test]
    fn a_reset_empties_the_settings_folder() {
        let d = tempfile::tempdir().unwrap();
        write(d.path(), "general.json", 4);
        write(d.path(), "layouts/bug.json", 4);
        write(d.path(), "connections/jira.json", 4);
        let done = reset_settings(d.path());
        assert_eq!(done.files, 3);
        assert!(d.path().is_dir());
        assert_eq!(fs::read_dir(d.path()).unwrap().count(), 0);
    }
}
