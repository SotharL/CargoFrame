//! Connectors: one per destination, all behind the same small interface.
//!
//! A connector knows how to sign in, test itself, fetch dropdown lists and send a share. It
//! uses only what [`Ctx`] gives it (network, credential store, browser), so every one of them is
//! tested against a local mock server with no real service involved.

// The services, each behind its own feature (see Cargo.toml).
#[cfg(feature = "bluesky")]
pub mod bluesky;
#[cfg(feature = "claude")]
pub mod claude;
#[cfg(feature = "custom")]
pub mod custom;
#[cfg(feature = "discord")]
pub mod discord;
#[cfg(feature = "github")]
pub mod github;
#[cfg(feature = "google")]
pub mod google;
#[cfg(feature = "instagram")]
pub mod instagram;
#[cfg(feature = "jira")]
pub mod jira;
#[cfg(feature = "reddit")]
pub mod reddit;
#[cfg(feature = "slack")]
pub mod slack;
#[cfg(feature = "teams")]
pub mod teams;
#[cfg(feature = "x")]
pub mod x;

// What they share.
pub mod ctx;
pub mod error;
#[cfg(any(feature = "x", feature = "bluesky", feature = "instagram"))]
pub mod media;
pub mod model;
pub mod oauth;
pub mod payload;
pub mod richtext;
pub mod secrets;

#[cfg(test)]
mod testkit;

pub use cargoframe_deliver::{Deliver, DeliverError, Delivered, Delivery, Target, Timing};
pub use ctx::{Browser, Ctx};
pub use model::{Connection, Kind, Settings};
pub use payload::{
    media_places, place_media, Attachment, Field, FieldKind, InlineFile, LocalCopies, OptionItem,
    Payload, Progress, SendOutcome, RECORDING_TOKEN, SCREENSHOT_TOKEN,
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
    /// What can be chosen *inside* each choice of this list (the threads of a Discord channel),
    /// so one dropdown can go as deep as the person wants.
    pub inside: Option<InsideSpec>,
}

/// A list of what is inside a choice of another list, and the field that takes the choice.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct InsideSpec {
    /// The list, asked for as `<list>:<the choice it is inside>`.
    pub list: &'static str,
    /// The field of the destination the choice fills.
    pub field: &'static str,
    /// What the person calls it ("Thread or post").
    pub label: &'static str,
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

/// Whether this build has the code for `kind`. A company build can leave services out.
pub fn available(kind: Kind) -> bool {
    match kind {
        Kind::Jira => cfg!(feature = "jira"),
        Kind::Google => cfg!(feature = "google"),
        Kind::Reddit => cfg!(feature = "reddit"),
        Kind::Claude => cfg!(feature = "claude"),
        Kind::Slack => cfg!(feature = "slack"),
        Kind::Discord => cfg!(feature = "discord"),
        Kind::Teams => cfg!(feature = "teams"),
        Kind::Custom => cfg!(feature = "custom"),
        Kind::X => cfg!(feature = "x"),
        Kind::Bluesky => cfg!(feature = "bluesky"),
        Kind::Instagram => cfg!(feature = "instagram"),
        Kind::Github => cfg!(feature = "github"),
    }
}

/// The kinds this build has, in the order the interface lists them.
pub fn available_kinds() -> Vec<Kind> {
    Kind::ALL.into_iter().filter(|k| available(*k)).collect()
}

/// The connector for a saved connection. `None` for a disabled one, or one this build does
/// not have the code for.
pub fn build(connection: &Connection) -> Option<Box<dyn Connector>> {
    if !connection.enabled {
        return None;
    }
    // (with every service left out, every arm but the last is gone)
    #[allow(unreachable_patterns, unreachable_code)]
    Some(match &connection.settings {
        #[cfg(feature = "jira")]
        Settings::Jira(s) => Box::new(jira::JiraConnector::new(s.clone())),
        #[cfg(feature = "reddit")]
        Settings::Reddit(s) => Box::new(reddit::RedditConnector::new(s.clone())),
        #[cfg(feature = "google")]
        Settings::Google(s) => Box::new(google::GoogleConnector::new(s.clone())),
        #[cfg(feature = "claude")]
        Settings::Claude(s) => Box::new(claude::ClaudeConnector::new(s.clone())),
        #[cfg(feature = "slack")]
        Settings::Slack(s) => Box::new(slack::SlackConnector::new(s.clone())),
        #[cfg(feature = "discord")]
        Settings::Discord(s) => Box::new(discord::DiscordConnector::new(s.clone())),
        #[cfg(feature = "teams")]
        Settings::Teams(s) => Box::new(teams::TeamsConnector::new(s.clone())),
        #[cfg(feature = "custom")]
        Settings::Custom(s) => Box::new(custom::CustomConnector::new(s.clone())),
        #[cfg(feature = "x")]
        Settings::X(s) => Box::new(x::XConnector::new(s.clone())),
        #[cfg(feature = "bluesky")]
        Settings::Bluesky(s) => Box::new(bluesky::BlueskyConnector::new(s.clone())),
        #[cfg(feature = "instagram")]
        Settings::Instagram(s) => Box::new(instagram::InstagramConnector::new(s.clone())),
        #[cfg(feature = "github")]
        Settings::Github(s) => Box::new(github::GithubConnector::new(s.clone())),
        _ => return None,
    })
}
