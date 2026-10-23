//! Small helpers behind Settings buttons: "Test" for a data rule, "Test folder" for safe
//! settings, and the list of variables the builder offers.

use cargoframe_config::{Kind, Store};
use cargoframe_rules::{run_all, FsFiles, Rule};
use cargoframe_window::vars;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct RuleTest {
    /// The folder the rule was run against.
    pub folder: Option<String>,
    pub value: Option<String>,
    pub problem: Option<String>,
}

/// Run one rule against `folder` (or say there is none). Never writes anything.
pub fn test_rule(rule: &Rule, folder: Option<&str>) -> RuleTest {
    if let Err(e) = cargoframe_rules::check(rule) {
        return RuleTest {
            folder: folder.map(str::to_string),
            value: None,
            problem: Some(e),
        };
    }
    let outcomes = run_all([("test", rule)], folder.map(Path::new), &FsFiles);
    let o = &outcomes["test"];
    RuleTest {
        folder: folder.map(str::to_string),
        value: o.value.clone(),
        problem: o.problem.clone(),
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
            out.push(VariableInfo {
                name: format!("rule.{id}"),
                description: if rule.name.trim().is_empty() {
                    "Data rule".to_string()
                } else {
                    format!("Data rule: {}", rule.name.trim())
                },
            });
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
        assert_eq!(all.last().unwrap().description, "Data rule: Build");
        assert!(all.iter().all(|v| !v.description.is_empty()));
    }
}
