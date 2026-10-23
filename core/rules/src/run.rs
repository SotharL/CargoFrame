//! Find a rule's file safely and run every rule.

use crate::extract::{extract, RuleError};
use crate::model::{ReadMode, Rule};
use cargoframe_context::glob;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Reading the project's files, behind a trait so tests (and the app, if it ever needs a
/// different source) can stand in.
pub trait ProjectFiles {
    /// The text of the first file under `root` that matches `pattern`, reading at most
    /// `limit` bytes. A file over the limit is refused for `Whole`; for `Head` and `Tail` its
    /// first or last whole lines that fit in the limit are read instead.
    fn read(
        &self,
        root: &Path,
        pattern: &str,
        limit: u64,
        mode: ReadMode,
    ) -> Result<String, RuleError>;
}

/// The real file system.
pub struct FsFiles;

const MAX_DEPTH: usize = 8;
const MAX_VISITED: usize = 50_000;
const SKIPPED_DIRS: [&str; 2] = [".git", "node_modules"];

pub(crate) fn pattern_ok(p: &str) -> bool {
    let p = p.trim();
    !p.is_empty()
        && !p.contains('\0')
        && !p.starts_with(['/', '\\'])
        && !p.contains(':')
        && !p.split(['/', '\\']).any(|part| part == "..")
}

fn has_wildcard(p: &str) -> bool {
    p.contains(['*', '?'])
}

fn walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    pattern: &str,
    visited: &mut usize,
    out: &mut Vec<String>,
) {
    if depth > MAX_DEPTH || *visited >= MAX_VISITED {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        *visited += 1;
        if *visited >= MAX_VISITED {
            return;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let path = entry.path();
        if kind.is_dir() {
            if SKIPPED_DIRS.iter().any(|s| name == *s) {
                continue;
            }
            walk(root, &path, depth + 1, pattern, visited, out);
        } else if kind.is_file() || kind.is_symlink() {
            if let Ok(rel) = path.strip_prefix(root) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                if glob::matches(pattern, &rel, cfg!(windows)) {
                    out.push(rel);
                }
            }
        }
    }
}

impl ProjectFiles for FsFiles {
    fn read(
        &self,
        root: &Path,
        pattern: &str,
        limit: u64,
        mode: ReadMode,
    ) -> Result<String, RuleError> {
        let pattern = pattern.trim();
        if !pattern_ok(pattern) {
            return Err(RuleError::BadQuery(
                "the file must be a path inside the project folder (no .., no drive letter)".into(),
            ));
        }
        let rel = if has_wildcard(pattern) {
            let mut found = Vec::new();
            let mut visited = 0;
            walk(root, root, 0, pattern, &mut visited, &mut found);
            found.sort();
            found.into_iter().next()
        } else {
            let direct = root.join(pattern);
            direct.exists().then(|| pattern.to_string())
        };
        let Some(rel) = rel else {
            return Err(RuleError::FileNotFound(pattern.to_string()));
        };
        read_inside(root, &root.join(&rel), limit, mode)
    }
}

