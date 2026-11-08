//! Find a rule's file safely and run every rule.

use crate::details::{self, FileDetails, What};
use crate::extract::{extract, RuleError};
use crate::find;
use crate::model::{Format, ReadMode, Rule, Several, Which};
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

    /// The path (relative to `root`) of the file `pattern` means. A source that cannot look
    /// takes the pattern for the file.
    fn locate(&self, _root: &Path, pattern: &str) -> Result<String, RuleError> {
        Ok(pattern.trim().to_string())
    }

    /// Every file `pattern` means, relative to `root`, the newest first.
    fn locate_all(&self, root: &Path, pattern: &str) -> Result<Vec<String>, RuleError> {
        self.locate(root, pattern).map(|f| vec![f])
    }

    /// Facts about the file `pattern` means. The file's content is only read when `what` needs it.
    fn details(&self, _root: &Path, _pattern: &str, _what: What) -> Result<FileDetails, RuleError> {
        Err(RuleError::Unreadable(
            "this source cannot report file details".into(),
        ))
    }
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

/// Every file under `dir` whose name (not its folder) passes `keep`.
fn walk_names(
    root: &Path,
    dir: &Path,
    depth: usize,
    keep: &dyn Fn(&str) -> bool,
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
            walk_names(root, &path, depth + 1, keep, visited, out);
        } else if (kind.is_file() || kind.is_symlink()) && keep(&name.to_string_lossy()) {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
}

