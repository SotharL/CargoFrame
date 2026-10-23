//! Reading, checking and writing the configuration items the Settings window edits: triggers,
//! layouts, data rules and info types. One generic pair of functions serves all four so the
//! window needs only a handful of commands.

use cargoframe_config::{infotype, shortcut, InfoType, Kind, Store, Trigger};
use cargoframe_layout::Layout;
use cargoframe_rules::Rule;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    Trigger,
    Layout,
    Rule,
    InfoType,
}

impl ItemKind {
    pub fn kind(self) -> Kind {
        match self {
            ItemKind::Trigger => Kind::Triggers,
            ItemKind::Layout => Kind::Layouts,
            ItemKind::Rule => Kind::Rules,
            ItemKind::InfoType => Kind::InfoTypes,
        }
    }
}

#[derive(Debug, Serialize, PartialEq)]
pub struct ItemEntry {
    pub id: String,
    pub item: Value,
}

#[derive(Debug, Serialize, Default, PartialEq)]
pub struct ItemList {
    pub items: Vec<ItemEntry>,
    /// Files that could not be read, in words. They are left untouched on disk.
    pub problems: Vec<String>,
}

fn to_entries<T: serde::Serialize>(
    loaded: cargoframe_config::Loaded<T>,
    fix: impl Fn(&str, &mut Value),
) -> ItemList {
    let mut out = ItemList::default();
    for (id, item) in loaded.items {
        match serde_json::to_value(&item) {
            Ok(mut v) => {
                fix(&id, &mut v);
                out.items.push(ItemEntry { id, item: v });
            }
            Err(e) => out.problems.push(format!("{id}: {e}")),
        }
    }
    out.problems
        .extend(loaded.errors.iter().map(ToString::to_string));
    out
}

pub fn list(store: &Store, kind: ItemKind) -> Result<ItemList, String> {
    let e = |e: cargoframe_config::ConfigError| e.to_string();
    Ok(match kind {
        ItemKind::Trigger => to_entries(
            store.load_all::<Trigger>(Kind::Triggers).map_err(e)?,
            |_, _| {},
        ),
        ItemKind::Rule => to_entries(store.load_all::<Rule>(Kind::Rules).map_err(e)?, |_, _| {}),
        ItemKind::InfoType => to_entries(
            store.load_all::<InfoType>(Kind::InfoTypes).map_err(e)?,
            |_, _| {},
        ),
        // A layout's id is its file name, whatever the file says.
        ItemKind::Layout => to_entries(
            store.load_all::<Layout>(Kind::Layouts).map_err(e)?,
            |id, v| {
                if let Some(o) = v.as_object_mut() {
                    o.insert("id".into(), Value::String(id.to_string()));
                }
            },
        ),
    })
}

fn parse<T: serde::de::DeserializeOwned>(item: Value) -> Result<T, String> {
    serde_json::from_value(item).map_err(|e| format!("This item is not valid: {e}"))
}

/// Validate `item` for `kind` and write it as `id`. Triggers get their shortcut rewritten to the
/// canonical form so it looks the same everywhere.
pub fn save(store: &Store, kind: ItemKind, id: &str, item: Value) -> Result<(), String> {
    if !cargoframe_config::valid_id(id) {
        return Err(format!("\"{id}\" cannot be used as a file name"));
    }
    let e = |e: cargoframe_config::ConfigError| e.to_string();
    match kind {
        ItemKind::Trigger => {
            let mut t: Trigger = parse(item)?;
            if t.name.trim().is_empty() {
                return Err("Give the trigger a name".into());
            }
            if !t.shortcut.trim().is_empty() {
                t.shortcut = shortcut::normalize(&t.shortcut).map_err(|e| e.to_string())?;
            }
            if let Some(l) = &t.layout_id {
                if !cargoframe_config::valid_id(l) {
                    return Err("The layout id is not valid".into());
                }
            }
            store.save_item(Kind::Triggers, id, &t).map_err(e)
        }
        ItemKind::Layout => {
            let mut l: Layout = parse(item)?;
            l.id = id.to_string();
            if l.name.trim().is_empty() {
                return Err("Give the layout a name".into());
            }
            store.save_item(Kind::Layouts, id, &l).map_err(e)
        }
        ItemKind::Rule => {
            let r: Rule = parse(item)?;
            cargoframe_rules::check(&r)?;
            store.save_item(Kind::Rules, id, &r).map_err(e)
        }
        ItemKind::InfoType => {
            let t: InfoType = parse(item)?;
            infotype::check(&t)?;
            store.save_item(Kind::InfoTypes, id, &t).map_err(e)
        }
    }
}

