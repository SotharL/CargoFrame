//! What the layout checks know about the rest of the configuration.

use cargoframe_layout::{ConnectionInfo, Registry};
use cargoframe_window::vars;
use std::collections::BTreeSet;

#[derive(Default)]
pub struct AppRegistry {
    pub rules: BTreeSet<String>,
}

impl Registry for AppRegistry {
    fn connection(&self, _id: &str) -> Option<ConnectionInfo> {
        // Connections arrive with the connectors (Phase 4); until then none exist.
        None
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

    #[test]
    fn built_ins_and_known_rules_resolve() {
        let reg = AppRegistry {
            rules: ["version".to_string()].into(),
        };
        assert!(reg.is_known_variable("app.exe"));
        assert!(reg.is_known_variable("rule.version"));
        assert!(!reg.is_known_variable("rule.other"));
        assert!(!reg.is_known_variable("nope"));
        assert!(reg.rule_exists("version") && !reg.rule_exists("x"));
        assert!(reg.connection("jira").is_none());
    }
}
