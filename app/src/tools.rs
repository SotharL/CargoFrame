//! Small helpers behind Settings buttons: "Test" for a data rule, "Test folder" for safe
//! settings, and the list of variables the builder offers.

use cargoframe_config::{Kind, Store};
use cargoframe_context::climb;
use cargoframe_i18n::{t, tf};
use cargoframe_layout::{reach, BlockKind, FilesBlock, Layout, Reach, Registry};
use cargoframe_rules::{FsFiles, Rule};
use cargoframe_window::files::{self, PlaceFound};
use cargoframe_window::{places, vars};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// For every sharing button of `layout`, what it does with the other blocks.
pub fn button_reach(layout: &Layout, registry: &dyn Registry) -> BTreeMap<String, Reach> {
    layout
        .blocks
        .iter()
        .filter(|b| matches!(b.kind, BlockKind::SharingButton(_)))
        .filter_map(|b| reach(layout, &b.id, registry).map(|r| (b.id.clone(), r)))
        .collect()
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RuleTest {
    /// The folder the rule was run against.
    pub folder: Option<String>,
    /// The first value, as before 0.3.0 (the Settings page reads `values`).
    pub value: Option<String>,
    /// Why the rule as a whole cannot run (not finished, no such file).
    pub problem: Option<String>,
    /// The file the rule reads in that folder (the newest when several match), and the start of
    /// it, so the person sees what the rule is looking at.
    pub file: Option<String>,
    pub excerpt: Option<String>,
    /// How the file is read: "JSON", "text"…
    pub kind: Option<String>,
    /// Every file the name matches, the newest first.
    pub files: Vec<String>,
    /// What each value gave.
    pub values: Vec<ValueTest>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ValueTest {
    pub key: String,
    pub value: Option<String>,
    pub problem: Option<String>,
    pub used_fallback: bool,
}

/// Run one rule against `folder` (or say there is none). Never writes anything. A rule that is
/// not finished yet (no name, a value being typed) still shows the file it points at, and every
/// value that can already be read.
///
/// `others` are the saved rules and `known` the values of the last capture: a value made from
/// other values is previewed with them.
pub fn test_rule(
    rule: &Rule,
    folder: Option<&str>,
    others: &[(String, Rule)],
    known: &std::collections::BTreeMap<String, String>,
) -> RuleTest {
    let root = folder.map(Path::new).filter(|f| f.is_dir());
    let seen = root
        .map(|f| cargoframe_rules::peek(rule, f, &FsFiles))
        .unwrap_or_default();
    let mut named = rule.clone();
    if named.name.trim().is_empty() {
        named.name = "preview".into();
    }
    // the other rules first, so the values this one is made from are there
    let all: Vec<(&str, &Rule)> = others
        .iter()
        .map(|(id, r)| (id.as_str(), r))
        .chain([("test", &named)])
        .collect();
    let mut report = if rule.values.iter().any(|v| v.find.is_combine()) {
        cargoframe_rules::run_rules(all.iter().copied(), root, &FsFiles)
    } else {
        cargoframe_rules::run_rules([("test", &named)], root, &FsFiles)
    };
    let mut values_known = known.clone();
    for (key, o) in &report.values {
        if let Some(v) = &o.value {
            values_known.insert(format!("rule.{key}"), v.clone());
        }
    }
    cargoframe_rules::combine_values(all.iter().copied(), &mut report, &mut values_known);
    let reads_no_file = named.only_combines() && named.file.trim().is_empty();
    let values: Vec<ValueTest> = named
        .steps("test")
        .iter()
        .filter_map(|s| {
            report.values.get(&s.key).map(|o| ValueTest {
                key: s.key.clone(),
                value: o.value.clone(),
                problem: o.problem.clone(),
                used_fallback: o.used_fallback,
            })
        })
        .collect();
    let first = values.first();
    RuleTest {
        folder: folder.map(str::to_string),
        value: first.and_then(|v| v.value.clone()),
        problem: cargoframe_rules::check(&named)
            .err()
            .or_else(|| match (&seen.file, root) {
                _ if reads_no_file => None,
                (None, Some(_)) => Some(
                    cargoframe_rules::RuleError::FileNotFound(rule.file.trim().to_string())
                        .to_string(),
                ),
                (_, None) => Some(cargoframe_rules::RuleError::NoProject.to_string()),
                _ => None,
            }),
        kind: seen
            .file
            .as_deref()
            .map(|f| cargoframe_rules::find::kind_name(f).to_string()),
        file: seen.file,
        excerpt: seen.excerpt,
        files: report.found.get("test").cloned().unwrap_or_default(),
        values,
    }
}

/// What a Files block would find, for the layout builder's live count.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct FilesPreview {
    /// The program file the folders were worked out from.
    pub program: Option<String>,
    pub places: Vec<PlaceFound>,
    /// Choices that could not be turned into a folder, with the reason.
    pub missing: Vec<String>,
    pub total: usize,
    pub ticked: usize,
    pub truncated: bool,
    pub problem: Option<String>,
}

/// Run a Files block's settings against `program` (the program file of a capture) without
/// capturing anything. Reads folder listings only.
pub fn preview_files(block: &FilesBlock, program: Option<&str>) -> FilesPreview {
    let Some(program) = program.map(str::trim).filter(|p| !p.is_empty()) else {
        return FilesPreview {
            program: None,
            places: Vec::new(),
            missing: Vec::new(),
            total: 0,
            ticked: 0,
            truncated: false,
            problem: Some(
                t("Make a capture first, or type the path of a program file, to see what is found")
                    .into(),
            ),
        };
    };
    let project = climb::resolve_project_folder(program).map(|r| r.folder);
    let found = places::places_for(&block.look_in, project.as_deref(), Some(program));
    let missing: Vec<String> = found
        .missing
        .iter()
        .map(|(which, why)| format!("{}: {why}", places::label(*which)))
        .collect();
    let Some(root) = found
        .places
        .iter()
        .map(|p| PathBuf::from(&p.base))
        .min_by_key(|p| p.components().count())
    else {
        return FilesPreview {
            program: Some(program.to_string()),
            places: Vec::new(),
            missing,
            total: 0,
            ticked: 0,
            truncated: false,
            problem: Some(t("There is no folder to look in").into()),
        };
    };
    let look: Vec<files::Where> = found
        .places
        .iter()
        .map(|p| files::Where {
            label: places::label(p.which).to_string(),
            dir: PathBuf::from(&p.dir),
        })
        .collect();
    let scan = files::scan_places(
        &root,
        &look,
        &block.include,
        &block.preselect,
        files::Limits::default(),
    );
    FilesPreview {
        program: Some(program.to_string()),
        places: scan.places,
        missing,
        total: scan.entries.len(),
        ticked: scan.entries.iter().filter(|e| e.selected).count(),
        truncated: scan.truncated,
        problem: scan.problem,
    }
}

/// Can CargoFrame write zips here? Creates and removes a tiny file; never creates the folder.
pub fn test_folder(path: &str) -> Result<(), String> {
    let dir = Path::new(path.trim());
    if path.trim().is_empty() {
        return Err(t("Choose a folder").into());
    }
    if !dir.is_dir() {
        return Err(t("That folder does not exist").into());
    }
    let probe = dir.join(format!(".cargoframe-write-test-{}", std::process::id()));
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err(t("CargoFrame is not allowed to write there").into())
        }
        Err(e) => Err(tf("Cannot write there: {e}", &[("e", &e)])),
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct VariableInfo {
    pub name: String,
    pub description: String,
}

/// Built-in variables, then one per data rule.
pub fn variables(store: &Store) -> Vec<VariableInfo> {
    let mut out: Vec<VariableInfo> = vars::NAMES
        .iter()
        .map(|n| VariableInfo {
            name: (*n).to_string(),
            description: vars::describe_name(n).to_string(),
        })
        .collect();
    if let Ok(loaded) = store.load_all::<Rule>(Kind::Rules) {
        // a value read in several files is offered once, naming its files
        let mut values: Vec<(String, String, Vec<String>)> = Vec::new();
        for (id, rule) in loaded.items {
            for step in rule.steps(&id) {
                let file = rule.file.trim().to_string();
                match values.iter_mut().find(|(k, _, _)| *k == step.key) {
                    Some((_, _, files)) => files.push(file),
                    None => {
                        values.push((step.key.clone(), step.name.trim().to_string(), vec![file]))
                    }
                }
            }
        }
        for (key, name, files) in values {
            let files: Vec<String> = files.into_iter().filter(|f| !f.is_empty()).collect();
            out.push(VariableInfo {
                name: format!("rule.{key}"),
                description: match (name.is_empty(), files.is_empty()) {
                    (_, true) if name.is_empty() => t("Made from other values").to_string(),
                    (_, true) => tf("{name}, made from other values", &[("name", &name)]),
                    (true, false) => tf("From {files}", &[("files", &files.join(", "))]),
                    (false, false) => tf(
                        "{name}, from {files}",
                        &[("name", &name), ("files", &files.join(", "))],
                    ),
                },
            });
        }
    }
    out
}

/// What the saved values give in a folder: each value, the file it came from, or why there is
/// none; and the capture's other values. The settings window shows it beside each value, and the
/// layout builder shows a text with it.
#[derive(Debug, Serialize, PartialEq, Eq, Default)]
pub struct ValuesPreview {
    /// The project folder the values were read in.
    pub folder: Option<String>,
    /// Every value by its name (`rule.gpu`, `app.exe`…).
    pub values: BTreeMap<String, String>,
    /// The file each value read from files came from, by key, relative to the folder.
    pub sources: BTreeMap<String, String>,
    /// Why a value read from files has none, by key.
    pub problems: BTreeMap<String, String>,
}

/// Run every saved rule in `folder`, then the values made from others, with `known` (the last
/// capture's values) for the rest.
pub fn preview_values(
    rules: &[(String, Rule)],
    folder: Option<&str>,
    known: &BTreeMap<String, String>,
) -> ValuesPreview {
    let root = folder.map(Path::new).filter(|f| f.is_dir());
    let all: Vec<(&str, &Rule)> = rules.iter().map(|(id, r)| (id.as_str(), r)).collect();
    let mut report = cargoframe_rules::run_rules(all.iter().copied(), root, &FsFiles);
    // the capture's own values, without the values its files gave then
    let mut values: BTreeMap<String, String> = known
        .iter()
        .filter(|(k, _)| !k.starts_with("rule."))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (key, o) in &report.values {
        if let Some(v) = &o.value {
            values.insert(format!("rule.{key}"), v.clone());
        }
    }
    cargoframe_rules::combine_values(all.iter().copied(), &mut report, &mut values);
    let problems = report
        .values
        .iter()
        .filter(|(_, o)| o.value.is_none())
        .filter_map(|(k, o)| o.problem.clone().map(|p| (k.clone(), p)))
        .collect();
    ValuesPreview {
        folder: root.map(|r| r.to_string_lossy().into_owned()),
        values,
        sources: report.sources,
        problems,
    }
}

/// A file the settings window can show to pick a value in.
#[derive(Debug, Serialize, PartialEq, Eq, Clone)]
pub struct SampleFile {
    /// Relative to the folder, with `/`.
    pub path: String,
    pub size: u64,
    /// Milliseconds since 1970.
    pub modified_ms: u64,
}

#[derive(Debug, Serialize, PartialEq, Eq, Default)]
pub struct SampleFiles {
    pub folder: Option<String>,
    pub files: Vec<SampleFile>,
}

/// The most files offered to pick a value in.
const MAX_SAMPLES: usize = 40;

fn modified_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as u64)
}