pub fn delete(store: &Store, kind: ItemKind, id: &str) -> Result<(), String> {
    store
        .delete_item(kind.kind(), id)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A free file name for a new item called `name`: lowercase, dashes, `-2`, `-3` on a clash.
pub fn new_id(store: &Store, kind: ItemKind, name: &str) -> Result<String, String> {
    let mut slug = String::new();
    let mut dash = true;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }
    let slug = slug.trim_matches('-');
    let base: String = if slug.is_empty() {
        match kind {
            ItemKind::Trigger => "trigger",
            ItemKind::Layout => "layout",
            ItemKind::Rule => "rule",
            ItemKind::InfoType => "info",
        }
        .to_string()
    } else {
        slug.chars()
            .take(48)
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    };
    let taken: std::collections::BTreeSet<String> = store
        .list_ids(kind.kind())
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();
    if !taken.contains(&base) {
        return Ok(base);
    }
    (2u32..)
        .map(|n| format!("{base}-{n}"))
        .find(|c| !taken.contains(c))
        .ok_or_else(|| "no free name".to_string())
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

    #[test]
    fn a_trigger_is_checked_normalised_and_listed() {
        let (_d, s) = store();
        save(
            &s,
            ItemKind::Trigger,
            "bug",
            json!({"name": "Bug", "shortcut": "shift+ctrl+f9", "capture": "display", "layout_id": "l"}),
        )
        .unwrap();
        let list = list(&s, ItemKind::Trigger).unwrap();
        assert_eq!(list.items.len(), 1);
        assert_eq!(list.items[0].id, "bug");
        assert_eq!(list.items[0].item["shortcut"], "Ctrl+Shift+F9");
        assert_eq!(list.items[0].item["capture"], "display");
        assert!(list.problems.is_empty());
    }

    #[test]
    fn bad_items_are_refused_with_a_reason_and_nothing_is_written() {
        let (_d, s) = store();
        let cases = [
            (ItemKind::Trigger, "t", json!({"name": " "}), "name"),
            (
                ItemKind::Trigger,
                "t",
                json!({"name": "x", "shortcut": "F9+F10"}),
                "",
            ),
            (
                ItemKind::Trigger,
                "BAD ID",
                json!({"name": "x"}),
                "file name",
            ),
            (ItemKind::Layout, "l", json!({"name": ""}), "name"),
            (
                ItemKind::Rule,
                "r",
                json!({"name": "R", "file": "../x", "query": "a"}),
                "inside the project",
            ),
            (ItemKind::InfoType, "i", json!({"name": "N"}), "connection"),
            (ItemKind::Rule, "r", json!({"name": 5}), "not valid"),
        ];
        for (kind, id, item, needle) in cases {
            let err = save(&s, kind, id, item).unwrap_err();
            assert!(err.to_lowercase().contains(needle), "{err}");
        }
        for kind in [
            ItemKind::Trigger,
            ItemKind::Layout,
            ItemKind::Rule,
            ItemKind::InfoType,
        ] {
            assert!(list(&s, kind).unwrap().items.is_empty());
        }
    }

    #[test]
    fn layouts_take_their_id_from_the_file_name() {
        let (_d, s) = store();
        save(
            &s,
            ItemKind::Layout,
            "bug",
            json!({"id": "other", "name": "Bug", "blocks": []}),
        )
        .unwrap();
        let l = list(&s, ItemKind::Layout).unwrap();
        assert_eq!(l.items[0].item["id"], "bug");
    }

    #[test]
    fn rules_and_info_types_round_trip() {
        let (_d, s) = store();
        save(
            &s,
            ItemKind::Rule,
            "version",
            json!({"name": "Version", "file": "package.json", "format": "json", "query": "version"}),
        )
        .unwrap();
        save(
            &s,
            ItemKind::InfoType,
            "prio",
            json!({"name": "Priority", "connection_id": "jira", "field": "priority"}),
        )
        .unwrap();
        assert_eq!(
            list(&s, ItemKind::Rule).unwrap().items[0].item["query"],
            "version"
        );
        assert_eq!(
            list(&s, ItemKind::InfoType).unwrap().items[0].item["field"],
            "priority"
        );
        delete(&s, ItemKind::Rule, "version").unwrap();
        assert!(list(&s, ItemKind::Rule).unwrap().items.is_empty());
        // Deleting what is not there is not an error.
        delete(&s, ItemKind::Rule, "version").unwrap();
    }

    #[test]
    fn a_damaged_file_is_reported_and_left_alone() {
        let (d, s) = store();
        let dir = d.path().join("config").join("rules");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.json"), "{ nope").unwrap();
        let l = list(&s, ItemKind::Rule).unwrap();
        assert!(l.items.is_empty());
        assert_eq!(l.problems.len(), 1);
        assert_eq!(
            std::fs::read_to_string(dir.join("broken.json")).unwrap(),
            "{ nope"
        );
    }

    #[test]
    fn new_ids_are_readable_and_never_clash() {
        let (_d, s) = store();
        assert_eq!(
            new_id(&s, ItemKind::Layout, "Bug report!").unwrap(),
            "bug-report"
        );
        save(&s, ItemKind::Layout, "bug-report", json!({"name": "x"})).unwrap();
        assert_eq!(
            new_id(&s, ItemKind::Layout, "Bug report").unwrap(),
            "bug-report-2"
        );
        assert_eq!(new_id(&s, ItemKind::Rule, "  ").unwrap(), "rule");
        assert_eq!(new_id(&s, ItemKind::Trigger, "ÄÖ").unwrap(), "trigger");
        let long = "x".repeat(200);
        assert!(new_id(&s, ItemKind::Rule, &long).unwrap().len() <= 48);
    }
}
