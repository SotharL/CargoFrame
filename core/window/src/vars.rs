//! Built-in `{{variables}}` available to text blocks, prompts and info types.
//!
//! | name             | value                                               |
//! |------------------|-----------------------------------------------------|
//! | `app.exe`        | file name of the program in focus (`Code.exe`)      |
//! | `app.path`       | full path of that program                           |
//! | `app.title`      | the title of its window                             |
//! | `project.folder` | the project folder found by climbing from the path  |
//! | `project.name`   | that folder's name                                  |
//! | `capture.date`   | `YYYY-MM-DD`, local time                            |
//! | `capture.time`   | `HH:MM:SS`, local time                              |
//!
//! Values that cannot be known (no program in focus, no folder found) are left out, so a
//! `{{name}}` that depends on them shows as unresolved instead of as an empty string.

use cargoframe_context::climb::{resolve_project_folder, Match, Resolution};
use std::collections::BTreeMap;

/// Every built-in name, for the builder's variable picker and for validation.
pub const NAMES: [&str; 7] = [
    "app.exe",
    "app.path",
    "app.title",
    "project.folder",
    "project.name",
    "capture.date",
    "capture.time",
];

/// Local date and time, supplied by the caller so this crate stays free of clock code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moment {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Moment {
    pub fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }

    pub fn time(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Builtins {
    pub values: BTreeMap<String, String>,
    pub project: Option<Resolution>,
}

/// Last component of a path written with `/` or `\`.
fn last_part(path: &str) -> &str {
    path.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
}

pub fn builtins(exe_path: Option<&str>, title: Option<&str>, now: Moment) -> Builtins {
    let mut values = BTreeMap::new();
    let mut project = None;
    if let Some(path) = exe_path.filter(|p| !p.is_empty()) {
        values.insert("app.path".to_string(), path.to_string());
        let exe = last_part(path);
        if !exe.is_empty() {
            values.insert("app.exe".to_string(), exe.to_string());
        }
        if let Some(found) = resolve_project_folder(path) {
            values.insert("project.folder".to_string(), found.folder.clone());
            let name = last_part(&found.folder);
            if !name.is_empty() {
                values.insert("project.name".to_string(), name.to_string());
            }
            project = Some(found);
        }
    }
    if let Some(t) = title.filter(|t| !t.is_empty()) {
        values.insert("app.title".to_string(), t.to_string());
    }
    values.insert("capture.date".to_string(), now.date());
    values.insert("capture.time".to_string(), now.time());
    Builtins { values, project }
}

/// A short phrase for how the folder was found, for a tooltip.
pub fn describe(how: &Match) -> String {
    match how {
        Match::SameName { levels_up: 0 } => "the folder holding the program has its name".into(),
        Match::SameName { levels_up } => {
            format!("{levels_up} folder(s) above the program has its name")
        }
        Match::ParentFallback => {
            "no folder has the program's name, so its own folder is used".into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Moment = Moment {
        year: 2026,
        month: 10,
        day: 3,
        hour: 9,
        minute: 5,
        second: 7,
    };

    #[test]
    fn windows_program_in_a_same_named_folder() {
        let b = builtins(
            Some(r"C:\Dev\Foo\bin\Release\Foo.exe"),
            Some("Foo - Editor"),
            NOW,
        );
        assert_eq!(b.values["app.exe"], "Foo.exe");
        assert_eq!(b.values["project.folder"], r"C:\Dev\Foo");
        assert_eq!(b.values["project.name"], "Foo");
        assert_eq!(b.values["app.title"], "Foo - Editor");
        assert_eq!(b.values["capture.date"], "2026-10-03");
        assert_eq!(b.values["capture.time"], "09:05:07");
        assert!(matches!(b.project.unwrap().how, Match::SameName { .. }));
    }

    #[test]
    fn linux_program_without_a_same_named_folder_uses_its_own_folder() {
        let b = builtins(Some("/usr/bin/firefox"), None, NOW);
        assert_eq!(b.values["project.folder"], "/usr/bin");
        assert_eq!(b.values["project.name"], "bin");
        assert!(!b.values.contains_key("app.title"));
        assert_eq!(b.project.unwrap().how, Match::ParentFallback);
    }

    #[test]
    fn nothing_known_about_the_program_leaves_only_the_clock() {
        let b = builtins(None, None, NOW);
        assert_eq!(b.values.len(), 2);
        assert!(b.project.is_none());
        let b = builtins(Some(""), Some(""), NOW);
        assert_eq!(b.values.len(), 2);
    }

    #[test]
    fn every_documented_name_can_be_produced() {
        let b = builtins(Some("/opt/App/bin/App"), Some("t"), NOW);
        for name in NAMES {
            assert!(b.values.contains_key(name), "{name} missing");
        }
    }

    #[test]
    fn describing_the_match() {
        assert!(describe(&Match::ParentFallback).contains("own folder"));
        assert!(describe(&Match::SameName { levels_up: 2 }).contains('2'));
    }
}