/// When a file was last changed, for "the newest".
fn modified(root: &Path, rel: &str) -> std::time::SystemTime {
    fs::metadata(root.join(rel))
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

/// Every file a rule's `file` means, relative to `root`, the newest first:
///
/// * a path with `*`, `**` or `?` is a pattern (`logs/*.log`); a pattern without a folder
///   (`*.log`) is a name looked for in every folder;
/// * a path with a folder (`config/app.toml`), or a name of a file at the top, is that file;
/// * any other text is a file name, or part of one, looked for in every folder (`system_info`
///   finds `logs/system_info.txt`); names that are exactly it come before names that hold it.
fn find_all(root: &Path, pattern: &str) -> Result<Vec<String>, RuleError> {
    let pattern = pattern.trim();
    if !pattern_ok(pattern) {
        return Err(RuleError::BadQuery(
            "the file must be a name or a path inside the project folder (no .., no drive letter)"
                .into(),
        ));
    }
    let mut found = Vec::new();
    let mut visited = 0;
    if has_wildcard(pattern) && !pattern.contains(['/', '\\']) {
        // `*.log`: a name, in every folder
        let insensitive = cfg!(windows);
        walk_names(
            root,
            root,
            0,
            &|n| glob::matches(pattern, n, insensitive),
            &mut visited,
            &mut found,
        );
    } else if has_wildcard(pattern) {
        walk(root, root, 0, pattern, &mut visited, &mut found);
    } else if pattern.contains(['/', '\\']) || root.join(pattern).is_file() {
        if root.join(pattern).exists() {
            found.push(pattern.replace('\\', "/"));
        }
    } else {
        let wanted = pattern.to_lowercase();
        walk_names(
            root,
            root,
            0,
            &|n| n.to_lowercase() == wanted,
            &mut visited,
            &mut found,
        );
        if found.is_empty() {
            visited = 0;
            walk_names(
                root,
                root,
                0,
                &|n| n.to_lowercase().contains(&wanted),
                &mut visited,
                &mut found,
            );
        }
    }
    if found.is_empty() {
        return Err(RuleError::FileNotFound(pattern.to_string()));
    }
    // the newest first; the same age in alphabetical order
    found.sort();
    found.sort_by_key(|rel| std::cmp::Reverse(modified(root, rel)));
    Ok(found)
}

/// The file a pattern means, relative to `root`: the first match in alphabetical order for a
/// pattern (as before 0.3.0), the newest for a name.
fn find(root: &Path, pattern: &str) -> Result<String, RuleError> {
    let mut all = find_all(root, pattern)?;
    if has_wildcard(pattern.trim()) {
        all.sort();
    }
    Ok(all.swap_remove(0))
}

impl ProjectFiles for FsFiles {
    fn read(
        &self,
        root: &Path,
        pattern: &str,
        limit: u64,
        mode: ReadMode,
    ) -> Result<String, RuleError> {
        let rel = find(root, pattern)?;
        read_inside(root, &root.join(&rel), limit, mode)
    }

    fn locate(&self, root: &Path, pattern: &str) -> Result<String, RuleError> {
        find(root, pattern)
    }

    fn locate_all(&self, root: &Path, pattern: &str) -> Result<Vec<String>, RuleError> {
        find_all(root, pattern)
    }

    fn details(&self, root: &Path, pattern: &str, what: What) -> Result<FileDetails, RuleError> {
        let rel = find(root, pattern)?;
        let (real, meta) = resolve_inside(root, &root.join(&rel))?;
        let mut found = FileDetails {
            name: Path::new(&rel)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            size: meta.len(),
            modified: meta.modified().ok(),
            ..FileDetails::default()
        };
        if what.needs_content() {
            let mut file =
                fs::File::open(&real).map_err(|e| RuleError::Unreadable(e.to_string()))?;
            let (lines, sha256) = details::read_content(&mut file, meta.len(), what)?;
            found.lines = lines;
            found.sha256 = sha256;
        }
        Ok(found)
    }
}

/// `file`, once links are followed, if it is a regular file inside `root`; and what the file
/// system says about it.
fn resolve_inside(root: &Path, file: &Path) -> Result<(PathBuf, fs::Metadata), RuleError> {
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
    Ok((real, meta))
}

/// Read `file` only if, once links are followed, it is a regular file inside `root`.
fn read_inside(root: &Path, file: &Path, limit: u64, mode: ReadMode) -> Result<String, RuleError> {
    let unreadable = |e: std::io::Error| RuleError::Unreadable(e.to_string());
    let (real, meta) = resolve_inside(root, file)?;
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

/// What running the rules gave.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// By value key (`{{rule.<key>}}`).
    pub values: BTreeMap<String, Outcome>,
    /// The rule each value belongs to.
    pub owners: BTreeMap<String, String>,
    /// The files to tick in the window's Files blocks, relative to the project folder.
    pub files: Vec<String>,
    /// The files each rule found, by rule id.
    pub found: BTreeMap<String, Vec<String>>,
}

fn outcome(result: Result<String, RuleError>, fallback: &Option<String>) -> Outcome {
    match result {
        Ok(value) => Outcome {
            value: Some(value),
            problem: None,
            used_fallback: false,
        },
        Err(e) => Outcome {
            value: fallback.clone(),
            problem: Some(e.to_string()),
            used_fallback: fallback.is_some(),
        },
    }
}

/// Run every rule against the project folder: find its file (or files), read each of its values,
/// and note the files to add. One failing rule or value never stops the others.
pub fn run_rules<'a>(
    rules: impl IntoIterator<Item = (&'a str, &'a Rule)>,
    project: Option<&Path>,
    files: &dyn ProjectFiles,
) -> Report {
    let mut report = Report::default();
    let mut legacy: Vec<(&str, &Rule)> = Vec::new();
    // Rules that read the same part of the same file share one read.
    let mut cache: BTreeMap<(String, u64, u8), Result<String, RuleError>> = BTreeMap::new();
    for (id, rule) in rules {
        let steps = rule.steps(id);
        // the details of a file itself (size, date…) keep the reader they had before 0.3.0
        if rule.values.is_empty() && rule.format == Format::File {
            legacy.push((id, rule));
            continue;
        }
        let located = match project {
            None => Err(RuleError::NoProject),
            // a rule from before 0.3.0 reads the file it always read (a pattern's first match
            // in alphabetical order)
            Some(root) if rule.values.is_empty() && !rule.add_to_files => {
                files.locate(root, &rule.file).map(|f| vec![f])
            }
            Some(root) => files.locate_all(root, &rule.file),
        };
        let chosen: Vec<String> = match &located {
            Ok(all) => match rule.several {
                Several::Newest => all.iter().take(1).cloned().collect(),
                Several::All => all.iter().take(MAX_FILES).cloned().collect(),
            },
            Err(_) => Vec::new(),
        };
        report.found.insert(id.to_string(), chosen.clone());
        if rule.add_to_files {
            for f in &chosen {
                if !report.files.contains(f) {
                    report.files.push(f.clone());
                }
            }
        }
        // each file is read once, from the end when only the last value is wanted
        let wants_tail = !steps.is_empty() && steps.iter().all(|s| s.which == Which::Last);
        let mode = if wants_tail {
            ReadMode::Tail
        } else {
            ReadMode::Head
        };
        let texts: Vec<(String, Result<String, RuleError>)> = match project {
            Some(root) if !steps.is_empty() => chosen
                .iter()
                .map(|f| {
                    let mode = if rule.values.is_empty() {
                        rule.read_mode()
                    } else {
                        mode
                    };
                    let key = (f.clone(), rule.byte_limit(), mode as u8);
                    let text = cache
                        .entry(key)
                        .or_insert_with(|| files.read(root, f, rule.byte_limit(), mode))
                        .clone();
                    (f.clone(), text)
                })
                .collect(),
            _ => Vec::new(),
        };
        for step in steps {
            let result = match (&located, texts.is_empty()) {
                (Err(e), _) => Err(e.clone()),
                (Ok(_), true) => Err(RuleError::NoProject),
                (Ok(_), false) => {
                    let mut all = Vec::new();
                    let mut problem = None;
                    for (path, text) in &texts {
                        match text {
                            Ok(t) => match find::all(&step.find, rule, path, t, step.which) {
                                Ok(mut v) => all.append(&mut v),
                                Err(e) => problem = Some(e),
                            },
                            Err(e) => problem = Some(e.clone()),
                        }
                    }
                    match (all.is_empty(), problem) {
                        (true, Some(e)) => Err(e),
                        _ => find::pick(all, step.which, &step.find),
                    }
                }
            };
            report.owners.insert(step.key.clone(), id.to_string());
            report
                .values
                .insert(step.key.clone(), outcome(result, &step.fallback));
        }
    }
    for (id, outcome) in run_details(legacy, project, files) {
        report.owners.insert(id.clone(), id.clone());
        report.values.insert(id, outcome);
    }
    report
}

