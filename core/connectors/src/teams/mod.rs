//! Microsoft Teams: post a message, with the screenshot inside it, into a chat, a channel or a
//! channel thread. The person creates an app registration in their own Microsoft Entra ID
//! (free), gives it a few delegated permissions and signs in once through the browser: the
//! message is then posted as that person, through Microsoft Graph.
//!
//! * The sign-in is the shared OAuth code with PKCE and no client secret (a "public client").
//!   Microsoft only accepts `http://localhost` as the return address of a desktop app, and
//!   ignores the port, so that spelling is used here (see `oauth::Redirect`).
//! * The access token is sent to the Graph address only; a "next page" link that points anywhere
//!   else is not followed.
//! * A Graph message cannot be given a key that makes a repeat harmless, so a post whose outcome
//!   is unknown (the answer was lost, or Graph failed on its side) is never tried again: the
//!   person is told to look in Teams, and a message cannot be written twice.
//! * The screenshot travels inside the message (`hostedContents`). The ticked files would need a
//!   OneDrive upload, which CargoFrame does not do; they are left out and the person is told.
//! * Text is escaped: it can never become markup or an @-mention.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::model;
use crate::oauth::{self, ClientAuth, OAuthClient, Redirect};
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::richtext::{render, Style};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cargoframe_net::{check_url, encode_component, Body, Method, Request, Response};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};
use std::time::Duration;

/// What the app registration needs (all delegated, none of them an "application" permission).
pub const SCOPES: [&str; 7] = [
    "offline_access",
    "User.Read",
    "Chat.ReadBasic",
    "ChatMessage.Send",
    "ChannelMessage.Send",
    "Team.ReadBasic.All",
    "Channel.ReadBasic.All",
];
/// Pages of chats read at most (50 each), so a huge account cannot hang the window.
const MAX_CHAT_PAGES: usize = 3;
/// Teams read at most for the channel list.
const MAX_TEAMS: usize = 40;
/// Characters of message text sent; a Teams message may not be much bigger than 28 KB.
pub const MAX_TEXT_CHARS: usize = 12_000;

#[derive(Debug, Clone)]
pub struct Endpoints {
    /// `https://login.microsoftonline.com`
    pub login: String,
    /// `https://graph.microsoft.com/v1.0`
    pub graph: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            login: "https://login.microsoftonline.com".into(),
            graph: "https://graph.microsoft.com/v1.0".into(),
        }
    }
}

pub struct TeamsConnector {
    settings: model::Teams,
    ep: Endpoints,
    wait: Duration,
}

impl TeamsConnector {
    pub fn new(settings: model::Teams) -> Self {
        Self {
            settings,
            ep: Endpoints::default(),
            wait: Duration::from_secs(180),
        }
    }

    pub fn with_endpoints(mut self, ep: Endpoints, wait: Duration) -> Self {
        self.ep = Endpoints {
            login: ep.login.trim_end_matches('/').to_string(),
            graph: ep.graph.trim_end_matches('/').to_string(),
        };
        self.wait = wait;
        self
    }

    fn tenant(&self) -> String {
        let t = self.settings.tenant.trim();
        if t.is_empty() {
            "organizations".into()
        } else {
            t.to_string()
        }
    }

    fn client(&self) -> OAuthClient {
        let tenant = self.tenant();
        OAuthClient {
            auth_url: format!("{}/{tenant}/oauth2/v2.0/authorize", self.ep.login),
            token_url: format!("{}/{tenant}/oauth2/v2.0/token", self.ep.login),
            // Microsoft has no address that revokes a refresh token; signing out forgets it here.
            revoke_url: None,
            client_id: self.settings.client_id.trim().to_string(),
            client_secret: None,
            scopes: SCOPES.iter().map(|s| s.to_string()).collect(),
            extra_auth_params: Vec::new(),
            pkce: true,
            client_auth: ClientAuth::Body,
            redirect_port: None,
            redirect: Redirect::Localhost,
            wait: self.wait,
        }
    }

    fn request(&self, ctx: &Ctx, method: Method, url: String) -> Result<Request, SendError> {
        let token = oauth::access_token(&self.client(), ctx)?;
        Ok(Request::new(method, url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .timeout(ctx.timeout))
    }

    fn graph(&self, path: &str) -> String {
        format!("{}{path}", self.ep.graph)
    }

    fn call(&self, ctx: &Ctx, req: Request, what: &str) -> Result<Value, SendError> {
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(graph_error(&resp, what));
        }
        resp.json().ok_or_else(|| {
            invalid(format!(
                "Microsoft answered {what} with something unreadable"
            ))
        })
    }

