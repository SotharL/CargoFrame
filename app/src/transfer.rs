//! Export and import groups of settings: layouts, data rules and info types, in one file.
//!
//! What a file holds is exactly those three groups. It never holds connections, sign-ins, keys,
//! shortcuts or anything from the credential store, so it is safe to hand to a colleague or keep
//! in a repository. A layout or an info type does name the connection it uses; that name goes
//! along, and the import says when the other computer has no such connection.
//!
//! Importing is two steps so nothing changes until the person has seen what is in the file:
//! [`parse`] reads and checks every item and [`preview`] describes it (new, or already here);
//! [`apply`] then writes what the person picked, each item as `skip`, `replace` or `copy`.
//! Every item goes through the same checks as one typed into Settings.

use crate::items::{self, ItemKind};
use cargoframe_config::{valid_id, InfoType, Kind, Store};
use cargoframe_layout::{BlockKind, DropdownSource, Layout, ValueSource};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const FORMAT: &str = "cargoframe-settings";
pub const VERSION: u32 = 1;
/// A settings file is text a person made; this is far more than any real one.
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ITEMS_PER_GROUP: usize = 500;

/// The groups a file can hold, named as they are in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Group {
    Layouts,
    Rules,
    Infotypes,
}

impl Group {
    pub fn item_kind(self) -> ItemKind {
        match self {
            Group::Layouts => ItemKind::Layout,
            Group::Rules => ItemKind::Rule,
            Group::Infotypes => ItemKind::InfoType,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Group::Layouts => "layout",
            Group::Rules => "data rule",
            Group::Infotypes => "info type",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SettingsFile {
    format: String,
    version: u32,
    /// Which CargoFrame wrote it, for the curious.
    app_version: String,
    /// Local time of the export, as written by the caller.
    exported: String,
    groups: BTreeMap<Group, BTreeMap<String, Value>>,
}

/// What to export: some items of a group, by id.
#[derive(Debug, Clone, Deserialize)]
pub struct Selection {
    pub group: Group,
    pub ids: Vec<String>,
}

/// The text of a settings file holding the selected items. An id that does not exist is
/// reported, not skipped silently.
pub fn export(store: &Store, selection: &[Selection], exported: &str) -> Result<String, String> {
    let mut groups: BTreeMap<Group, BTreeMap<String, Value>> = BTreeMap::new();
    for pick in selection {
        if pick.ids.is_empty() {
            continue;
        }
        let list = items::list(store, pick.group.item_kind())?;
        let have: BTreeMap<&str, &Value> = list
            .items
            .iter()
            .map(|e| (e.id.as_str(), &e.item))
            .collect();
        let out = groups.entry(pick.group).or_default();
        for id in &pick.ids {
            let Some(item) = have.get(id.as_str()) else {
                return Err(format!(
                    "The {} \u{201c}{id}\u{201d} does not exist any more",
                    pick.group.label()
                ));
            };
            out.insert(id.clone(), (*item).clone());
        }
    }
    groups.retain(|_, items| !items.is_empty());
    if groups.is_empty() {
        return Err("Choose something to export".into());
    }
    let file = SettingsFile {
        format: FORMAT.to_string(),
        version: VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        exported: exported.to_string(),
        groups,
    };
    serde_json::to_string_pretty(&file).map_err(|e| e.to_string())
}

/// One item read from a file, already checked.
#[derive(Debug, Clone)]
pub struct Entry {
    pub group: Group,
    pub id: String,
    pub item: Value,
    /// Why it cannot be imported, in words.
    pub error: Option<String>,
}

impl Entry {
    fn name(&self) -> String {
        self.item
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(&self.id)
            .to_string()
    }
}

/// Read and check a settings file. Errors when it is not one of ours; an item that is wrong
/// is kept with its reason so the preview can show it.
pub fn parse(text: &str) -> Result<Vec<Entry>, String> {
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err("This file is too large to be a CargoFrame settings file".into());
    }
    let value: Value = serde_json::from_str(text.strip_prefix('\u{feff}').unwrap_or(text))
        .map_err(|_| "This is not a CargoFrame settings file (it is not valid JSON)".to_string())?;
    match value.get("format").and_then(Value::as_str) {
        Some(FORMAT) => {}
        _ => return Err("This is not a CargoFrame settings file".into()),
    }
    match value.get("version").and_then(Value::as_u64) {
        Some(v) if v == u64::from(VERSION) => {}
        Some(v) if v > u64::from(VERSION) => {
            return Err(
                "This file was made by a newer CargoFrame. Update CargoFrame to read it".into(),
            )
        }
        _ => return Err("This settings file has no readable version".into()),
    }
    let groups = value
        .get("groups")
        .and_then(Value::as_object)
        .ok_or("This settings file holds no groups")?;
    let mut out = Vec::new();
    for (name, items) in groups {
        // A group this CargoFrame does not know is left alone, not an error: a newer one may
        // have added more.
        let Ok(group) = serde_json::from_value::<Group>(Value::String(name.clone())) else {
            continue;
        };
        let items = items
            .as_object()
            .ok_or_else(|| format!("The group \u{201c}{name}\u{201d} is not a list of items"))?;
        if items.len() > MAX_ITEMS_PER_GROUP {
            return Err(format!(
                "The group \u{201c}{name}\u{201d} holds more than {MAX_ITEMS_PER_GROUP} items"
            ));
        }
        for (id, item) in items {
            let error = items::validate(group.item_kind(), id, item).err();
            out.push(Entry {
                group,
                id: id.clone(),
                item: item.clone(),
                error,
            });
        }
    }
    if out.is_empty() {
        return Err("This settings file holds no layouts, data rules or info types".into());
    }
    out.sort_by(|a, b| (a.group, &a.id).cmp(&(b.group, &b.id)));
    Ok(out)
}

/// One line of the import preview.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewItem {
    pub group: Group,
    pub id: String,
    pub name: String,
    /// An item with this id is already here.
    pub exists: bool,
    /// What the one already here is called.
    pub existing_name: Option<String>,
    /// Set when the item cannot be imported at all.
    pub error: Option<String>,
    /// Things worth knowing that do not stop the import.
    pub notes: Vec<String>,
    /// Whether "import as a copy" is possible (not for data rules: layouts name them by id).
    pub can_copy: bool,
}

/// Describe every entry against what is on this computer.
pub fn preview(store: &Store, entries: &[Entry]) -> Vec<PreviewItem> {
    let local_connections: BTreeSet<String> = store
        .list_ids(Kind::Connections)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let local_rules: BTreeSet<String> = store
        .list_ids(Kind::Rules)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let incoming_rules: BTreeSet<&str> = entries
        .iter()
        .filter(|e| e.group == Group::Rules && e.error.is_none())
        .map(|e| e.id.as_str())
        .collect();
    entries
        .iter()
        .map(|e| {
            let existing = items::list(store, e.group.item_kind())
                .ok()
                .and_then(|l| l.items.into_iter().find(|x| x.id == e.id));
            let mut notes = Vec::new();
            if e.error.is_none() {
                match e.group {
                    Group::Layouts => {
                        if let Ok(layout) = serde_json::from_value::<Layout>(e.item.clone()) {
                            let (connections, rules) = layout_references(&layout);
                            for c in connections.difference(&local_connections) {
                                notes.push(format!(
                                    "Uses the connection \u{201c}{c}\u{201d}, which is not on this computer: its button reads \u{201c}Save locally\u{201d} until you add one with that name"
                                ));
                            }
                            for r in rules {
                                if !local_rules.contains(&r) && !incoming_rules.contains(r.as_str())
                                {
                                    notes.push(format!(
                                        "Reads the data rule \u{201c}{r}\u{201d}, which does not exist here"
                                    ));
                                }
                            }
                        }
                    }
                    Group::Infotypes => {
                        if let Ok(t) = serde_json::from_value::<InfoType>(e.item.clone()) {
                            if !local_connections.contains(&t.connection_id) {
                                notes.push(format!(
                                    "Belongs to the connection \u{201c}{}\u{201d}, which is not on this computer",
                                    t.connection_id
                                ));
                            }
                        }
                    }
                    Group::Rules => {}
                }
            }
            PreviewItem {
                group: e.group,
                id: e.id.clone(),
                name: e.name(),
                exists: existing.is_some(),
                existing_name: existing.as_ref().map(|x| {
                    x.item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or(&x.id)
                        .to_string()
                }),
                error: e.error.clone(),
                notes,
                can_copy: e.group != Group::Rules,
            }
        })
        .collect()
}

/// The connections a layout names and the data rules its sharing buttons read.
fn layout_references(layout: &Layout) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut connections = BTreeSet::new();
    let mut rules = BTreeSet::new();
    for block in &layout.blocks {
        match &block.kind {
            BlockKind::SharingButton(b) => {
                if let Some(c) = &b.connection_id {
                    connections.insert(c.clone());
                }
                for m in &b.mappings {
                    if let ValueSource::Rule(id) = &m.source {
                        rules.insert(id.clone());
                    }
                }
            }
            BlockKind::Dropdown(d) => {
                if let DropdownSource::WebApi { connection_id, .. } = &d.source {
                    connections.insert(connection_id.clone());
                }
            }
            _ => {}
        }
    }
    (connections, rules)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Skip,
    /// Write it under its own id, replacing the one that is here (if any).
    Replace,
    /// Write it under a new id and leave the one that is here alone.
    Copy,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Pick {
    pub group: Group,
    pub id: String,
    pub action: Action,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Imported {
    pub group: Group,
    /// The id it was written under (a copy gets a new one).
    pub id: String,
    pub name: String,
    pub copy: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct Report {
    pub imported: Vec<Imported>,
    pub skipped: usize,
    /// Items that could not be written, with the reason.
    pub failed: Vec<String>,
}

/// Write what the person picked. Each item is checked again as it is written; one that fails
/// never stops the rest.
pub fn apply(store: &Store, entries: &[Entry], picks: &[Pick]) -> Report {
    let mut report = Report::default();
    let mut done = BTreeSet::new();
    for pick in picks {
        if !done.insert((pick.group, pick.id.clone())) {
            continue;
        }
        if pick.action == Action::Skip {
            report.skipped += 1;
            continue;
        }
        let Some(entry) = entries
            .iter()
            .find(|e| e.group == pick.group && e.id == pick.id)
        else {
            report
                .failed
                .push(format!("\u{201c}{}\u{201d} is not in the file", pick.id));
            continue;
        };
        if let Some(why) = &entry.error {
            report
                .failed
                .push(format!("\u{201c}{}\u{201d}: {why}", entry.name()));
            continue;
        }
        let kind = entry.group.item_kind();
        let (id, item, copy) = match pick.action {
            Action::Replace => (entry.id.clone(), entry.item.clone(), false),
            Action::Copy => {
                if entry.group == Group::Rules {
                    report.failed.push(format!(
                        "\u{201c}{}\u{201d}: a data rule cannot be imported as a copy, because layouts refer to it by its id",
                        entry.name()
                    ));
                    continue;
                }
                let id = match items::new_id(store, kind, &entry.name()) {
                    Ok(id) => id,
                    Err(e) => {
                        report
                            .failed
                            .push(format!("\u{201c}{}\u{201d}: {e}", entry.name()));
                        continue;
                    }
                };
                let mut item = entry.item.clone();
                if let Some(o) = item.as_object_mut() {
                    let name = format!("{} (imported)", entry.name());
                    o.insert("name".into(), Value::String(name));
                }
                (id, item, true)
            }
            Action::Skip => unreachable!("handled above"),
        };
        if !valid_id(&id) {
            report.failed.push(format!(
                "\u{201c}{}\u{201d}: the id is not usable",
                entry.name()
            ));
            continue;
        }
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(&id)
            .to_string();
        match items::save(store, kind, &id, item) {
            Ok(()) => report.imported.push(Imported {
                group: entry.group,
                id,
                name,
                copy,
            }),
            Err(e) => report
                .failed
                .push(format!("\u{201c}{}\u{201d}: {e}", entry.name())),
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_config::Dirs;
    use serde_json::json;

    fn store() -> (tempfile::TempDir, Store) {
        let d = tempfile::tempdir().unwrap();
        let s = Store::new(Dirs::under(d.path()));
        (d, s)
    }

    fn rule(name: &str) -> Value {
        json!({"name": name, "file": "package.json", "format": "json", "query": "version"})
    }

    fn layout(name: &str, connection: &str) -> Value {
        json!({"id": "", "name": name, "blocks": [
            {"id": "s", "rect": {"x":0,"y":0,"w":6,"h":1}, "type": "sharing_button",
             "label": "Send", "connection_id": connection,
             "mappings": [{"field": "build", "source": {"kind": "rule", "value": "version"}}]}
        ]})
    }

    fn info(connection: &str) -> Value {
        json!({"name": "Priority", "connection_id": connection, "field": "priority", "kind": "text"})
    }

    fn seed(s: &Store) {
        items::save(s, ItemKind::Rule, "version", rule("Version")).unwrap();
        items::save(
            s,
            ItemKind::Layout,
            "bug",
            layout("Bug report", "work-jira"),
        )
        .unwrap();
        items::save(s, ItemKind::InfoType, "prio", info("work-jira")).unwrap();
    }

    fn everything() -> Vec<Selection> {
        vec![
            Selection {
                group: Group::Layouts,
                ids: vec!["bug".into()],
            },
            Selection {
                group: Group::Rules,
                ids: vec!["version".into()],
            },
            Selection {
                group: Group::Infotypes,
                ids: vec!["prio".into()],
            },
        ]
    }

    #[test]
    fn an_export_holds_the_chosen_items_and_nothing_else() {
        let (_d, s) = store();
        seed(&s);
        items::save(&s, ItemKind::Rule, "other", rule("Other")).unwrap();
        // A connection and a trigger exist, and must not be in the file.
        items::save(
            &s,
            ItemKind::Connection,
            "work-jira",
            json!({"name": "Work", "kind": "jira", "site_url": "https://a.atlassian.net", "email": "me@a.com"}),
        )
        .unwrap();
        items::save(&s, ItemKind::Trigger, "t", json!({"name": "T"})).unwrap();

        let text = export(&s, &everything(), "2026-10-23 12:00").unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["format"], FORMAT);
        assert_eq!(v["version"], VERSION);
        assert_eq!(v["groups"].as_object().unwrap().len(), 3);
        assert!(v["groups"]["rules"].get("version").is_some());
        assert!(
            v["groups"]["rules"].get("other").is_none(),
            "only what was chosen"
        );
        assert!(!text.contains("atlassian"), "no connection settings");
        assert!(!text.contains("\"triggers\""));
    }

    #[test]
    fn nothing_chosen_or_an_item_that_is_gone_is_an_error() {
        let (_d, s) = store();
        seed(&s);
        assert!(export(&s, &[], "x").unwrap_err().contains("Choose"));
        let none = [Selection {
            group: Group::Rules,
            ids: vec![],
        }];
        assert!(export(&s, &none, "x").unwrap_err().contains("Choose"));
        let gone = [Selection {
            group: Group::Rules,
            ids: vec!["nope".into()],
        }];
        assert!(export(&s, &gone, "x").unwrap_err().contains("nope"));
    }

    #[test]
    fn a_file_goes_from_one_computer_to_another() {
        let (_a, from) = store();
        seed(&from);
        let text = export(&from, &everything(), "now").unwrap();

        let (_b, to) = store();
        let entries = parse(&text).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().all(|e| e.error.is_none()));
        let shown = preview(&to, &entries);
        assert!(shown.iter().all(|p| !p.exists));
        let picks: Vec<Pick> = entries
            .iter()
            .map(|e| Pick {
                group: e.group,
                id: e.id.clone(),
                action: Action::Replace,
            })
            .collect();
        let report = apply(&to, &entries, &picks);
        assert_eq!(report.imported.len(), 3, "{report:?}");
        assert!(report.failed.is_empty());
        assert_eq!(
            items::list(&to, ItemKind::Layout).unwrap().items[0].id,
            "bug"
        );
        assert_eq!(
            items::list(&to, ItemKind::Rule).unwrap().items[0].item["name"],
            "Version"
        );
        assert_eq!(
            items::list(&to, ItemKind::InfoType).unwrap().items[0].item["connection_id"],
            "work-jira"
        );
    }

    #[test]
    fn the_preview_says_what_exists_and_what_is_missing_here() {
        let (_a, from) = store();
        seed(&from);
        let entries = parse(&export(&from, &everything(), "now").unwrap()).unwrap();

        let (_b, to) = store();
        items::save(
            &to,
            ItemKind::Layout,
            "bug",
            layout("My own bug layout", "x"),
        )
        .unwrap();
        let shown = preview(&to, &entries);
        let bug = shown.iter().find(|p| p.group == Group::Layouts).unwrap();
        assert!(bug.exists && bug.can_copy);
        assert_eq!(bug.existing_name.as_deref(), Some("My own bug layout"));
        // The connection and the rule it reads are not here; the rule is in the file, so only
        // the connection is reported.
        assert_eq!(bug.notes.len(), 1, "{:?}", bug.notes);
        assert!(bug.notes[0].contains("work-jira"));
        let prio = shown.iter().find(|p| p.group == Group::Infotypes).unwrap();
        assert!(prio.notes[0].contains("work-jira"));
        let rule = shown.iter().find(|p| p.group == Group::Rules).unwrap();
        assert!(!rule.can_copy);

        // Without the rule in the file, the layout warns about it too.
        let only_layout: Vec<Entry> = entries
            .iter()
            .filter(|e| e.group == Group::Layouts)
            .cloned()
            .collect();
        let shown = preview(&to, &only_layout);
        assert!(
            shown[0].notes.iter().any(|n| n.contains("data rule")),
            "{:?}",
            shown[0].notes
        );
    }

    #[test]
    fn replace_overwrites_copy_keeps_both_and_skip_leaves_it() {
        let (_a, from) = store();
        seed(&from);
        let entries = parse(&export(&from, &everything(), "now").unwrap()).unwrap();

        let (_b, to) = store();
        items::save(&to, ItemKind::Layout, "bug", layout("Mine", "x")).unwrap();
        items::save(&to, ItemKind::Rule, "version", rule("Mine too")).unwrap();
        let pick = |g, a| Pick {
            group: g,
            id: match g {
                Group::Layouts => "bug",
                Group::Rules => "version",
                Group::Infotypes => "prio",
            }
            .into(),
            action: a,
        };

        let report = apply(
            &to,
            &entries,
            &[
                pick(Group::Layouts, Action::Copy),
                pick(Group::Rules, Action::Skip),
                pick(Group::Infotypes, Action::Replace),
            ],
        );
        assert_eq!(report.skipped, 1);
        assert_eq!(report.imported.len(), 2, "{report:?}");
        let layouts = items::list(&to, ItemKind::Layout).unwrap().items;
        assert_eq!(layouts.len(), 2);
        assert!(
            layouts
                .iter()
                .any(|l| l.id == "bug" && l.item["name"] == "Mine"),
            "the original is untouched"
        );
        let copy = report
            .imported
            .iter()
            .find(|i| i.group == Group::Layouts)
            .unwrap();
        assert!(copy.copy && copy.id != "bug");
        assert_eq!(copy.name, "Bug report (imported)");
        assert_eq!(
            items::list(&to, ItemKind::Rule).unwrap().items[0].item["name"],
            "Mine too"
        );

        let report = apply(&to, &entries, &[pick(Group::Rules, Action::Replace)]);
        assert_eq!(report.imported.len(), 1);
        assert_eq!(
            items::list(&to, ItemKind::Rule).unwrap().items[0].item["name"],
            "Version"
        );
    }

    #[test]
    fn a_data_rule_cannot_become_a_copy() {
        let (_a, from) = store();
        seed(&from);
        let entries = parse(&export(&from, &everything(), "now").unwrap()).unwrap();
        let (_b, to) = store();
        let report = apply(
            &to,
            &entries,
            &[Pick {
                group: Group::Rules,
                id: "version".into(),
                action: Action::Copy,
            }],
        );
        assert!(report.imported.is_empty());
        assert!(report.failed[0].contains("copy"));
        assert!(items::list(&to, ItemKind::Rule).unwrap().items.is_empty());
    }

    #[test]
    fn files_that_are_not_ours_are_refused_with_a_reason() {
        for (text, why) in [
            ("not json", "not valid JSON"),
            ("{}", "not a CargoFrame settings file"),
            (
                r#"{"format":"other","version":1,"groups":{}}"#,
                "not a CargoFrame",
            ),
            (
                r#"{"format":"cargoframe-settings","version":99,"groups":{}}"#,
                "newer",
            ),
            (
                r#"{"format":"cargoframe-settings","version":1}"#,
                "no groups",
            ),
            (
                r#"{"format":"cargoframe-settings","version":1,"groups":{}}"#,
                "holds no",
            ),
            (
                r#"{"format":"cargoframe-settings","version":1,"groups":{"rules":[]}}"#,
                "not a list",
            ),
        ] {
            let e = parse(text).unwrap_err();
            assert!(e.contains(why), "{text:?}: {e}");
        }
        let big = " ".repeat(MAX_FILE_BYTES as usize + 1);
        assert!(parse(&big).unwrap_err().contains("too large"));
    }

    #[test]
    fn a_group_from_a_newer_version_is_ignored_not_fatal() {
        let text = json!({"format": FORMAT, "version": 1, "app_version": "9", "exported": "x",
            "groups": {"futuristic": {"a": {}}, "rules": {"v": rule("V")}}})
        .to_string();
        let entries = parse(&text).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].group, Group::Rules);
    }

    #[test]
    fn a_bad_item_is_shown_with_its_reason_and_never_written() {
        let text = json!({"format": FORMAT, "version": 1, "app_version": "1", "exported": "x",
        "groups": {"rules": {
            "good": rule("Good"),
            "bad": {"name": "Bad", "file": "../secret.json", "format": "json", "query": "x"},
            "BAD ID": rule("Odd id")
        }}})
        .to_string();
        let entries = parse(&text).unwrap();
        let (_d, s) = store();
        let shown = preview(&s, &entries);
        assert_eq!(shown.iter().filter(|p| p.error.is_some()).count(), 2);
        let picks: Vec<Pick> = entries
            .iter()
            .map(|e| Pick {
                group: e.group,
                id: e.id.clone(),
                action: Action::Replace,
            })
            .collect();
        let report = apply(&s, &entries, &picks);
        assert_eq!(report.imported.len(), 1);
        assert_eq!(report.failed.len(), 2);
        let ids: Vec<String> = items::list(&s, ItemKind::Rule)
            .unwrap()
            .items
            .into_iter()
            .map(|e| e.id)
            .collect();
        assert_eq!(ids, ["good"]);
    }

    #[test]
    fn a_pick_for_something_not_in_the_file_changes_nothing() {
        let (_a, from) = store();
        seed(&from);
        let entries = parse(&export(&from, &everything(), "now").unwrap()).unwrap();
        let (_b, to) = store();
        let report = apply(
            &to,
            &entries,
            &[Pick {
                group: Group::Layouts,
                id: "../evil".into(),
                action: Action::Replace,
            }],
        );
        assert_eq!(report.imported.len(), 0);
        assert_eq!(report.failed.len(), 1);
    }

    #[test]
    fn the_same_item_picked_twice_is_written_once() {
        let (_a, from) = store();
        seed(&from);
        let entries = parse(&export(&from, &everything(), "now").unwrap()).unwrap();
        let (_b, to) = store();
        let p = Pick {
            group: Group::Infotypes,
            id: "prio".into(),
            action: Action::Replace,
        };
        let report = apply(&to, &entries, &[p.clone(), p]);
        assert_eq!(report.imported.len(), 1);
    }

    #[test]
    fn secrets_have_no_way_into_a_file() {
        // The only groups there are: connections, triggers and the credential store are not
        // among them, and a file naming one is ignored.
        let groups = [Group::Layouts, Group::Rules, Group::Infotypes];
        for g in groups {
            assert!(!matches!(
                g.item_kind(),
                ItemKind::Connection | ItemKind::Trigger
            ));
        }
        let names: Vec<String> = groups
            .iter()
            .map(|g| {
                serde_json::to_value(g)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        assert_eq!(names, ["layouts", "rules", "infotypes"]);
        let text = json!({"format": FORMAT, "version": 1, "app_version": "1", "exported": "x",
            "groups": {"connections": {"c": {"name": "C", "kind": "claude"}},
                       "triggers": {"t": {"name": "T"}},
                       "rules": {"v": rule("V")}}})
        .to_string();
        let entries = parse(&text).unwrap();
        assert_eq!(entries.len(), 1, "only the rule is read");
    }
}