/// The files of the last capture worth picking a value in, the newest first: those its window
/// offers, and those the saved rules find in its folder. Only files inside `folder`, and only text.
pub fn sample_files(
    folder: Option<&str>,
    offered: &[String],
    rules: &[(String, Rule)],
) -> SampleFiles {
    let Some(root) = folder.map(Path::new).filter(|f| f.is_dir()) else {
        return SampleFiles::default();
    };
    let all: Vec<(&str, &Rule)> = rules.iter().map(|(id, r)| (id.as_str(), r)).collect();
    let found = cargoframe_rules::run_rules(all.iter().copied(), Some(root), &FsFiles).found;
    let mut paths: Vec<String> = offered.to_vec();
    for f in found.into_values().flatten() {
        if !paths.contains(&f) {
            paths.push(f);
        }
    }
    let mut files: Vec<SampleFile> = paths
        .into_iter()
        .filter(|p| !crate::tools::looks_binary(p))
        .filter_map(|path| {
            let real = cargoframe_bundle::inside(root, &path).ok()?;
            let meta = std::fs::metadata(&real).ok().filter(|m| m.is_file())?;
            Some(SampleFile {
                size: meta.len(),
                modified_ms: modified_ms(&meta),
                path,
            })
        })
        .collect();
    files.sort_by_key(|f| std::cmp::Reverse(f.modified_ms));
    files.truncate(MAX_SAMPLES);
    SampleFiles {
        folder: Some(root.to_string_lossy().into_owned()),
        files,
    }
}