    fn get(&self, ctx: &Ctx, url: String, what: &str) -> Result<Value, SendError> {
        self.call(ctx, self.request(ctx, Method::Get, url)?, what)
    }

    /// Is `link` on the Graph address? Only then may the token be sent to it.
    fn is_graph(&self, link: &str) -> bool {
        link.strip_prefix(&self.ep.graph)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?']))
    }

    fn me(&self, ctx: &Ctx) -> Result<Value, SendError> {
        self.get(
            ctx,
            self.graph("/me?$select=id,displayName,userPrincipalName"),
            "check the sign-in",
        )
    }

    /// The chats of the person, named by their topic or by who is in them.
    fn chats(&self, ctx: &Ctx) -> Result<Vec<OptionItem>, SendError> {
        let my_id = self
            .me(ctx)?
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let mut out = Vec::new();
        let mut next = Some(self.graph("/me/chats?$expand=members&$top=50"));
        for _ in 0..MAX_CHAT_PAGES {
            let Some(url) = next.take() else { break };
            let v = self.get(ctx, url, "list the chats")?;
            for chat in v
                .get("value")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(id) = chat.get("id").and_then(Value::as_str) else {
                    continue;
                };
                let (group, fallback) = match chat.get("chatType").and_then(Value::as_str) {
                    Some("oneOnOne") => ("Direct chats", "Chat"),
                    Some("group") => ("Group chats", "Group chat"),
                    Some("meeting") => ("Meeting chats", "Meeting chat"),
                    _ => ("Other chats", "Chat"),
                };
                let topic = chat
                    .get("topic")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|t| !t.is_empty());
                let label = match topic {
                    Some(t) => t.to_string(),
                    None => {
                        let names = member_names(chat, &my_id);
                        if names.is_empty() {
                            fallback.to_string()
                        } else {
                            names
                        }
                    }
                };
                let mut item = OptionItem::new(format!("chat:{id}"), label);
                item.group = Some(group.to_string());
                out.push(item);
            }
            next = v
                .get("@odata.nextLink")
                .and_then(Value::as_str)
                .filter(|l| self.is_graph(l))
                .map(String::from);
        }
        out.sort_by(|a, b| (&a.group, &a.label).cmp(&(&b.group, &b.label)));
        Ok(out)
    }

    /// The channels of every team the person has joined.
    fn channels(&self, ctx: &Ctx) -> Result<Vec<OptionItem>, SendError> {
        let teams = self.get(ctx, self.graph("/me/joinedTeams"), "list the teams")?;
        let mut teams: Vec<(String, String)> = teams
            .get("value")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|t| {
                Some((
                    t.get("id")?.as_str()?.to_string(),
                    t.get("displayName")?.as_str()?.to_string(),
                ))
            })
            .collect();
        teams.sort_by(|a, b| a.1.cmp(&b.1));
        teams.truncate(MAX_TEAMS);
        let mut out = Vec::new();
        let mut first_error = None;
        for (team_id, team_name) in &teams {
            let v = match self.get(
                ctx,
                self.graph(&format!("/teams/{}/channels", encode_component(team_id))),
                "list the channels",
            ) {
                Ok(v) => v,
                Err(e) => {
                    first_error.get_or_insert(e);
                    continue;
                }
            };
            let mut channels: Vec<(String, String, &str)> = v
                .get("value")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|c| {
                    Some((
                        c.get("id")?.as_str()?.to_string(),
                        c.get("displayName")?.as_str()?.to_string(),
                        c.get("membershipType")
                            .and_then(Value::as_str)
                            .unwrap_or("standard"),
                    ))
                })
                // A channel shared from another organisation has to be reached through that one.
                .filter(|(_, _, kind)| *kind != "shared")
                .collect();
            channels.sort_by(|a, b| a.1.cmp(&b.1));
            for (id, name, kind) in channels {
                let label = if kind == "private" {
                    format!("#{name} (private)")
                } else {
                    format!("#{name}")
                };
                let mut item = OptionItem::new(format!("channel:{team_id}/{id}"), label);
                item.group = Some(format!("Team: {team_name}"));
                out.push(item);
            }
        }
        match (out.is_empty(), first_error) {
            (true, Some(e)) => Err(e),
            _ => Ok(out),
        }
    }

    /// Post, and never post twice: see the module notes.
    fn post(&self, ctx: &Ctx, path: &str, body: &Value) -> Result<Value, SendError> {
        let req = self
            .request(ctx, Method::Post, self.graph(path))?
            .body(Body::json(body));
        let maybe = |mut e: SendError| {
            e.kind = FailureKind::Rejected(0);
            e.message = format!(
                "{}. Teams may or may not have posted the message, so CargoFrame did not try again: look in Teams",
                e.message.trim_end_matches('.')
            );
            e
        };
        let resp = ctx.http.send(&req).map_err(|e| maybe(from_net(&e)))?;
        if resp.is_success() {
            return Ok(resp.json().unwrap_or(Value::Null));
        }
        let err = graph_error(&resp, "post the message");
        Err(match err.kind {
            FailureKind::Server(_) | FailureKind::Timeout | FailureKind::NoNetwork => maybe(err),
            _ => err,
        })
    }
}

