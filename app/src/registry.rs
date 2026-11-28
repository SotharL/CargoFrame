//! What the layout checks know about the rest of the configuration.

use crate::connections::Connections;
use cargoframe_config::{InfoType, Kind, Store};
use cargoframe_layout::{ConnectionInfo, FieldNeed, Registry};
use cargoframe_window::vars;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

type ConnectionLookup<'a> = Box<dyn Fn(&str) -> Option<ConnectionInfo> + 'a>;
type ListLookup<'a> = Box<dyn Fn(&str, &str) -> bool + 'a>;
type NeedsLookup<'a> = Box<dyn Fn(&str) -> Vec<FieldNeed> + 'a>;

pub struct AppRegistry<'a> {
    pub rules: BTreeSet<String>,
    connection: ConnectionLookup<'a>,
    /// (connection id, field) of every field its service requires.
    required: BTreeSet<(String, String)>,
    /// Does (connection id, list) search for what is typed?
    searches: ListLookup<'a>,
    /// What each connection's service refuses to share without, asked once per connection.
    needs: NeedsLookup<'a>,
    needs_seen: RefCell<BTreeMap<String, Vec<FieldNeed>>>,
    /// (connection id, field) of every field a field fill sets.
    filled: BTreeSet<(String, String)>,
}

impl<'a> AppRegistry<'a> {
    /// The rules on disk, and the connections as `connections` reports them. A connection is
    /// looked at only when a layout asks about it, so a layout that uses none never touches the
    /// credential store.
    pub fn from_store(store: &'a Store, connections: &'a Connections) -> Self {
        let infos: Vec<InfoType> = store
            .load_all::<InfoType>(Kind::InfoTypes)
            .map(|l| l.items.into_iter().map(|(_, t)| t).collect())
            .unwrap_or_default();
        let required = infos
            .iter()
            .filter(|t| t.required)
            .map(|t| (t.connection_id.clone(), t.field.clone()))
            .collect();
        let filled = infos
            .iter()
            .filter(|t| t.fills())
            .map(|t| (t.connection_id.clone(), t.field.clone()))
            .collect();
        // a value is named by its key (`{gpu}`), a rule from before 0.3.0 by its id
        let mut rules: BTreeSet<String> = store
            .list_ids(Kind::Rules)
            .unwrap_or_default()
            .into_iter()
            .collect();
        if let Ok(loaded) = store.load_all::<cargoframe_rules::Rule>(Kind::Rules) {
            for (id, r) in &loaded.items {
                rules.extend(r.steps(id).into_iter().map(|s| s.key));
            }
        }
        let mut reg = Self::with(rules, move |id| connections.info(store, id)).requiring(required);
        reg.filled = filled;
        // the connector says what its service needs; read without the network
        reg.needs = Box::new(move |id| {
            connections
                .describe(store, id)
                .map(|d| needs_of(&d))
                .unwrap_or_default()
        });
        // the connector says which of its lists it searches; asked only for a dropdown's list
        reg.searches = Box::new(move |id, list| {
            connections
                .describe(store, id)
                .is_ok_and(|d| d.lists.iter().any(|l| l.id == list && l.search))
        });
        reg
    }

    /// The fields the services require, as (connection id, field).
    pub fn requiring(mut self, required: BTreeSet<(String, String)>) -> Self {
        self.required = required;
        self
    }

    pub fn with(
        rules: BTreeSet<String>,
        connection: impl Fn(&str) -> Option<ConnectionInfo> + 'a,
    ) -> Self {
        Self {
            rules,
            connection: Box::new(connection),
            required: BTreeSet::new(),
            searches: Box::new(|_, _| false),
            needs: Box::new(|_| Vec::new()),
            needs_seen: RefCell::new(BTreeMap::new()),
            filled: BTreeSet::new(),
        }
    }
}

/// The fields a connector marks as required, as the layout checks know them.
pub fn needs_of(d: &cargoframe_connectors::Describe) -> Vec<FieldNeed> {
    d.fields
        .iter()
        .filter(|f| f.required)
        .map(|f| FieldNeed {
            field: f.key.to_string(),
            label: f.label.to_string(),
            unless: f.unless.map(str::to_string),
        })
        .collect()
}