/// Read `file` only if, once links are followed, it is a regular file inside `root`.
fn read_inside(root: &Path, file: &Path, limit: u64, mode: ReadMode) -> Result<String, RuleError> {
    let unreadable = |e: std::io::Error| RuleError::Unreadable(e.to_string());
    let root: PathBuf = root.canonicalize().map_err(unreadable)?;
    let real = file.canonicalize().map_err(unreadable)?;
    if !real.starts_with(&root) {
        return Err(RuleError::Unreadable(
            "the file is a link that leads outside the project folder".into(),
        ));
    }
    let meta = fs::metadata(&real).map_err(unreadable)?;
    if !meta.is_file() {
        return Err(RuleError::Unreadable("not a regular file".into()));
    }
    let too_big = meta.len() > limit;
    if too_big && mode == ReadMode::Whole {
        return Err(RuleError::TooLarge(limit));
    }
    let mut handle = fs::File::open(&real).map_err(unreadable)?;
    let mut bytes = Vec::new();
    if too_big && mode == ReadMode::Tail {
        // The last `limit` bytes, from the first whole line in them.
        handle
            .seek(SeekFrom::Start(meta.len() - limit))
            .map_err(unreadable)?;
        handle
            .take(limit)
            .read_to_end(&mut bytes)
            .map_err(unreadable)?;
        if let Some(newline) = bytes.iter().position(|b| *b == b'\n') {
            bytes.drain(..=newline);
        }
    } else {
        handle
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(unreadable)?;
        if bytes.len() as u64 > limit {
            if mode == ReadMode::Whole {
                return Err(RuleError::TooLarge(limit));
            }
            // The first `limit` bytes, to the end of the last whole line in them.
            bytes.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
            if let Some(newline) = bytes.iter().rposition(|b| *b == b'\n') {
                bytes.truncate(newline + 1);
            }
        }
    }
    if bytes.contains(&0) {
        return Err(RuleError::Unreadable("it looks like a binary file".into()));
    }
    let text = String::from_utf8_lossy(&bytes);
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

/// What one rule gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The value to use for `{{rule.<id>}}`, if there is one (including a fallback).
    pub value: Option<String>,
    /// Why the rule found nothing, if it did not. Present even when a fallback was used.
    pub problem: Option<String>,
    pub used_fallback: bool,
}