/// Names of the people in a chat other than the person themselves, a few at most.
fn member_names(chat: &Value, my_id: &str) -> String {
    let names: Vec<&str> = chat
        .get("members")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|m| m.get("userId").and_then(Value::as_str) != Some(my_id))
        .filter_map(|m| m.get("displayName").and_then(Value::as_str))
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    match names.len() {
        0 => String::new(),
        1..=4 => names.join(", "),
        n => format!("{}, +{} more", names[..3].join(", "), n - 3),
    }
}

/// What Graph says when it refuses, in words for the person.
fn graph_error(resp: &Response, what: &str) -> SendError {
    let mut e = from_response(resp, what);
    let hint = match resp.status {
        401 => {
            e.message = format!(
                "Could not {what}: the Microsoft sign-in is not valid any more. Open Connections and press Sign in"
            );
            return e;
        }
        403 => " Either you are not a member of that chat or channel, your organisation does not allow this app, or a permission is missing: press Sign in again and accept every permission it asks for",
        404 => " Teams cannot find that chat or channel: choose it again from the list",
        413 => " The screenshot may be too large for Teams",
        _ => return e,
    };
    e.message = format!("{}.{hint}", e.message.trim_end_matches('.'));
    e
}

// ----- where a message goes -----

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Chat(String),
    Channel { team: String, channel: String },
}

fn is_chat_id(s: &str) -> bool {
    s.starts_with("19:")
        && s.len() <= 256
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '@' | '.' | '_' | '-'))
}

fn is_channel_id(s: &str) -> bool {
    is_chat_id(s) && s.contains("@thread")
}

/// `chat:<id>`, `channel:<team>/<channel>`, or a bare chat id.
pub fn parse_target(raw: &str) -> Result<Target, String> {
    let raw = raw.trim();
    let bad = || {
        format!(
            "\"{raw}\" is not a Teams chat or channel. Choose one from the list on the sharing button"
        )
    };
    if let Some(id) = raw.strip_prefix("chat:") {
        return if is_chat_id(id) {
            Ok(Target::Chat(id.to_string()))
        } else {
            Err(bad())
        };
    }
    if let Some(rest) = raw.strip_prefix("channel:") {
        let (team, channel) = rest.split_once('/').ok_or_else(bad)?;
        return if model::is_guid(team) && is_channel_id(channel) {
            Ok(Target::Channel {
                team: team.to_string(),
                channel: channel.to_string(),
            })
        } else {
            Err(bad())
        };
    }
    if is_chat_id(raw) && !raw.contains("@thread.tacv") {
        return Ok(Target::Chat(raw.to_string()));
    }
    Err(bad())
}

/// A message in a channel, from "Copy link" in Teams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRef {
    pub team: String,
    pub channel: String,
    /// The first message of the thread (a reply is posted under it).
    pub message: String,
}

