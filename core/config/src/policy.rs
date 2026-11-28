//! `policy.json` in the organisation's managed folder: the few things an organisation decides for
//! everyone, which nobody can change in Settings. Every rule is optional; a missing file, or one
//! that cannot be read, decides nothing.
//!
//! ```json
//! { "schema_version": 1,
//!   "services": ["jira", "teams", "custom"],
//!   "check_updates": false,
//!   "debug_mode": false,
//!   "own_connections": false }
//! ```

use crate::store::Document;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[serde(default)]
pub struct Policy {
    /// The services people may use, by kind (`jira`, `teams`, `custom`…). Empty: every service
    /// this copy of CargoFrame has. A saved connection to another one cannot send.
    pub services: Vec<String>,
    /// Look for new versions: always (`true`) or never (`false`). Absent: each person chooses.
    pub check_updates: Option<bool>,
    /// `false`: debug mode cannot be turned on (it attaches settings and logs to reports).
    pub debug_mode: Option<bool>,
    /// `false`: people use the organisation's connections only and cannot add their own.
    pub own_connections: Option<bool>,
}

impl Document for Policy {
    const SCHEMA: u32 = 1;
    const NAME: &'static str = "policy";
}

impl Policy {
    /// May people use the service `kind` (its key, `jira`)?
    pub fn allows_service(&self, kind: &str) -> bool {
        self.services.is_empty() || self.services.iter().any(|s| s.eq_ignore_ascii_case(kind))
    }

    /// May people add connections of their own?
    pub fn allows_own_connections(&self) -> bool {
        self.own_connections != Some(false)
    }

    /// May debug mode be turned on?
    pub fn allows_debug_mode(&self) -> bool {
        self.debug_mode != Some(false)
    }

    /// Does the organisation decide anything at all?
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_decided_until_the_organisation_says() {
        let p = Policy::default();
        assert!(p.is_empty());
        assert!(p.allows_service("jira") && p.allows_own_connections() && p.allows_debug_mode());
        let p: Policy = serde_json::from_str(
            r#"{"schema_version":1,"services":["Jira","teams"],"own_connections":false,"debug_mode":false,"check_updates":false}"#,
        )
        .unwrap();
        assert!(p.allows_service("jira") && p.allows_service("teams"));
        assert!(!p.allows_service("x"));
        assert!(!p.allows_own_connections() && !p.allows_debug_mode());
        assert_eq!(p.check_updates, Some(false));
    }
}
