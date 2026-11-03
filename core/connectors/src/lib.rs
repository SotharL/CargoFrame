//! Connectors: one per destination, all behind the same small interface.
//!
//! A connector knows how to sign in, test itself, fetch dropdown lists and send a share. It
//! uses only what [`Ctx`] gives it (network, credential store, browser), so every one of them is
//! tested against a local mock server with no real service involved.

pub mod claude;
pub mod ctx;
pub mod custom;
pub mod discord;
pub mod error;
pub mod google;
pub mod jira;
pub mod model;
pub mod oauth;
pub mod payload;
pub mod reddit;
pub mod secrets;
pub mod slack;
pub mod teams;

#[cfg(test)]
mod testkit;

pub use cargoframe_deliver::{Deliver, DeliverError, Delivered, Delivery, Target, Timing};
pub use ctx::{Browser, Ctx};
pub use model::{Connection, Kind, Settings};
pub use payload::{
    Attachment, Field, FieldKind, InlineFile, LocalCopies, OptionItem, Payload, Progress,
    SendOutcome,
};
pub use secrets::{MemorySecrets, SecretError, SecretStore};

use cargoframe_safe::SendError;
use serde::Serialize;

/// A field of the destination service a sharing button can fill.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct FieldSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    /// The dropdown list that suits this field, if any.
    pub list: Option<&'static str>,
}

/// A list the service can supply (for a dropdown block or a choice info type).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ListSpec {
    pub id: String,
    pub label: String,
    /// The list needs a value after the colon (`components:PROJ`), such as a project key.
    pub param: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Default)]
pub struct Describe {
    pub fields: Vec<FieldSpec>,
    pub lists: Vec<ListSpec>,
    /// A sentence about how a share to this destination behaves.
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Everything needed is stored.
    Connected,
    /// A password, key or sign-in is missing.
    Disconnected,
}

pub trait Connector {
    fn describe(&self) -> Describe;

    /// Whether the credentials needed are stored. Looks only at this computer.
    fn status(&self, ctx: &Ctx) -> Status;

    /// Sign in (OAuth connectors open the browser). Returns a line for the person.
    fn connect(&self, ctx: &Ctx) -> Result<String, SendError> {
        self.test(ctx)
    }

    /// Forget the stored sign-in.
    fn disconnect(&self, ctx: &Ctx);

    /// Ask the service who we are. Returns a line for the person ("Signed in as ...").
    fn test(&self, ctx: &Ctx) -> Result<String, SendError>;

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError>;

    /// Deliver the share. Must be safe to call again after a failure: what an earlier attempt
    /// already did is recorded in `ctx.progress`.
    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError>;
}

/// The connector for a saved connection. `None` for a disabled one.
pub fn build(connection: &Connection) -> Option<Box<dyn Connector>> {
    if !connection.enabled {
        return None;
    }
    Some(match &connection.settings {
        Settings::Jira(s) => Box::new(jira::JiraConnector::new(s.clone())),
        Settings::Reddit(s) => Box::new(reddit::RedditConnector::new(s.clone())),
        Settings::Google(s) => Box::new(google::GoogleConnector::new(s.clone())),
        Settings::Claude(s) => Box::new(claude::ClaudeConnector::new(s.clone())),
        Settings::Slack(s) => Box::new(slack::SlackConnector::new(s.clone())),
        Settings::Discord(s) => Box::new(discord::DiscordConnector::new(s.clone())),
        Settings::Teams(s) => Box::new(teams::TeamsConnector::new(s.clone())),
        Settings::Custom(s) => Box::new(custom::CustomConnector::new(s.clone())),
    })
}