pub fn parse_thread(raw: &str) -> Result<ThreadRef, String> {
    let bad = |why: &str| {
        format!(
            "\"{}\" is not a link to a message in a Teams channel ({why}). In Teams, open the \"…\" menu of the first message of the thread and choose Copy link",
            raw.trim()
        )
    };
    let rest = raw
        .trim()
        .strip_prefix("https://")
        .ok_or_else(|| bad("it must start with https://"))?;
    let (host, after) = rest
        .split_once('/')
        .ok_or_else(|| bad("there is no path"))?;
    if !matches!(
        host.to_ascii_lowercase().as_str(),
        "teams.microsoft.com" | "teams.cloud.microsoft"
    ) {
        return Err(bad("it is not a Teams address"));
    }
    let (path, query) = after.split_once('?').unwrap_or((after, ""));
    let query = query.split('#').next().unwrap_or("");
    let segments: Vec<&str> = path.split('/').collect();
    if segments.len() < 4 || segments[0] != "l" || segments[1] != "message" {
        return Err(bad("it is not a message link"));
    }
    let channel = oauth::decode(segments[2]);
    if !is_channel_id(&channel) {
        return Err(bad("it is a chat, not a channel"));
    }
    let pairs = oauth::query_pairs(query);
    let get = |k: &str| {
        pairs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_str())
    };
    let team = get("groupId")
        .filter(|g| model::is_guid(g))
        .ok_or_else(|| bad("it names no team"))?;
    let message = get("parentMessageId")
        .filter(|m| !m.is_empty())
        .unwrap_or(segments[3]);
    if message.is_empty() || !message.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad("it names no message"));
    }
    Ok(ThreadRef {
        team: team.to_string(),
        channel,
        message: message.to_string(),
    })
}

// ----- the text -----

pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Composed {
    pub html: String,
    /// The headline of a new channel post, when there is one.
    pub subject: Option<String>,
    pub shortened: bool,
}

/// The message as Teams wants it: HTML with every piece of text escaped. `subject` says whether
/// a title can be the headline of the post (a new channel post) or must be a bold first line.
pub fn compose(payload: &Payload, with_image: bool, subject: bool) -> Composed {
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let title = payload
        .value("title")
        .map(one_line)
        .filter(|t| !t.is_empty());
    let body = payload
        .value("message")
        .map(str::to_string)
        .or_else(|| {
            payload
                .prompt
                .as_deref()
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
        })
        .or_else(|| {
            Some(payload.title.trim().to_string())
                .filter(|t| !t.is_empty() && payload.value("title").is_none())
        });
    let mut html = String::new();
    let mut headline = None;
    if let Some(t) = title {
        if subject {
            headline = Some(t);
        } else {
            html.push_str(&format!("<p><b>{}</b></p>", escape(&t)));
        }
    }
    let mut shortened = false;
    if let Some(b) = body {
        let b = if b.chars().count() > MAX_TEXT_CHARS {
            shortened = true;
            format!("{}…", b.chars().take(MAX_TEXT_CHARS).collect::<String>())
        } else {
            b
        };
        // bold, italic, underline, code, quotes and lists (see `richtext`); every text is escaped
        html.push_str(&render(&b, Style::Html));
    }
    // the screenshot goes where the text puts it, or at the end; a recording cannot be shown
    let img = "<img src=\"../hostedContents/1/$value\" alt=\"Screenshot\">";
    let (placed, _) = crate::payload::media_places(&html);
    html = crate::payload::place_media(&html, if with_image { img } else { "" }, "");
    if with_image && !placed {
        html.push_str(&format!("<p>{img}</p>"));
    }
    Composed {
        html,
        subject: headline,
        shortened,
    }
}

