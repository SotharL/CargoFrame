//! What the layout checks know about the rest of the configuration.

use crate::connections::Connections;
use cargoframe_config::{Kind, Store};
use cargoframe_layout::{ConnectionInfo, Registry};
use cargoframe_window::vars;
use std::collections::BTreeSet;

pub struct AppRegistry<'a> {
    pub rules: BTreeSet<String>,
    connection: Box<dyn Fn(&str) -> Option<ConnectionInfo> + 'a>,
}

impl<'a> AppRegistry<'a> {
    /// The rules on disk, and the connections as `connections` reports them. A connection is
    /// looked at only when a layout asks about it, so a layout that uses none never touches the
    /// credential store.
    pub fn from_store(store: &'a Store, connections: &'a Connections) -> Self {
        Self::with(
            store
                .list_ids(Kind::Rules)
                .unwrap_or_default()
                .into_iter()
                .collect(),
            move |id| connections.info(store, id),
        )
    }

    pub fn with(
        rules: BTreeSet<String>,
        connection: impl Fn(&str) -> Option<ConnectionInfo> + 'a,
    ) -> Self {
        Self {
            rules,
            connection: Box::new(connection),
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
            || name
                .strip_prefix("rule.")
                .is_some_and(|id| self.rules.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargoframe_layout::{ConnectionKind, ConnectionState};

    #[test]
    fn built_ins_and_known_rules_resolve() {
        let reg = AppRegistry::with(["version".to_string()].into(), |id| {
            (id == "jira").then_some(ConnectionInfo {
                kind: ConnectionKind::Jira,
                state: ConnectionState::Connected,
            })
        });
        assert!(reg.is_known_variable("app.exe"));
        assert!(reg.is_known_variable("rule.version"));
        assert!(!reg.is_known_variable("rule.other"));
        assert!(!reg.is_known_variable("nope"));
        assert!(reg.rule_exists("version") && !reg.rule_exists("x"));
        assert!(reg.connection("jira").is_some());
        assert!(reg.connection("other").is_none());
    }
}
