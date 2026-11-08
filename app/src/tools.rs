//! Small helpers behind Settings buttons: "Test" for a data rule, "Test folder" for safe
//! settings, and the list of variables the builder offers.

use cargoframe_config::{Kind, Store};
use cargoframe_context::climb;
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
pub fn test_rule(rule: &Rule, folder: Option<&str>) -> RuleTest {
    let root = folder.map(Path::new).filter(|f| f.is_dir());
    let seen = root
        .map(|f| cargoframe_rules::peek(rule, f, &FsFiles))
        .unwrap_or_default();
    let mut named = rule.clone();
    if named.name.trim().is_empty() {
        named.name = "preview".into();
    }
    let report = cargoframe_rules::run_rules([("test", &named)], root, &FsFiles);
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
                "Make a capture first, or type the path of a program file, to see what is found"
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
            problem: Some("There is no folder to look in".into()),
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
        return Err("Choose a folder".into());
    }
    if !dir.is_dir() {
        return Err("That folder does not exist".into());
    }
    let probe = dir.join(format!(".cargoframe-write-test-{}", std::process::id()));
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            Err("CargoFrame is not allowed to write there".into())
        }
        Err(e) => Err(format!("Cannot write there: {e}")),
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
        for (id, rule) in loaded.items {
            for step in rule.steps(&id) {
                let file = rule.name.trim();
                let value = step.name.trim();
                out.push(VariableInfo {
                    name: format!("rule.{}", step.key),
                    description: match (file.is_empty(), value.is_empty() || value == file) {
                        (true, true) => "From a file".to_string(),
                        (false, true) => format!("From a file: {file}"),
                        (true, false) => format!("From a file: {value}"),
                        (false, false) => format!("From a file: {file}, {value}"),
                    },
                });
            }
        }
    }
    out
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
        let t = test_rule(&draft, Some(folder));
        assert!(t.problem.unwrap().contains("two texts"));
        assert_eq!(t.file.as_deref(), Some("run.log"));
        assert_eq!(t.excerpt.as_deref(), Some("first\nsecond"));
        // A finished one gives its value as well.
        let done = Rule {
            query: "line: -1".into(),
            ..draft.clone()
        };
        let t = test_rule(&done, Some(folder));
        assert_eq!(t.value.as_deref(), Some("second"));
        assert_eq!(t.problem, None);
        assert_eq!(t.file.as_deref(), Some("run.log"));
        // File details of the same file.
        let size = Rule {
            format: cargoframe_rules::Format::File,
            query: "size".into(),
            ..draft
        };
        assert_eq!(test_rule(&size, Some(folder)).value.as_deref(), Some("13"));
        // No folder, or no such file: nothing to show, and no crash.
        let none = test_rule(&done, None);
        assert_eq!((none.file, none.excerpt), (None, None));
        let gone = Rule {
            file: "nothing.log".into(),
            ..done
        };
        assert_eq!(test_rule(&gone, Some(folder)).file, None);
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
        let ok = test_rule(&rule, Some(folder));
        assert_eq!(ok.value.as_deref(), Some("1.0.0"));
        assert_eq!(ok.problem, None);

        let missing = Rule {
            file: "nothing.json".into(),
            ..rule.clone()
        };
        assert!(test_rule(&missing, Some(folder))
            .problem
            .unwrap()
            .contains("no file matches"));
        assert!(test_rule(&rule, None)
            .problem
            .unwrap()
            .contains("no project folder"));
        let invalid = Rule {
            query: "a[".into(),
            ..rule
        };
        assert!(test_rule(&invalid, Some(folder)).problem.is_some());
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
        assert_eq!(all.last().unwrap().description, "From a file: Build");
        assert!(all.iter().all(|v| !v.description.is_empty()));
    }
}