impl Connector for TeamsConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint, list| FieldSpec {
            key,
            label,
            hint,
            list,
        };
        Describe {
            fields: vec![
                f("target", "Chat or channel", "Where to post. You must be a member of it", Some("targets")),
                f("thread", "Reply in a channel thread", "A message link (Copy link in Teams) in a channel. Empty: a new message", None),
                f("message", "Message", "Defaults to the prompt text, then the window title", None),
                f("title", "Headline", "Empty: none. The headline of a new channel post, or a bold first line in a chat or a reply", None),
            ],
            lists: vec![ListSpec {
                id: "targets".into(),
                label: "Your chats and the channels of your teams".into(),
                param: None,
                inside: None,
            }],
            summary: "Posts a message as you into a chat, a channel or a channel thread, with the screenshot inside it. The ticked files cannot be sent yet: Teams needs them in OneDrive.".into(),
        }
    }

    fn status(&self, ctx: &Ctx) -> Status {
        if oauth::is_signed_in(ctx) {
            Status::Connected
        } else {
            Status::Disconnected
        }
    }

    fn connect(&self, ctx: &Ctx) -> Result<String, SendError> {
        if self.settings.client_id.trim().is_empty() {
            return Err(not_set_up(
                "Teams needs the Application (client) ID of your Microsoft app: open Connections, paste it and press Save",
            ));
        }
        oauth::authorize(&self.client(), ctx)?;
        self.test(ctx)
    }

    fn disconnect(&self, ctx: &Ctx) {
        oauth::sign_out(&self.client(), ctx);
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let v = self.me(ctx)?;
        let text = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
        Ok(match (text("displayName"), text("userPrincipalName")) {
            (Some(name), Some(upn)) => format!("Signed in to Microsoft as {name} ({upn})"),
            (Some(who), None) | (None, Some(who)) => format!("Signed in to Microsoft as {who}"),
            _ => "Signed in to Microsoft".to_string(),
        })
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        if list != "targets" {
            return Err(invalid(format!("Teams has no list called \"{list}\"")));
        }
        let channels = self.channels(ctx);
        let chats = self.chats(ctx);
        match (channels, chats) {
            (Err(e), Err(_)) => Err(e),
            (channels, chats) => {
                // One of them failing (a permission the organisation withholds) does not hide
                // the other.
                let mut out = channels.unwrap_or_default();
                out.extend(chats.unwrap_or_default());
                Ok(out)
            }
        }
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let thread = payload
            .value("thread")
            .map(parse_thread)
            .transpose()
            .map_err(invalid)?;
        let target = payload
            .value("target")
            .map(parse_target)
            .transpose()
            .map_err(invalid)?;

        // (path to post to, a new channel post that may carry a headline, in a thread)
        let (path, new_channel_post, reply) = match (&thread, &target) {
            (Some(t), None) => (reply_path(t), false, true),
            (
                Some(t),
                Some(Target::Channel { team, channel }),
            ) if *team == t.team && *channel == t.channel => (reply_path(t), false, true),
            (Some(_), Some(Target::Channel { .. })) => {
                return Err(invalid(
                    "The thread link is in a different channel than the one chosen",
                ))
            }
            (Some(_), Some(Target::Chat(_))) => {
                return Err(invalid(
                    "A thread link works in a channel; a chat has no threads. Clear one of the two",
                ))
            }
            (None, Some(Target::Chat(id))) => {
                (format!("/chats/{}/messages", encode_component(id)), false, false)
            }
            (None, Some(Target::Channel { team, channel })) => (
                format!(
                    "/teams/{}/channels/{}/messages",
                    encode_component(team),
                    encode_component(channel)
                ),
                true,
                false,
            ),
            (None, None) => {
                return Err(invalid(
                    "Teams needs a chat or a channel: map one to the field \"target\" on the sharing button",
                ))
            }
        };

        let composed = compose(payload, payload.image.is_some(), new_channel_post);
        if composed.html.is_empty() {
            return Err(invalid(
                "There is nothing to send to Teams: no text and no screenshot",
            ));
        }
        let mut body = json!({
            "body": { "contentType": "html", "content": composed.html },
        });
        if let Some(subject) = &composed.subject {
            body["subject"] = json!(subject);
        }
        if let Some(image) = &payload.image {
            body["hostedContents"] = json!([{
                "@microsoft.graph.temporaryId": "1",
                "contentBytes": STANDARD.encode(&image.data),
                "contentType": image.mime,
            }]);
        }

        let posted = self.post(ctx, &path, &body)?;
        let url = posted
            .get("webUrl")
            .and_then(Value::as_str)
            .filter(|u| check_url(u).is_ok())
            .map(String::from);

        let mut notes = Vec::new();
        if payload.files_zip.is_some() {
            notes.push(
                "The ticked files were not sent: Teams needs them in OneDrive, which CargoFrame does not use yet"
                    .to_string(),
            );
        }
        if composed.shortened {
            notes.push("The message was shortened to fit Teams' size limit".to_string());
        }
        Ok(SendOutcome {
            summary: if reply {
                "Posted in the Teams thread".to_string()
            } else {
                "Posted to Teams".to_string()
            },
            url,
            notes,
            place: None,
        })
    }
}

fn reply_path(t: &ThreadRef) -> String {
    format!(
        "/teams/{}/channels/{}/messages/{}/replies",
        encode_component(&t.team),
        encode_component(&t.channel),
        encode_component(&t.message)
    )
}

#[cfg(test)]
mod tests;
