//! The project's files offered by a Files block: patterns relative to the project folder,
//! `/` as separator, `*` within a folder, `**` across folders. Read-only; nothing is opened.

use cargoframe_context::glob;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileEntry {
    /// Path relative to the project folder, with `/` separators.
    pub path: String,
    pub size: u64,
    /// Ticked by default.
    pub selected: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_depth: usize,
    /// Directory entries looked at in total; stops runaway scans of huge trees.
    pub max_visited: usize,
    pub max_results: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 8,
            max_visited: 50_000,
            max_results: 500,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Scan {
    pub entries: Vec<FileEntry>,
    /// The scan stopped at a limit; more files may exist.
    pub truncated: bool,
    /// Why nothing could be listed, if so.
    pub problem: Option<String>,
    /// What each folder looked in held. Empty for a scan of one folder with [`scan`].
    pub places: Vec<PlaceFound>,
}

/// One folder a Files block looked in, and how many files it offered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlaceFound {
    /// What the choice is called ("Project folder", "Two folders above the program").
    pub label: String,
    pub folder: String,
    /// False when the folder (or the subfolder in it) is not there.
    pub exists: bool,
    pub found: usize,
}

/// Folders never entered: version-control internals and dependency caches are large and
/// almost never what a person wants to attach.
const SKIPPED_DIRS: [&str; 2] = [".git", "node_modules"];

/// How deep the patterns can reach: `*.log` only matches files in the folder itself, and
/// `logs/*.log` one folder down, so nothing deeper needs to be listed. A pattern with `**` can
/// match at any depth.
fn reach(include: &[&str], max_depth: usize) -> usize {
    include
        .iter()
        .map(|p| {
            let p = p.replace('\\', "/");
            if p.contains("**") {
                max_depth
            } else {
                p.matches('/').count()
            }
        })
        .max()
        .unwrap_or(0)
        .min(max_depth)
}

fn usable(pattern: &str) -> bool {
    !pattern.trim().is_empty()
        && !pattern.contains('\0')
        && !pattern.split(['/', '\\']).any(|part| part == "..")
}

pub fn scan(root: &Path, include: &[String], preselect: &[String], limits: Limits) -> Scan {
    let include: Vec<&str> = include
        .iter()
        .map(String::as_str)
        .filter(|p| usable(p))
        .collect();
    let preselect: Vec<&str> = preselect
        .iter()
        .map(String::as_str)
        .filter(|p| usable(p))
        .collect();
    let mut out = Scan::default();
    if include.is_empty() {
        return out;
    }
    if !root.is_dir() {
        out.problem = Some(format!("{} is not a folder", root.display()));
        return out;
    }
    let ignore_case = cfg!(windows);
    let reach = reach(&include, limits.max_depth);
    let mut visited = 0usize;
    let mut stack = vec![(root.to_path_buf(), String::new(), 0usize)];
    while let Some((dir, rel_dir, depth)) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue, // unreadable folders are skipped silently
        };
        for entry in entries.filter_map(Result::ok) {
            visited += 1;
            if visited > limits.max_visited {
                out.truncated = true;
                stack.clear();
                break;
            }
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(meta) = fs::symlink_metadata(entry.path()) else {
                continue;
            };
            let rel = if rel_dir.is_empty() {
                name.clone()
            } else {
                format!("{rel_dir}/{name}")
            };
            if meta.is_dir() {
                if depth < reach && !SKIPPED_DIRS.contains(&name.as_str()) {
                    stack.push((entry.path(), rel, depth + 1));
                }
            } else if meta.is_file() && include.iter().any(|p| glob::matches(p, &rel, ignore_case))
            {
                if out.entries.len() >= limits.max_results {
                    out.truncated = true;
                    continue;
                }
                let selected = preselect
                    .iter()
                    .any(|p| glob::matches(p, &rel, ignore_case));
                out.entries.push(FileEntry {
                    path: rel,
                    size: meta.len(),
                    selected,
                });
            }
        }
    }
    out.entries.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

/// A folder to look in, with what to call it.
#[derive(Debug, Clone)]
pub struct Where {
    pub label: String,
    pub dir: PathBuf,
}

