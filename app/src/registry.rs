//! What the layout checks know about the rest of the configuration.

use crate::connections::Connections;
use cargoframe_config::{InfoType, Kind, Store};
use cargoframe_layout::{ConnectionInfo, Registry};
use cargoframe_window::vars;
use std::collections::BTreeSet;

type ConnectionLookup<'a> = Box<dyn Fn(&str) -> Option<ConnectionInfo> + 'a>;
type ListLookup<'a> = Box<dyn Fn(&str, &str) -> bool + 'a>;

pub struct AppRegistry<'a> {
    pub rules: BTreeSet<String>,
    connection: ConnectionLookup<'a>,
    /// (connection id, field) of every field its service requires.
    required: BTreeSet<(String, String)>,
    /// Does (connection id, list) search for what is typed?
    searches: ListLookup<'a>,
}

impl<'a> AppRegistry<'a> {
    /// The rules on disk, and the connections as `connections` reports them. A connection is
    /// looked at only when a layout asks about it, so a layout that uses none never touches the
    /// credential store.
    pub fn from_store(store: &'a Store, connections: &'a Connections) -> Self {
        let required = store
            .load_all::<InfoType>(Kind::InfoTypes)
            .map(|l| {
                l.items
                    .into_iter()
                    .map(|(_, t)| t)
                    .filter(|t| t.required)
                    .map(|t| (t.connection_id, t.field))
                    .collect()
            })
            .unwrap_or_default();
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
        }
    }
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
    }

    fn list_searches(&self, connection_id: &str, list: &str) -> bool {
        (self.searches)(connection_id, list)
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
}
