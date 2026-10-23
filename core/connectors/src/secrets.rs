//! Where passwords, tokens and keys live: the operating system's credential store in the app,
//! memory in tests. Never a config file, a log or a zip.

use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretError {
    /// There is no credential store to talk to (for example no keyring service on Linux).
    #[error("No secure credential store is available: {0}")]
    Unavailable(String),
    #[error("The credential store refused: {0}")]
    Failed(String),
}

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

/// `<connection id>/<name>`
pub fn key(connection_id: &str, name: &str) -> String {
    format!("{connection_id}/{name}")
}

/// Forget everything stored for a connection.
pub fn delete_all(store: &dyn SecretStore, connection_id: &str, names: &[&str]) {
    for n in names {
        let _ = store.delete(&key(connection_id, n));
    }
}

#[derive(Default)]
pub struct MemorySecrets {
    map: Mutex<HashMap<String, String>>,
}

impl MemorySecrets {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn keys(&self) -> Vec<String> {
        let mut k: Vec<String> = self
            .map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys()
            .cloned()
            .collect();
        k.sort();
        k
    }
}

impl SecretStore for MemorySecrets {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        Ok(self
            .map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(key)
            .cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.map
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_round_trips() {
        let s = MemorySecrets::new();
        assert_eq!(s.get("a/token").unwrap(), None);
        s.set("a/token", "x").unwrap();
        s.set("a/other", "y").unwrap();
        assert_eq!(s.get("a/token").unwrap().as_deref(), Some("x"));
        delete_all(&s, "a", &["token", "missing"]);
        assert_eq!(s.keys(), vec!["a/other".to_string()]);
        assert_eq!(key("jira", "token"), "jira/token");
    }
}