impl Registry for AppRegistry<'_> {
    fn connection(&self, id: &str) -> Option<ConnectionInfo> {
        (self.connection)(id)
    }

    fn rule_exists(&self, id: &str) -> bool {
        self.rules.contains(id)
    }

    fn is_known_variable(&self, name: &str) -> bool {
        vars::NAMES.contains(&name)
            || crate::debug::NAMES.contains(&name)
            || cargoframe_context::template::rule_key(name)
                .is_some_and(|key| self.rules.contains(key))
    }

    fn field_required(&self, connection_id: &str, field: &str) -> bool {
        self.required
            .contains(&(connection_id.to_string(), field.to_string()))
            || self.needs(connection_id).iter().any(|n| n.field == field)
    }

    fn list_searches(&self, connection_id: &str, list: &str) -> bool {
        (self.searches)(connection_id, list)
    }

    fn needs(&self, connection_id: &str) -> Vec<FieldNeed> {
        if let Some(n) = self.needs_seen.borrow().get(connection_id) {
            return n.clone();
        }
        let n = (self.needs)(connection_id);
        self.needs_seen
            .borrow_mut()
            .insert(connection_id.to_string(), n.clone());
        n
    }

    fn field_filled(&self, connection_id: &str, field: &str) -> bool {
        self.filled
            .contains(&(connection_id.to_string(), field.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_layout::{ConnectionKind, ConnectionState};

    #[test]
    fn built_ins_and_known_rules_resolve() {
        let reg = AppRegistry::with(["version".to_string()].into(), |id| {
            (id == "jira").then_some(ConnectionInfo::new(
                ConnectionKind::Jira,
                ConnectionState::Connected,
            ))
        });
        assert!(reg.is_known_variable("app.exe"));
        assert!(reg.is_known_variable("rule.version"));
        assert!(!reg.is_known_variable("rule.other"));
        assert!(!reg.is_known_variable("nope"));
        assert!(reg.rule_exists("version") && !reg.rule_exists("x"));
        assert!(reg.connection("jira").is_some());
        assert!(reg.connection("other").is_none());
    }

    #[test]
    fn required_fields_are_known_by_connection_and_field() {
        let reg = AppRegistry::with(BTreeSet::new(), |_| None)
            .requiring([("jira".to_string(), "summary".to_string())].into());
        assert!(reg.field_required("jira", "summary"));
        assert!(!reg.field_required("jira", "priority"));
        assert!(!reg.field_required("slack", "summary"));
    }

    #[test]
    fn each_service_says_what_it_refuses_to_share_without() {
        let needs = |settings: serde_json::Value| {
            let mut c = serde_json::json!({"name": "C"});
            for (k, v) in settings.as_object().unwrap() {
                c[k] = v.clone();
            }
            let c: cargoframe_connectors::Connection = serde_json::from_value(c).unwrap();
            let d = cargoframe_connectors::build(&c).unwrap().describe();
            needs_of(&d)
                .into_iter()
                .map(|n| (n.field, n.unless))
                .collect::<Vec<_>>()
        };
        let need = |f: &str, u: Option<&str>| (f.to_string(), u.map(str::to_string));
        assert_eq!(
            needs(serde_json::json!({"kind": "slack"})),
            vec![need("channel", Some("thread"))]
        );
        assert_eq!(
            needs(serde_json::json!({"kind": "teams", "client_id": "x"})),
            vec![need("target", Some("thread"))]
        );
        assert_eq!(
            needs(serde_json::json!({"kind": "reddit", "client_id": "x"})),
            vec![need("subreddit", None)]
        );
        assert_eq!(
            needs(serde_json::json!({"kind": "jira", "site_url": "https://x.atlassian.net"})),
            vec![
                need("project", Some("issue")),
                need("issuetype", Some("issue"))
            ]
        );
        assert_eq!(
            needs(serde_json::json!({"kind": "discord"})),
            vec![need("channel", Some("thread"))]
        );
        // one webhook is where every post goes; two, and the button has to say which
        let hook = |slot: u8| serde_json::json!({"slot": slot, "name": format!("c{slot}")});
        assert!(needs(
            serde_json::json!({"kind": "discord", "mode": "webhook", "hooks": [hook(0)]})
        )
        .is_empty());
        assert_eq!(
            needs(
                serde_json::json!({"kind": "discord", "mode": "webhook", "hooks": [hook(0), hook(1)]})
            ),
            vec![need("channel", Some("thread"))]
        );
        for open in ["github", "bluesky", "x"] {
            assert!(
                needs(serde_json::json!({"kind": open})).is_empty(),
                "{open}"
            );
        }
    }
}