/// Look in several folders and list what they hold as one list. Every path is relative to
/// `root`, which must be one of the folders or above all of them, so a file has one name
/// whichever folder found it. Patterns are matched inside each folder, as for [`scan`]. A folder
/// that is not there is skipped and shown as missing; a problem is reported only when none is.
pub fn scan_places(
    root: &Path,
    places: &[Where],
    include: &[String],
    preselect: &[String],
    limits: Limits,
) -> Scan {
    let mut out = Scan::default();
    let mut merged: BTreeMap<String, FileEntry> = BTreeMap::new();
    for place in places {
        let one = scan(&place.dir, include, preselect, limits);
        let exists = place.dir.is_dir();
        let prefix = place
            .dir
            .strip_prefix(root)
            .ok()
            .map(|p| {
                p.components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .unwrap_or_default();
        out.truncated |= one.truncated;
        let mut found = 0;
        for mut entry in one.entries {
            if !prefix.is_empty() {
                entry.path = format!("{prefix}/{}", entry.path);
            }
            found += 1;
            merged
                .entry(entry.path.clone())
                .and_modify(|e| e.selected |= entry.selected)
                .or_insert(entry);
        }
        out.places.push(PlaceFound {
            label: place.label.clone(),
            folder: place.dir.display().to_string(),
            exists,
            found,
        });
        if !exists && places.len() == 1 {
            out.problem = one.problem;
        }
    }
    if places.len() > 1 && out.places.iter().all(|p| !p.exists) {
        out.problem = Some("None of the folders to look in exist".into());
    }
    out.entries = merged.into_values().collect();
    if out.entries.len() > limits.max_results {
        out.entries.truncate(limits.max_results);
        out.truncated = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, body: &str| {
            let path = d.path().join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        };
        w("app.log", "1234");
        w("notes.txt", "x");
        w("logs/a.log", "aa");
        w("logs/deep/b.log", "bbb");
        w("src/main.rs", "fn main(){}");
        w(".git/config", "secret");
        w("node_modules/x/y.log", "no");
        d
    }

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn star_stays_in_the_folder_and_double_star_crosses_folders() {
        let d = tree();
        let top = scan(d.path(), &s(&["*.log"]), &[], Limits::default());
        assert_eq!(
            top.entries
                .iter()
                .map(|e| e.path.as_str())
                .collect::<Vec<_>>(),
            ["app.log"]
        );
        let all = scan(d.path(), &s(&["**/*.log"]), &[], Limits::default());
        let paths: Vec<&str> = all.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["app.log", "logs/a.log", "logs/deep/b.log"]);
    }

    #[test]
    fn preselect_ticks_a_subset_and_sizes_are_reported() {
        let d = tree();
        let r = scan(
            d.path(),
            &s(&["**/*.log", "*.txt"]),
            &s(&["logs/*.log", "notes.txt"]),
            Limits::default(),
        );
        let sel: Vec<(&str, bool)> = r
            .entries
            .iter()
            .map(|e| (e.path.as_str(), e.selected))
            .collect();
        assert_eq!(
            sel,
            [
                ("app.log", false),
                ("logs/a.log", true),
                ("logs/deep/b.log", false),
                ("notes.txt", true)
            ]
        );
        assert_eq!(r.entries[0].size, 4);
    }

    #[test]
    fn git_and_node_modules_are_never_entered() {
        let d = tree();
        let r = scan(d.path(), &s(&["**/*"]), &[], Limits::default());
        assert!(r
            .entries
            .iter()
            .all(|e| !e.path.starts_with(".git") && !e.path.starts_with("node_modules")));
        assert!(r.entries.iter().any(|e| e.path == "src/main.rs"));
    }

    #[test]
    fn escaping_patterns_are_ignored() {
        let d = tree();
        assert!(scan(
            d.path(),
            &s(&["../*", "..\\x", "  "]),
            &[],
            Limits::default()
        )
        .entries
        .is_empty());
    }

    #[test]
    fn limits_set_the_truncated_flag() {
        let d = tree();
        let r = scan(
            d.path(),
            &s(&["**/*.log"]),
            &[],
            Limits {
                max_results: 2,
                ..Limits::default()
            },
        );
        assert_eq!(r.entries.len(), 2);
        assert!(r.truncated);
        let r = scan(
            d.path(),
            &s(&["**/*.log"]),
            &[],
            Limits {
                max_depth: 1,
                ..Limits::default()
            },
        );
        assert!(r.entries.iter().all(|e| e.path != "logs/deep/b.log"));
    }

    #[test]
    fn missing_folder_is_reported_not_fatal() {
        let r = scan(
            Path::new("/definitely/not/here"),
            &s(&["*"]),
            &[],
            Limits::default(),
        );
        assert!(r.entries.is_empty());
        assert!(r.problem.is_some());
    }

    #[test]
    fn no_patterns_means_no_work() {
        let d = tree();
        assert_eq!(scan(d.path(), &[], &[], Limits::default()), Scan::default());
    }

    fn where_(label: &str, dir: PathBuf) -> Where {
        Where {
            label: label.into(),
            dir,
        }
    }

    #[test]
    fn several_folders_give_one_list_named_from_the_root() {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str, body: &str| {
            let path = d.path().join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, body).unwrap();
        };
        w("foo/run.log", "1");
        w("foo/logs/a.log", "22");
        w("foo/bin/release/logs/b.log", "333");
        w("foo/bin/release/run.log", "4444");
        let foo = d.path().join("foo");
        let places = [
            where_("Project folder", foo.clone()),
            where_("Program folder", foo.join("bin/release")),
        ];
        let scan = scan_places(
            &foo,
            &places,
            &s(&["*.log"]),
            &s(&["run.log"]),
            Limits::default(),
        );
        let paths: Vec<_> = scan.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["bin/release/run.log", "run.log"]);
        assert!(scan.entries.iter().all(|e| e.selected));
        assert_eq!(scan.places.len(), 2);
        assert_eq!(scan.places[0].found, 1);
        assert_eq!(scan.places[1].found, 1);
        assert!(scan.problem.is_none());
    }

    #[test]
    fn a_subfolder_in_each_folder_and_a_root_above_them_all() {
        let d = tempfile::tempdir().unwrap();
        let w = |p: &str| {
            let path = d.path().join(p);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "x").unwrap();
        };
        w("dev/foo/logs/a.log");
        w("dev/foo/bin/release/logs/b.log");
        w("dev/logs/c.log");
        w("dev/other/logs/never.log");
        let dev = d.path().join("dev");
        let places = [
            where_("Three above", dev.join("logs")),
            where_("Two above", dev.join("foo/logs")),
            where_("Program", dev.join("foo/bin/release/logs")),
        ];
        let scan = scan_places(&dev, &places, &s(&["*.log"]), &[], Limits::default());
        let paths: Vec<_> = scan.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(
            paths,
            ["foo/bin/release/logs/b.log", "foo/logs/a.log", "logs/c.log"]
        );
    }

    #[test]
    fn overlapping_folders_list_a_file_once_and_keep_it_ticked() {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("sub")).unwrap();
        fs::write(d.path().join("sub/x.log"), "x").unwrap();
        let places = [
            where_("Whole", d.path().to_path_buf()),
            where_("Sub", d.path().join("sub")),
        ];
        let scan = scan_places(
            d.path(),
            &places,
            &s(&["**/*.log"]),
            &s(&["sub/x.log"]),
            Limits::default(),
        );
        assert_eq!(scan.entries.len(), 1);
        assert_eq!(scan.entries[0].path, "sub/x.log");
        // The second folder sees it as x.log: the tick comes from the first, which knows the full path.
        assert!(scan.entries[0].selected);
    }

    #[test]
    fn a_missing_folder_is_shown_not_fatal_until_none_exist() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("a.log"), "x").unwrap();
        let places = [
            where_("Here", d.path().to_path_buf()),
            where_("Gone", d.path().join("logs")),
        ];
        let scan = scan_places(d.path(), &places, &s(&["*.log"]), &[], Limits::default());
        assert_eq!(scan.entries.len(), 1);
        assert!(scan.problem.is_none());
        assert!(!scan.places[1].exists && scan.places[1].found == 0);

        let none = scan_places(
            d.path(),
            &places[1..],
            &s(&["*.log"]),
            &[],
            Limits::default(),
        );
        assert!(none.problem.unwrap().contains("not a folder"));
        let two_gone = [
            where_("A", d.path().join("x")),
            where_("B", d.path().join("y")),
        ];
        let none = scan_places(d.path(), &two_gone, &s(&["*.log"]), &[], Limits::default());
        assert!(none.problem.unwrap().contains("None of the folders"));
        assert!(none.entries.is_empty());
    }

    #[test]
    fn the_results_limit_applies_to_the_whole_list() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..5 {
            fs::write(d.path().join(format!("{i}.log")), "x").unwrap();
        }
        fs::create_dir_all(d.path().join("s")).unwrap();
        for i in 5..10 {
            fs::write(d.path().join(format!("s/{i}.log")), "x").unwrap();
        }
        let places = [
            where_("A", d.path().to_path_buf()),
            where_("B", d.path().join("s")),
        ];
        let limits = Limits {
            max_results: 6,
            ..Limits::default()
        };
        let scan = scan_places(d.path(), &places, &s(&["*.log"]), &[], limits);
        assert_eq!(scan.entries.len(), 6);
        assert!(scan.truncated);
    }

    #[test]
    fn folders_the_patterns_cannot_reach_are_not_walked() {
        // A pattern with no `**` and no `/` only needs the folder itself; one slash, one level.
        assert_eq!(reach(&["*.log"], 8), 0);
        assert_eq!(reach(&["logs/*.log", "*.txt"], 8), 1);
        assert_eq!(
            reach(&["a/b/c/*.log"], 2),
            2,
            "never beyond the depth limit"
        );
        assert_eq!(reach(&["**/*.log"], 8), 8);
        assert_eq!(reach(&["logs\\**"], 5), 5);
        assert_eq!(reach(&[], 8), 0);
        // And it does not change what is found.
        let d = tree();
        let s1 = scan(d.path(), &s(&["logs/*.log"]), &[], Limits::default());
        let paths: Vec<_> = s1.entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["logs/a.log"]);
        let s2 = scan(d.path(), &s(&["**/*.log"]), &[], Limits::default());
        assert!(s2.entries.iter().any(|e| e.path == "logs/deep/b.log"));
    }
}