/// Run every rule against the project folder. One failing rule never stops the others. Rules
/// that read the same file at the same size limit share one read.
pub fn run_all<'a>(
    rules: impl IntoIterator<Item = (&'a str, &'a Rule)>,
    project: Option<&Path>,
    files: &dyn ProjectFiles,
) -> BTreeMap<String, Outcome> {
    // Rules share a read only when they need the same part of the same file.
    type Key = (String, u64, u8);
    let mut cache: BTreeMap<Key, Result<String, RuleError>> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for (id, rule) in rules {
        let result = match project {
            None => Err(RuleError::NoProject),
            Some(root) => {
                let mode = rule.read_mode();
                let key = (rule.file.trim().to_string(), rule.byte_limit(), mode as u8);
                let text = cache
                    .entry(key)
                    .or_insert_with(|| files.read(root, &rule.file, rule.byte_limit(), mode));
                match text {
                    Ok(t) => extract(rule, t),
                    Err(e) => Err(e.clone()),
                }
            }
        };
        let outcome = match result {
            Ok(value) => Outcome {
                value: Some(value),
                problem: None,
                used_fallback: false,
            },
            Err(e) => Outcome {
                value: rule.fallback.clone(),
                problem: Some(e.to_string()),
                used_fallback: rule.fallback.is_some(),
            },
        };
        out.insert(id.to_string(), outcome);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Format, Multiple};
    use std::cell::Cell;

    fn rule(file: &str, format: Format, query: &str) -> Rule {
        Rule {
            file: file.into(),
            format,
            query: query.into(),
            ..Rule::default()
        }
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        fs::create_dir_all(p.join("config/deep")).unwrap();
        fs::create_dir_all(p.join(".git")).unwrap();
        fs::write(p.join("package.json"), r#"{"version":"3.1.4"}"#).unwrap();
        fs::write(p.join("config/app.toml"), "[build]\nnumber = 12\n").unwrap();
        fs::write(p.join("config/deep/x.toml"), "[build]\nnumber = 99\n").unwrap();
        fs::write(p.join(".git/config.toml"), "[build]\nnumber = 1\n").unwrap();
        fs::write(p.join("bom.json"), "\u{feff}{\"a\":1}").unwrap();
        fs::write(p.join("bin.dat"), [0u8, 1, 2]).unwrap();
        dir
    }

    #[test]
    fn reads_a_named_file_and_a_pattern_taking_the_first_alphabetical_match() {
        let d = project();
        let out = run_all(
            [
                ("version", &rule("package.json", Format::Json, "version")),
                ("number", &rule("**/*.toml", Format::Toml, "build.number")),
            ],
            Some(d.path()),
            &FsFiles,
        );
        assert_eq!(out["version"].value.as_deref(), Some("3.1.4"));
        // `config/app.toml` sorts before `config/deep/x.toml`; `.git` is never entered.
        assert_eq!(out["number"].value.as_deref(), Some("12"));
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        let d = project();
        let out = run_all(
            [("a", &rule("bom.json", Format::Json, "a"))],
            Some(d.path()),
            &FsFiles,
        );
        assert_eq!(out["a"].value.as_deref(), Some("1"));
    }

    #[test]
    fn problems_are_reported_per_rule_and_fallbacks_fill_in() {
        let d = project();
        let mut with_fallback = rule("missing.json", Format::Json, "x");
        with_fallback.fallback = Some("unknown".into());
        let out = run_all(
            [
                ("gone", &rule("missing.json", Format::Json, "x")),
                ("fb", &with_fallback),
                ("bin", &rule("bin.dat", Format::Regex, ".")),
                ("ok", &rule("package.json", Format::Json, "version")),
            ],
            Some(d.path()),
            &FsFiles,
        );
        assert_eq!(out["gone"].value, None);
        assert!(out["gone"]
            .problem
            .as_deref()
            .unwrap()
            .contains("no file matches"));
        assert_eq!(out["fb"].value.as_deref(), Some("unknown"));
        assert!(out["fb"].used_fallback && out["fb"].problem.is_some());
        assert!(out["bin"].problem.as_deref().unwrap().contains("binary"));
        assert_eq!(out["ok"].value.as_deref(), Some("3.1.4"));
    }

    #[test]
    fn without_a_project_folder_every_rule_says_so() {
        let out = run_all(
            [("a", &rule("package.json", Format::Json, "version"))],
            None,
            &FsFiles,
        );
        assert_eq!(out["a"].value, None);
        assert!(out["a"]
            .problem
            .as_deref()
            .unwrap()
            .contains("no project folder"));
    }

    #[test]
    fn paths_that_leave_the_project_are_refused() {
        let d = project();
        for bad in [
            "../secret.json",
            "/etc/passwd",
            "C:\\Windows\\win.ini",
            "a/../../b",
            "",
        ] {
            let out = run_all(
                [("x", &rule(bad, Format::Regex, "."))],
                Some(d.path()),
                &FsFiles,
            );
            assert!(out["x"].problem.is_some(), "{bad:?} must not be read");
            assert_eq!(out["x"].value, None);
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_points_outside_is_not_followed() {
        let d = project();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.json"), r#"{"k":"s3cret"}"#).unwrap();
        std::os::unix::fs::symlink(
            outside.path().join("secret.json"),
            d.path().join("link.json"),
        )
        .unwrap();
        let out = run_all(
            [("k", &rule("link.json", Format::Json, "k"))],
            Some(d.path()),
            &FsFiles,
        );
        assert_eq!(out["k"].value, None);
        assert!(out["k"]
            .problem
            .as_deref()
            .unwrap()
            .contains("outside the project"));
    }

    #[test]
    fn large_files_are_refused_at_the_rules_limit() {
        let d = project();
        fs::write(d.path().join("big.txt"), vec![b'a'; 5000]).unwrap();
        let mut small = rule("big.txt", Format::Regex, "a+");
        small.max_bytes = 1000;
        let out = run_all([("s", &small)], Some(d.path()), &FsFiles);
        assert!(out["s"].problem.as_deref().unwrap().contains("larger than"));
        let ok = run_all(
            [("s", &rule("big.txt", Format::Regex, "a{3}"))],
            Some(d.path()),
            &FsFiles,
        );
        assert_eq!(ok["s"].value.as_deref(), Some("aaa"));
    }

    fn big_log(dir: &Path) {
        // 3000 numbered lines, far over the 1000-byte limit used below.
        let text: String = (1..=3000).map(|i| format!("line {i} INFO ok\n")).collect();
        fs::write(dir.join("app.log"), text).unwrap();
    }

    #[test]
    fn a_big_log_is_read_from_its_end_for_tail_and_level_queries() {
        let d = project();
        big_log(d.path());
        let mut r = rule("app.log", Format::Text, "tail: 2");
        r.max_bytes = 1000;
        r.multiple = Multiple::Join;
        r.separator = " | ".into();
        let out = run_all([("t", &r)], Some(d.path()), &FsFiles);
        assert_eq!(
            out["t"].value.as_deref(),
            Some("line 2999 INFO ok | line 3000 INFO ok")
        );
        let mut last = rule("app.log", Format::Text, "contains: info");
        last.max_bytes = 1000;
        last.multiple = Multiple::Last;
        let out = run_all([("t", &last)], Some(d.path()), &FsFiles);
        assert_eq!(out["t"].value.as_deref(), Some("line 3000 INFO ok"));
    }

    #[test]
    fn the_part_read_starts_on_a_whole_line() {
        let d = project();
        big_log(d.path());
        let mut first = rule("app.log", Format::Text, "contains: line");
        first.max_bytes = 1000;
        let out = run_all([("t", &first)], Some(d.path()), &FsFiles);
        let value = out["t"].value.clone().unwrap_or_default();
        assert!(
            value.starts_with("line ") && value.ends_with("INFO ok"),
            "the first line read is whole, not cut in the middle: {value:?}"
        );
    }

    #[test]
    fn a_big_log_is_read_from_its_start_for_head_and_line_numbers() {
        let d = project();
        big_log(d.path());
        let mut r = rule("app.log", Format::Text, "line: 2");
        r.max_bytes = 1000;
        let out = run_all([("h", &r)], Some(d.path()), &FsFiles);
        assert_eq!(out["h"].value.as_deref(), Some("line 2 INFO ok"));
        let mut head = rule("app.log", Format::Text, "head: 3");
        head.max_bytes = 1000;
        head.multiple = Multiple::Last;
        let out = run_all([("h", &head)], Some(d.path()), &FsFiles);
        assert_eq!(out["h"].value.as_deref(), Some("line 3 INFO ok"));
    }

    #[test]
    fn data_formats_still_refuse_a_file_over_the_limit() {
        let d = project();
        big_log(d.path());
        let mut r = rule("app.log", Format::Regex, "line 1 ");
        r.max_bytes = 1000;
        let out = run_all([("r", &r)], Some(d.path()), &FsFiles);
        assert!(out["r"].problem.as_deref().unwrap().contains("larger than"));
    }

    #[test]
    fn txt_and_log_rules_read_a_small_file_whole() {
        let d = project();
        fs::write(d.path().join("notes.txt"), "Version: 2.5\nOwner: Ann\n").unwrap();
        let out = run_all(
            [
                ("v", &rule("notes.txt", Format::Text, "after: version:")),
                ("o", &rule("*.txt", Format::Text, "after: Owner")),
            ],
            Some(d.path()),
            &FsFiles,
        );
        assert_eq!(out["v"].value.as_deref(), Some("2.5"));
        assert_eq!(out["o"].value.as_deref(), Some("Ann"));
    }

    struct Counting(Cell<usize>);
    impl ProjectFiles for Counting {
        fn read(
            &self,
            _root: &Path,
            _pattern: &str,
            _limit: u64,
            _mode: ReadMode,
        ) -> Result<String, RuleError> {
            self.0.set(self.0.get() + 1);
            Ok(r#"{"a":1,"b":2}"#.into())
        }
    }

    #[test]
    fn rules_on_the_same_file_share_one_read() {
        let files = Counting(Cell::new(0));
        let mut b = rule("same.json", Format::Json, "b");
        b.multiple = Multiple::First;
        let out = run_all(
            [("a", &rule("same.json", Format::Json, "a")), ("b", &b)],
            Some(Path::new(".")),
            &files,
        );
        assert_eq!(files.0.get(), 1);
        assert_eq!(out["a"].value.as_deref(), Some("1"));
        assert_eq!(out["b"].value.as_deref(), Some("2"));
    }
}