/// Names of files that are surely not text (pictures, programs, archives…).
pub fn looks_binary(path: &str) -> bool {
    const BINARY: [&str; 16] = [
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "exe", "dll", "so", "zip", "7z", "gz", "pdf",
        "mp4", "webm", "dmp",
    ];
    path.rsplit('.').next().is_some_and(|ext| {
        path.contains('.') && BINARY.contains(&ext.to_ascii_lowercase().as_str())
    })
}

/// The text of a file inside `folder` (its end when it is large), and when it was changed.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SampleText {
    pub text: String,
    pub cut: bool,
    pub size: u64,
    pub modified_ms: u64,
}

pub fn read_sample(folder: &str, path: &str) -> Result<SampleText, String> {
    if looks_binary(path) {
        return Err(t("That file is not text").into());
    }
    let real = cargoframe_bundle::inside(Path::new(folder), path).map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(&real)
        .map_err(|e| tf("The file could not be read: {e}", &[("e", &e)]))?;
    let read = crate::session::read_end(&real, crate::session::MAX_LOG_BYTES)
        .map_err(|e| tf("The file could not be read: {e}", &[("e", &e)]))?;
    Ok(SampleText {
        text: read.text,
        cut: read.cut,
        size: read.size,
        modified_ms: modified_ms(&meta),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_config::Dirs;
    use cargoframe_rules::Format;

    #[test]
    fn every_button_of_a_layout_gets_its_own_reach() {
        use crate::registry::AppRegistry;
        use cargoframe_layout::{
            Block, ConnectionInfo, ConnectionKind, ConnectionState, Rect, SharingButton, Verdict,
        };
        let reg = AppRegistry::with(Default::default(), |id| match id {
            "jira" => Some(ConnectionInfo::new(
                ConnectionKind::Jira,
                ConnectionState::Connected,
            )),
            "reddit" => Some(ConnectionInfo::new(
                ConnectionKind::Reddit,
                ConnectionState::Connected,
            )),
            _ => None,
        });
        let button = |id: &str, connection: &str| Block {
            id: id.into(),
            rect: Rect::new(0, 1, 6, 1),
            kind: BlockKind::SharingButton(SharingButton {
                connection_id: Some(connection.into()),
                ..SharingButton::default()
            }),
        };
        let layout = Layout {
            accepts_input: false,
            columns: cargoframe_layout::GRID_COLUMNS,
            id: "l".into(),
            name: "L".into(),
            blocks: vec![
                Block {
                    id: "files".into(),
                    rect: Rect::new(0, 0, 6, 1),
                    kind: BlockKind::Files(files_block(&[], "")),
                },
                button("to-jira", "jira"),
                button("to-reddit", "reddit"),
            ],
        };
        let all = button_reach(&layout, &reg);
        assert_eq!(all.keys().collect::<Vec<_>>(), ["to-jira", "to-reddit"]);
        assert!(matches!(
            all["to-jira"].of("files"),
            Some(Verdict::Sent { .. })
        ));
        assert!(matches!(
            all["to-reddit"].of("files"),
            Some(Verdict::CannotSend { .. })
        ));
    }

    fn files_block(folders: &[cargoframe_layout::LookFolder], sub: &str) -> FilesBlock {
        FilesBlock {
            title: "Logs".into(),
            include: vec!["*.log".into()],
            preselect: vec!["run.log".into()],
            look_in: cargoframe_layout::LookIn {
                folders: folders.to_vec(),
                subfolder: sub.into(),
            },
        }
    }

    fn deep_project() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let w = |p: &str| {
            let path = dir.path().join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "x").unwrap();
        };
        w("dev/foo/bin/release/foo");
        w("dev/foo/bin/release/logs/run.log");
        w("dev/foo/logs/app.log");
        w("dev/logs/shared.log");
        let program = dir
            .path()
            .join("dev/foo/bin/release/foo")
            .display()
            .to_string();
        (dir, program)
    }

    #[test]
    fn the_files_preview_counts_what_each_folder_holds() {
        use cargoframe_layout::LookFolder::*;
        let (_dir, program) = deep_project();
        let p = preview_files(
            &files_block(&[Program, Above2, Above3], "logs"),
            Some(&program),
        );
        assert_eq!(p.problem, None);
        assert_eq!(p.total, 3);
        assert_eq!(p.ticked, 1, "run.log is ticked by default");
        assert_eq!(p.places.len(), 3);
        assert!(p.places.iter().all(|f| f.exists && f.found == 1));
        assert_eq!(p.program.as_deref(), Some(program.as_str()));
        assert!(p.missing.is_empty());

        // The default is the project folder: dev/foo, where only a subfolder holds logs.
        let none = preview_files(&files_block(&[Project], ""), Some(&program));
        assert_eq!(none.total, 0);
    }

    #[test]
    fn a_rule_test_shows_the_file_it_reads_even_before_the_rule_is_finished() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("run.log"), "first\nsecond\n").unwrap();
        let folder = dir.path().to_str().unwrap();
        // No name and a half-typed query: not saved yet, but the person sees what it points at.
        let draft = Rule {
            file: "*.log".into(),
            format: cargoframe_rules::Format::Text,
            query: "between: a".into(),
            ..Rule::default()
        };
        let t = test_rule(&draft, Some(folder), &[], &Default::default());
        assert!(t.problem.unwrap().contains("two texts"));
        assert_eq!(t.file.as_deref(), Some("run.log"));
        assert_eq!(t.excerpt.as_deref(), Some("first\nsecond"));
        // A finished one gives its value as well.
        let done = Rule {
            query: "line: -1".into(),
            ..draft.clone()
        };
        let t = test_rule(&done, Some(folder), &[], &Default::default());
        assert_eq!(t.value.as_deref(), Some("second"));
        assert_eq!(t.problem, None);
        assert_eq!(t.file.as_deref(), Some("run.log"));
        // File details of the same file.
        let size = Rule {
            format: cargoframe_rules::Format::File,
            query: "size".into(),
            ..draft
        };
        assert_eq!(
            test_rule(&size, Some(folder), &[], &Default::default())
                .value
                .as_deref(),
            Some("13")
        );
        // No folder, or no such file: nothing to show, and no crash.
        let none = test_rule(&done, None, &[], &Default::default());
        assert_eq!((none.file, none.excerpt), (None, None));
        let gone = Rule {
            file: "nothing.log".into(),
            ..done
        };
        assert_eq!(
            test_rule(&gone, Some(folder), &[], &Default::default()).file,
            None
        );
    }

    #[test]
    fn the_files_preview_says_what_it_needs_or_what_is_missing() {
        use cargoframe_layout::LookFolder::*;
        let none = preview_files(&files_block(&[Project], ""), None);
        assert!(none.problem.unwrap().contains("Make a capture"));
        let blank = preview_files(&files_block(&[Project], ""), Some("  "));
        assert!(blank.problem.is_some());
        // A program that is not nested three deep cannot have three folders above it.
        let shallow = preview_files(&files_block(&[Above3], ""), Some("/a/b/prog"));
        assert_eq!(
            shallow.problem.as_deref(),
            Some("There is no folder to look in")
        );
        assert!(shallow.missing[0].contains("Three folders above"));
        // Some found, some not: the count is for what exists and the rest is named.
        let (_dir, program) = deep_project();
        let mixed = preview_files(&files_block(&[Above2, Above3], "nothing"), Some(&program));
        assert_eq!(mixed.total, 0);
        assert!(mixed.places.iter().all(|f| !f.exists));
        assert_eq!(
            mixed.problem.as_deref(),
            Some("None of the folders to look in exist")
        );
    }

    #[test]
    fn a_rule_test_reads_a_value_or_says_why_not() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("package.json"), r#"{"version":"1.0.0"}"#).unwrap();
        let rule = Rule {
            name: "Version".into(),
            file: "package.json".into(),
            format: Format::Json,
            query: "version".into(),
            ..Rule::default()
        };
        let folder = dir.path().to_str().unwrap();
        let ok = test_rule(&rule, Some(folder), &[], &Default::default());
        assert_eq!(ok.value.as_deref(), Some("1.0.0"));
        assert_eq!(ok.problem, None);

        let missing = Rule {
            file: "nothing.json".into(),
            ..rule.clone()
        };
        assert!(test_rule(&missing, Some(folder), &[], &Default::default())
            .problem
            .unwrap()
            .contains("no file matches"));
        assert!(test_rule(&rule, None, &[], &Default::default())
            .problem
            .unwrap()
            .contains("no project folder"));
        let invalid = Rule {
            query: "a[".into(),
            ..rule
        };
        assert!(test_rule(&invalid, Some(folder), &[], &Default::default())
            .problem
            .is_some());
    }

    #[test]
    fn folder_test_checks_existence_and_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        assert!(test_folder(dir.path().to_str().unwrap()).is_ok());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        assert!(test_folder("").is_err());
        assert!(test_folder(dir.path().join("missing").to_str().unwrap())
            .unwrap_err()
            .contains("does not exist"));
    }

    #[test]
    fn variables_list_built_ins_then_rules() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(Dirs::under(dir.path()));
        let names = |s: &Store| variables(s).into_iter().map(|v| v.name).collect::<Vec<_>>();
        assert_eq!(names(&store).len(), vars::NAMES.len());
        store
            .save_item(
                Kind::Rules,
                "build",
                &Rule {
                    name: "Build".into(),
                    file: "a".into(),
                    query: "x".into(),
                    ..Rule::default()
                },
            )
            .unwrap();
        let all = variables(&store);
        assert_eq!(all.last().unwrap().name, "rule.build");
        assert_eq!(all.last().unwrap().description, "Build, from a");
        assert!(all.iter().all(|v| !v.description.is_empty()));
    }

    #[test]
    fn a_value_made_from_other_values_is_previewed_with_the_saved_rules_and_the_last_capture() {
        use cargoframe_rules::{Choice, Find, ValueStep, Which};
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("build.txt"),
            "Stream: main\nConfig: Debug\n",
        )
        .unwrap();
        let read = |key: &str, name: &str| ValueStep {
            key: key.into(),
            name: name.into(),
            find: Find::ValueOf { name: name.into() },
            which: Which::First,
            fallback: None,
            across: Default::default(),
        };
        let saved = Rule {
            name: "Build".into(),
            file: "build.txt".into(),
            values: vec![read("stream", "Stream"), read("config", "Config")],
            ..Rule::default()
        };
        let combined = Rule {
            name: "Build id".into(),
            values: vec![ValueStep {
                key: "build_id".into(),
                name: "Build id".into(),
                find: Find::Combine {
                    choices: vec![Choice {
                        when: vec![],
                        text: "{{rule.stream}}_{{rule.config}}_{{app.exe}}".into(),
                    }],
                },
                which: Which::First,
                fallback: None,
                across: Default::default(),
            }],
            ..Rule::default()
        };
        let known: BTreeMap<String, String> =
            [("app.exe".to_string(), "Editor.exe".to_string())].into();
        let folder = dir.path().to_str().unwrap();
        let t = test_rule(&combined, Some(folder), &[("build".into(), saved)], &known);
        assert_eq!(t.problem, None, "{t:?}");
        assert_eq!(t.value.as_deref(), Some("main_Debug_Editor.exe"));
    }

    #[test]
    fn the_values_of_a_folder_name_their_file_and_the_files_to_pick_in_are_text_inside_it() {
        use cargoframe_rules::{Find, ValueStep, Which};
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        std::fs::create_dir_all(p.join("Saved/Logs")).unwrap();
        std::fs::write(p.join("Saved/Logs/Game.log"), "LogRHI: GPU: RTX 4070\n").unwrap();
        std::fs::write(p.join("shot.png"), [0u8, 1, 2]).unwrap();
        let read = |key: &str, after: &str| ValueStep {
            key: key.into(),
            name: key.into(),
            find: Find::Pick {
                lines_with: after.into(),
                from_line_start: false,
                stop: None,
            },
            which: Which::Last,
            fallback: None,
            across: Default::default(),
        };
        let rules = vec![(
            "game".to_string(),
            Rule {
                name: "Game.log".into(),
                file: "Game.log".into(),
                values: vec![read("gpu", "GPU:"), read("crash", "Fatal error:")],
                ..Rule::default()
            },
        )];
        let known: BTreeMap<String, String> = [
            ("app.exe".to_string(), "Game.exe".to_string()),
            ("rule.gpu".to_string(), "old".to_string()),
        ]
        .into();
        let folder = p.to_str().unwrap();
        let v = preview_values(&rules, Some(folder), &known);
        assert_eq!(v.values["rule.gpu"], "RTX 4070");
        assert_eq!(v.values["app.exe"], "Game.exe");
        assert_eq!(v.sources["gpu"], "Saved/Logs/Game.log");
        assert!(v.problems.contains_key("crash") && !v.values.contains_key("rule.crash"));

        let s = sample_files(
            Some(folder),
            &["shot.png".to_string(), "../x.log".to_string()],
            &rules,
        );
        assert_eq!(
            s.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(),
            ["Saved/Logs/Game.log"]
        );
        assert_eq!(
            read_sample(folder, "Saved/Logs/Game.log").unwrap().text,
            "LogRHI: GPU: RTX 4070\n"
        );
        assert!(read_sample(folder, "../outside.log").is_err());
        assert!(read_sample(folder, "shot.png").is_err());
        assert!(sample_files(None, &[], &rules).files.is_empty());
    }
}
