//! The project's files offered by a Files block: patterns relative to the project folder,
//! `/` as separator, `*` within a folder, `**` across folders. Read-only; nothing is opened.

use cargoframe_context::glob;
use serde::Serialize;
use std::fs;
use std::path::Path;

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
}

/// Folders never entered: version-control internals and dependency caches are large and
/// almost never what a person wants to attach.
const SKIPPED_DIRS: [&str; 2] = [".git", "node_modules"];

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
                if depth < limits.max_depth && !SKIPPED_DIRS.contains(&name.as_str()) {
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
}
