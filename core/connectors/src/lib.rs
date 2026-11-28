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
    Payload, Progress, SendOutcome, Update, RECORDING_TOKEN, SCREENSHOT_TOKEN,
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
    /// The service refuses a share without it: the window names it beside the button, and the
    /// share is held back until it has a value.
    pub required: bool,
    /// Not needed when this other field has a value: a thread link says the channel, an issue
    /// picked to update needs no project.
    pub unless: Option<&'static str>,
}

impl FieldSpec {
    /// A field with no need: what most fields are.
    pub fn new(
        key: &'static str,
        label: &'static str,
        hint: &'static str,
        list: Option<&'static str>,
    ) -> Self {
        Self {
            key,
            label,
            hint,
            list,
            required: false,
            unless: None,
        }
    }

    /// The service needs this field, unless `unless` has a value.
    pub fn required(mut self, unless: Option<&'static str>) -> Self {
        self.required = true;
        self.unless = unless;
        self
    }
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
    /// The service searches this list for what is typed (people, teams): too long to list whole.
    /// It is asked for as `<list>[:<param>]?<typed>`; with nothing typed, the first few.
    pub search: bool,
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

impl Describe {
    /// The labels, hints and summary in the language in use (they are written with `msg!`).
    pub fn translated(mut self) -> Self {
        use cargoframe_i18n::t;
        for f in &mut self.fields {
            f.label = t(f.label);
            f.hint = t(f.hint);
        }
        for l in &mut self.lists {
            l.label = t(&l.label).to_string();
            if let Some(inside) = &mut l.inside {
                inside.label = t(inside.label);
            }
        }
        self.summary = t(&self.summary).to_string();
        self
    }
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
