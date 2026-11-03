//! What a connector may use while it works. Everything with a side effect is behind a trait so
//! tests can stand in for the network, the credential store and the browser.

use crate::payload::Progress;
use crate::secrets::SecretStore;
use cargoframe_deliver::Deliver;
use cargoframe_net::Http;
use std::time::Duration;

/// Opens a sign-in page in the person's browser.
pub trait Browser: Send + Sync {
    fn open(&self, url: &str) -> Result<(), String>;
}

pub struct Ctx<'a> {
    pub connection_id: &'a str,
    pub http: &'a dyn Http,
    pub secrets: &'a dyn SecretStore,
    pub browser: &'a dyn Browser,
    /// Pastes a capture into another program's window (a Claude chat).
    pub deliver: &'a dyn Deliver,
    pub progress: &'a Progress,
    /// Time allowed for one request (the safe settings' timeout).
    pub timeout: Duration,
    /// Seconds since 1970; injected so token expiry can be tested.
    pub now: &'a dyn Fn() -> u64,
    /// True once the person gave up (closed the window): stops waiting for a sign-in.
    pub cancelled: &'a dyn Fn() -> bool,
}

impl Ctx<'_> {
    pub fn secret(&self, name: &str) -> Result<Option<String>, cargoframe_safe::SendError> {
        self.secrets
            .get(&crate::secrets::key(self.connection_id, name))
            .map_err(|e| crate::error::not_set_up(e.to_string()))
    }

    pub fn set_secret(&self, name: &str, value: &str) -> Result<(), cargoframe_safe::SendError> {
        self.secrets
            .set(&crate::secrets::key(self.connection_id, name), value)
            .map_err(|e| crate::error::not_set_up(e.to_string()))
    }
}