/// The most files one rule reads or adds.
const MAX_FILES: usize = 20;

/// Run every rule and give each value. See [`run_rules`] for the files to add.
pub fn run_all<'a>(
    rules: impl IntoIterator<Item = (&'a str, &'a Rule)>,
    project: Option<&Path>,
    files: &dyn ProjectFiles,
) -> BTreeMap<String, Outcome> {
    run_rules(rules, project, files).values
}

/// Rules from before 0.3.0 that read a detail of a file itself (its size, its date…).
fn run_details<'a>(
    rules: impl IntoIterator<Item = (&'a str, &'a Rule)>,
    project: Option<&Path>,
    files: &dyn ProjectFiles,
) -> BTreeMap<String, Outcome> {
    // Rules share a read only when they need the same part of the same file.
    type Key = (String, u64, u8);
    let mut cache: BTreeMap<Key, Result<String, RuleError>> = BTreeMap::new();
    // Rules that ask for details of the same file share one look at it (and one fingerprint).
    let mut seen: BTreeMap<(String, u8), Result<FileDetails, RuleError>> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for (id, rule) in rules {
        let result = match project {
            None => Err(RuleError::NoProject),
            Some(root) if rule.format == Format::File => match details::parse(&rule.query) {
                Err(e) => Err(RuleError::BadQuery(e)),
                Ok(what) => {
                    let kind = match what {
                        What::Lines => 1,
                        What::Sha256(_) => 2,
                        _ => 0,
                    };
                    let key = (rule.file.trim().to_string(), kind);
                    match seen
                        .entry(key)
                        .or_insert_with(|| files.details(root, &rule.file, what))
                    {
                        Ok(d) => details::value(d, what),
                        Err(e) => Err(e.clone()),
                    }
                }
            },
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

/// Which file a rule reads and the start of it, so the settings window can show the person what
/// the rule is looking at while they write it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Peek {
    /// The file the rule's pattern means, relative to the project folder.
    pub file: Option<String>,
    /// The first lines of it, if it is text.
    pub excerpt: Option<String>,
}

const PEEK_BYTES: u64 = 4096;
const PEEK_LINES: usize = 12;
const PEEK_LINE_CHARS: usize = 200;

pub fn peek(rule: &Rule, root: &Path, files: &dyn ProjectFiles) -> Peek {
    // the file the rule reads: the newest of its matches since 0.3.0
    let located = if rule.values.is_empty() && !rule.add_to_files {
        files.locate(root, &rule.file)
    } else {
        files
            .locate_all(root, &rule.file)
            .and_then(|all| all.into_iter().next().ok_or(RuleError::NoValue))
    };
    let Ok(file) = located else {
        return Peek::default();
    };
    let excerpt = files
        .read(root, &file, PEEK_BYTES, ReadMode::Head)
        .ok()
        .map(|text| {
            text.lines()
                .take(PEEK_LINES)
                .map(|l| l.chars().take(PEEK_LINE_CHARS).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|t| !t.trim().is_empty());
    Peek {
        file: Some(file),
        excerpt,
    }
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

    fn details_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        fs::create_dir_all(p.join("bin")).unwrap();
        fs::write(p.join("app.log"), "one\ntwo\nthree\n").unwrap();
        fs::write(p.join("bin/tool.exe"), [0u8, 159, 146, 150, 10, 0]).unwrap();
        fs::write(p.join("empty.txt"), "").unwrap();
        dir
    }

    fn ask(d: &tempfile::TempDir, file: &str, query: &str) -> Outcome {
        let r = rule(file, Format::File, query);
        run_all([("r", &r)], Some(d.path()), &FsFiles)
            .remove("r")
            .unwrap()
    }

    #[test]
    fn file_details_read_the_file_itself() {
        let d = details_project();
        assert_eq!(ask(&d, "app.log", "name").value.as_deref(), Some("app.log"));
        assert_eq!(ask(&d, "app.log", "size").value.as_deref(), Some("14"));
        assert_eq!(
            ask(&d, "app.log", "size: human").value.as_deref(),
            Some("14 bytes")
        );
        assert_eq!(ask(&d, "app.log", "lines").value.as_deref(), Some("3"));
        assert_eq!(ask(&d, "empty.txt", "lines").value.as_deref(), Some("0"));
        let sha = ask(&d, "app.log", "sha256").value.unwrap();
        assert_eq!(sha.len(), 64);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(
            ask(&d, "app.log", "sha256: short").value.unwrap(),
            sha[..12]
        );
        let modified = ask(&d, "app.log", "modified: utc").value.unwrap();
        assert!(
            modified.ends_with(" UTC") && modified.starts_with("20"),
            "{modified}"
        );
    }

    #[test]
    fn file_details_work_on_any_kind_of_file_and_find_it_by_pattern() {
        let d = details_project();
        // A program file: no lines, but a size and a fingerprint.
        assert_eq!(ask(&d, "bin/*.exe", "size").value.as_deref(), Some("6"));
        assert!(ask(&d, "**/tool.exe", "sha256: short").value.is_some());
        let lines = ask(&d, "bin/tool.exe", "lines");
        assert_eq!(lines.value, None);
        assert!(lines.problem.unwrap().contains("binary"));
    }

    #[test]
    fn file_details_say_what_went_wrong_and_use_the_fallback() {
        let d = details_project();
        assert!(ask(&d, "nope.log", "size")
            .problem
            .unwrap()
            .contains("no file matches"));
        assert!(ask(&d, "app.log", "colour")
            .problem
            .unwrap()
            .contains("name, size"));
        let r = Rule {
            fallback: Some("unknown".into()),
            ..rule("nope.log", Format::File, "sha256")
        };
        let out = run_all([("r", &r)], Some(d.path()), &FsFiles)
            .remove("r")
            .unwrap();
        assert_eq!(out.value.as_deref(), Some("unknown"));
        assert!(out.used_fallback);
        let none = run_all([("r", &r)], None, &FsFiles).remove("r").unwrap();
        assert!(none.problem.unwrap().contains("no project folder"));
    }

    #[test]
    fn file_details_cannot_leave_the_project_folder() {
        let d = details_project();
        assert!(ask(&d, "../secret", "size")
            .problem
            .unwrap()
            .contains("inside the project folder"));
        assert!(ask(&d, "/etc/passwd", "size").problem.is_some());
        #[cfg(unix)]
        {
            let outside = tempfile::tempdir().unwrap();
            fs::write(outside.path().join("o.txt"), "outside").unwrap();
            std::os::unix::fs::symlink(outside.path().join("o.txt"), d.path().join("link.txt"))
                .unwrap();
            assert!(ask(&d, "link.txt", "size")
                .problem
                .unwrap()
                .contains("outside"));
        }
    }

    #[test]
    fn rules_asking_the_same_file_the_same_thing_share_one_look() {
        struct Counting(Cell<u32>);
        impl ProjectFiles for Counting {
            fn read(&self, _: &Path, _: &str, _: u64, _: ReadMode) -> Result<String, RuleError> {
                Err(RuleError::NoValue)
            }
            fn details(&self, _: &Path, _: &str, _: What) -> Result<FileDetails, RuleError> {
                self.0.set(self.0.get() + 1);
                Ok(FileDetails {
                    size: 5,
                    sha256: Some("ab".repeat(32)),
                    ..FileDetails::default()
                })
            }
        }
        let files = Counting(Cell::new(0));
        let (a, b, c) = (
            rule("f", Format::File, "size"),
            rule("f", Format::File, "size: human"),
            rule("f", Format::File, "sha256: short"),
        );
        let out = run_all(
            [("a", &a), ("b", &b), ("c", &c)],
            Some(Path::new("/p")),
            &files,
        );
        assert_eq!(out["a"].value.as_deref(), Some("5"));
        assert_eq!(out["b"].value.as_deref(), Some("5 bytes"));
        assert_eq!(out["c"].value.as_deref(), Some("abababababab"));
        // One look for the two that need no content, one for the fingerprint.
        assert_eq!(files.0.get(), 2);
    }

    #[test]
    fn a_source_without_file_details_says_so() {
        struct Plain;
        impl ProjectFiles for Plain {
            fn read(&self, _: &Path, _: &str, _: u64, _: ReadMode) -> Result<String, RuleError> {
                Ok(String::new())
            }
        }
        let r = rule("f", Format::File, "size");
        let out = run_all([("r", &r)], Some(Path::new("/p")), &Plain);
        assert!(out["r"]
            .problem
            .as_deref()
            .unwrap()
            .contains("cannot report file details"));
    }

    #[test]
    fn a_peek_names_the_file_and_shows_its_first_lines() {
        let d = tempfile::tempdir().unwrap();
        let body: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        fs::write(d.path().join("run.log"), &body).unwrap();
        fs::write(d.path().join("blob.bin"), [0u8, 1, 2]).unwrap();
        let p = peek(&rule("*.log", Format::Text, "tail: 1"), d.path(), &FsFiles);
        assert_eq!(p.file.as_deref(), Some("run.log"));
        let excerpt = p.excerpt.unwrap();
        assert_eq!(excerpt.lines().count(), 12);
        assert!(excerpt.starts_with("line 1\nline 2"));
        // A file that is not text still names itself, with nothing to show.
        let bin = peek(&rule("blob.bin", Format::File, "size"), d.path(), &FsFiles);
        assert_eq!(bin.file.as_deref(), Some("blob.bin"));
        assert_eq!(bin.excerpt, None);
        // No such file: nothing.
        assert_eq!(
            peek(&rule("nope", Format::Text, "tail: 1"), d.path(), &FsFiles),
            Peek::default()
        );
    }

    // ----- 0.3.0: a file found by its name, values read from it, files added -----

    use crate::model::{Find, ValueStep};

    fn step(key: &str, find: Find, which: Which) -> ValueStep {
        ValueStep {
            key: key.into(),
            name: key.into(),
            find,
            which,
            fallback: None,
        }
    }

    fn machine() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        fs::create_dir_all(p.join("logs/old")).unwrap();
        fs::write(
            p.join("logs/old/system_info.txt"),
            "GPU: GTX 970\nOS Name: Windows 10\n",
        )
        .unwrap();
        // the newer one, written a moment later
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(
            p.join("logs/system_info.txt"),
            "Host: DEV-07\nGPU: RTX 4070\nGPU: UHD 770\nOS Name: Windows 11\n",
        )
        .unwrap();
        fs::write(p.join("package.json"), r#"{"version":"3.1.4"}"#).unwrap();
        dir
    }

    fn config(several: Several, add: bool) -> Rule {
        Rule {
            name: "Machine".into(),
            file: "system_info".into(),
            several,
            add_to_files: add,
            values: vec![
                step("gpu", Find::ValueOf { name: "GPU".into() }, Which::All),
                step(
                    "os",
                    Find::Between {
                        start: "OS Name:".into(),
                        end: None,
                    },
                    Which::First,
                ),
            ],
            ..Rule::default()
        }
    }

    #[test]
    fn a_file_named_by_part_of_its_name_is_found_anywhere_and_the_newest_is_read() {
        let d = machine();
        let r = config(Several::Newest, true);
        let report = run_rules([("machine", &r)], Some(d.path()), &FsFiles);
        assert_eq!(
            report.values["gpu"].value.as_deref(),
            Some("RTX 4070, UHD 770")
        );
        assert_eq!(report.values["os"].value.as_deref(), Some("Windows 11"));
        assert_eq!(report.owners["gpu"], "machine");
        assert_eq!(report.files, vec!["logs/system_info.txt".to_string()]);
        assert_eq!(
            report.found["machine"],
            vec!["logs/system_info.txt".to_string()]
        );
    }

    #[test]
    fn all_the_files_can_be_read_and_added() {
        let d = machine();
        let r = config(Several::All, true);
        let report = run_rules([("machine", &r)], Some(d.path()), &FsFiles);
        assert_eq!(
            report.values["gpu"].value.as_deref(),
            Some("RTX 4070, UHD 770, GTX 970")
        );
        assert_eq!(report.files.len(), 2);
        // without the tick, nothing is added
        let quiet = config(Several::All, false);
        assert!(run_rules([("machine", &quiet)], Some(d.path()), &FsFiles)
            .files
            .is_empty());
    }

    #[test]
    fn a_rule_that_only_adds_files_reads_nothing() {
        let d = machine();
        let r = Rule {
            name: "Logs".into(),
            file: "*.txt".into(),
            several: Several::All,
            add_to_files: true,
            ..Rule::default()
        };
        crate::check(&r).unwrap();
        let report = run_rules([("logs", &r)], Some(d.path()), &FsFiles);
        assert!(report.values.is_empty());
        assert_eq!(report.files.len(), 2);
    }

    #[test]
    fn a_missing_file_or_value_gives_the_fallback_and_says_why() {
        let d = machine();
        let mut r = config(Several::Newest, false);
        r.values[0].find = Find::ValueOf { name: "CPU".into() };
        r.values[0].fallback = Some("unknown".into());
        let report = run_rules([("machine", &r)], Some(d.path()), &FsFiles);
        assert_eq!(report.values["gpu"].value.as_deref(), Some("unknown"));
        assert!(report.values["gpu"].used_fallback);
        let mut gone = config(Several::Newest, true);
        gone.file = "nowhere".into();
        let report = run_rules([("machine", &gone)], Some(d.path()), &FsFiles);
        assert!(report.values["os"]
            .problem
            .as_deref()
            .unwrap()
            .contains("nowhere"));
        assert!(report.files.is_empty());
    }

    #[test]
    fn a_rule_from_before_keeps_its_name_and_a_file_at_the_top_is_that_file() {
        let d = machine();
        // `package.json` at the top is that file, as before
        let old = rule("package.json", Format::Json, "version");
        let report = run_rules([("version", &old)], Some(d.path()), &FsFiles);
        assert_eq!(report.values["version"].value.as_deref(), Some("3.1.4"));
        assert_eq!(report.owners["version"], "version");
    }

    #[test]
    fn two_values_with_one_name_are_refused() {
        let mut r = config(Several::Newest, false);
        r.values[1].key = "gpu".into();
        assert!(crate::check(&r).unwrap_err().contains("{{rule.gpu}}"));
        let mut bad = config(Several::Newest, false);
        bad.values[0].key = "g p u".into();
        assert!(crate::check(&bad).is_err());
        let empty = Rule {
            name: "x".into(),
            file: "a.txt".into(),
            ..Rule::default()
        };
        assert!(crate::check(&empty).unwrap_err().contains("Add a value"));
    }
}
