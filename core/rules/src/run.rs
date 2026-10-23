//! Find a rule's file safely and run every rule.

use crate::extract::{extract, RuleError};
use crate::model::Rule;
use cargoframe_context::glob;
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Reading the project's files, behind a trait so tests (and the app, if it ever needs a
/// different source) can stand in.
pub trait ProjectFiles {
    /// The text of the first file under `root` that matches `pattern`, reading at most
    /// `limit` bytes.
    fn read(&self, root: &Path, pattern: &str, limit: u64) -> Result<String, RuleError>;
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
    fn read(&self, root: &Path, pattern: &str, limit: u64) -> Result<String, RuleError> {
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
        read_inside(root, &root.join(&rel), limit)
    }
}

/// Read `file` only if, once links are followed, it is a regular file inside `root`.
fn read_inside(root: &Path, file: &Path, limit: u64) -> Result<String, RuleError> {
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
    if meta.len() > limit {
        return Err(RuleError::TooLarge(limit));
    }
    let mut bytes = Vec::new();
    fs::File::open(&real)
        .map_err(unreadable)?
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(unreadable)?;
    if bytes.len() as u64 > limit {
        return Err(RuleError::TooLarge(limit));
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
    let mut cache: BTreeMap<(String, u64), Result<String, RuleError>> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for (id, rule) in rules {
        let result = match project {
            None => Err(RuleError::NoProject),
            Some(root) => {
                let key = (rule.file.trim().to_string(), rule.byte_limit());
                let text = cache
                    .entry(key)
                    .or_insert_with(|| files.read(root, &rule.file, rule.byte_limit()));
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

    struct Counting(Cell<usize>);
    impl ProjectFiles for Counting {
        fn read(&self, _root: &Path, _pattern: &str, _limit: u64) -> Result<String, RuleError> {
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
