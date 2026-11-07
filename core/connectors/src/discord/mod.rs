//! Discord: post a message into a text channel or a thread, or start a post in a forum channel,
//! with the screenshot and a zip of the ticked files attached. Two ways in:
//!
//! - **Webhook** (the easy one, no bot): the person copies a webhook link from a channel's
//!   settings and pastes it once, one link for each channel they post to. A webhook cannot list
//!   servers, channels or threads, so the person names each channel. Posts can carry any name.
//! - **Bot**: the person creates a bot in the Developer Portal, adds it to their servers and
//!   pastes its token. A bot can list every server, category, channel and thread it can see.
//!
//! Posting as a personal account is not offered: Discord's terms forbid automating one.
//!
//! Links and tokens are kept in the credential store and sent to Discord's API address only.
//!
//! Nothing the bot posts can ping anyone: every message says `allowed_mentions: {parse: []}`, so
//! a window title or a data rule that contains `@everyone` stays text.
//!
//! A message carries a `nonce` and `enforce_nonce`, so if the answer is lost and the share is tried
//! again, Discord hands back the first message instead of posting a second one. A forum post
//! cannot be protected that way: if its outcome is unknown it is not retried, so a post can never
//! appear twice.

use crate::ctx::Ctx;
use crate::error::{from_net, invalid, not_set_up};
use crate::model::{self, DiscordMode};
use crate::payload::{Attachment, OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use cargoframe_net::{Body, Method, Multipart, Request, Response};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};
use std::time::Duration;

const TOKEN: &str = "token";
/// Servers listed at most, so a bot in a huge number of servers cannot hang the window.
const MAX_GUILDS: usize = 25;
/// Discord's limit for the text of one message.
const MAX_CONTENT: usize = 2000;
/// And for the name of a forum post.
const MAX_POST_NAME: usize = 100;
const USER_AGENT: &str = "DiscordBot (https://github.com/SotharL/CargoFrame, 0.1)";

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub api: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            api: "https://discord.com/api/v10".into(),
        }
    }
}

pub struct DiscordConnector {
    ep: Endpoints,
    settings: model::Discord,
}

/// What kind of place an id is, as far as posting goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// A text or announcement channel, or a thread: a message is posted in it.
    Talk,
    /// A forum channel: posting starts a new post.
    Forum,
}

struct Target {
    id: String,
    place: Place,
    guild: Option<String>,
}

struct Guild {
    id: String,
    name: String,
}

impl DiscordConnector {
    pub fn new(settings: model::Discord) -> Self {
        Self {
            ep: Endpoints::default(),
            settings,
        }
    }

    fn webhooks(&self) -> bool {
        self.settings.mode == DiscordMode::Webhook
    }

    pub fn with_endpoints(mut self, ep: Endpoints) -> Self {
        self.ep = Endpoints {
            api: ep.api.trim_end_matches('/').to_string(),
        };
        self
    }

    fn token(&self, ctx: &Ctx) -> Result<String, SendError> {
        ctx.secret(TOKEN)?
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                not_set_up(
                    "Discord has no bot token yet: open Connections, paste it and press Save",
                )
            })
    }

    fn request(&self, ctx: &Ctx, method: Method, path: &str) -> Result<Request, SendError> {
        Ok(Request::new(method, format!("{}{path}", self.ep.api))
            .header("Authorization", format!("Bot {}", self.token(ctx)?))
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .timeout(ctx.timeout))
    }

    fn call(&self, ctx: &Ctx, req: Request, what: &str) -> Result<Value, SendError> {
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if resp.is_success() {
            Ok(resp.json().unwrap_or(Value::Null))
        } else {
            Err(discord_error(&resp, what))
        }
    }

    fn get(&self, ctx: &Ctx, path: &str, what: &str) -> Result<Value, SendError> {
        self.call(ctx, self.request(ctx, Method::Get, path)?, what)
    }

    fn guilds(&self, ctx: &Ctx) -> Result<Vec<Guild>, SendError> {
        let v = self.get(ctx, "/users/@me/guilds?limit=100", "list the servers")?;
        Ok(v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|g| {
                Some(Guild {
                    id: g.get("id")?.as_str()?.to_string(),
                    name: g.get("name")?.as_str()?.to_string(),
                })
            })
            .take(MAX_GUILDS)
            .collect())
    }

    /// Something a failure in one server must not stop the list for the others.
    fn skippable(e: &SendError) -> bool {
        !matches!(
            e.kind,
            FailureKind::AuthExpired
                | FailureKind::NoNetwork
                | FailureKind::Timeout
                | FailureKind::Certificate
                | FailureKind::ConnectorUnavailable
        )
    }

    /// Where to post, from what the person picked, pasted or typed.
    fn resolve(&self, ctx: &Ctx, raw: &str) -> Result<Target, SendError> {
        let raw = raw.trim();
        let id = if let Some(name) = raw.strip_prefix('#') {
            self.channel_named(ctx, name.trim())?
        } else {
            parse_place(raw).map_err(invalid)?
        };
        let info = self.get(ctx, &format!("/channels/{id}"), "look at the channel")?;
        let kind = info.get("type").and_then(Value::as_i64).unwrap_or(-1);
        let place = match kind {
            0 | 5 | 10 | 11 | 12 => Place::Talk,
            15 => Place::Forum,
            2 | 13 => {
                return Err(invalid(
                    "That is a voice or stage channel: choose a text channel, a thread or a forum",
                ))
            }
            4 => {
                return Err(invalid(
                    "That is a category, not a channel: choose a channel inside it",
                ))
            }
            16 => {
                return Err(invalid(
                    "Media channels are not supported: choose a text channel, a thread or a forum",
                ))
            }
            _ => {
                return Err(invalid(
                    "That is not a place the bot can post in: choose a text channel, a thread or a forum",
                ))
            }
        };
        Ok(Target {
            id,
            place,
            guild: info
                .get("guild_id")
                .and_then(Value::as_str)
                .map(String::from),
        })
    }

    /// `#name` to an id, through the channels the bot can see.
    fn channel_named(&self, ctx: &Ctx, name: &str) -> Result<String, SendError> {
        let wanted = name.to_lowercase();
        if wanted.is_empty() {
            return Err(invalid("The Discord channel is empty"));
        }
        let found: Vec<_> = self
            .all_channels(ctx)?
            .into_iter()
            .filter(|c| c.name == wanted)
            .collect();
        match found.as_slice() {
            [one] => Ok(one.id.clone()),
            [] => Err(invalid(format!(
                "Discord has no channel called #{wanted} that the bot can see. Add the bot to the server and let it view the channel"
            ))),
            _ => Err(invalid(format!(
                "Several servers have a channel called #{wanted}: pick it from the list instead"
            ))),
        }
    }

    fn all_channels(&self, ctx: &Ctx) -> Result<Vec<ChannelInfo>, SendError> {
        let guilds = self.guilds(ctx)?;
        self.channels_in(ctx, &guilds)
    }

    /// The text channels and forums of these servers, each with the category it sits in and the
    /// place Discord shows it (uncategorised channels come first, like in Discord).
    fn channels_in(&self, ctx: &Ctx, guilds: &[Guild]) -> Result<Vec<ChannelInfo>, SendError> {
        let mut out = Vec::new();
        for g in guilds {
            let list = match self.get(
                ctx,
                &format!("/guilds/{}/channels", g.id),
                "list the channels",
            ) {
                Ok(v) => v,
                Err(e) if Self::skippable(&e) => continue,
                Err(e) => return Err(e),
            };
            let all: Vec<&Value> = list.as_array().into_iter().flatten().collect();
            let text_of =
                |c: &Value, key: &str| c.get(key).and_then(Value::as_str).map(String::from);
            let number = |c: &Value, key: &str| c.get(key).and_then(Value::as_i64).unwrap_or(0);
            let kind_of = |c: &Value| c.get("type").and_then(Value::as_i64).unwrap_or(-1);
            let categories: std::collections::BTreeMap<String, (String, i64)> = all
                .iter()
                .filter(|c| kind_of(c) == 4)
                .filter_map(|c| {
                    Some((
                        text_of(c, "id")?,
                        (text_of(c, "name")?, number(c, "position")),
                    ))
                })
                .collect();
            for c in &all {
                let kind = kind_of(c);
                if !matches!(kind, 0 | 5 | 15) {
                    continue;
                }
                let (Some(id), Some(name)) = (text_of(c, "id"), text_of(c, "name")) else {
                    continue;
                };
                let parent = text_of(c, "parent_id").and_then(|p| categories.get(&p));
                out.push(ChannelInfo {
                    id,
                    name: name.to_lowercase(),
                    forum: kind == 15,
                    guild: g.name.clone(),
                    category: parent.map(|(n, _)| n.clone()),
                    order: (parent.map_or(-1, |(_, pos)| *pos), number(c, "position")),
                });
            }
        }
        Ok(out)
    }

    fn post_message(
        &self,
        ctx: &Ctx,
        channel: &str,
        content: &str,
        files: &[&Attachment],
        nonce: &str,
    ) -> Result<Value, SendError> {
        let path = format!("/channels/{channel}/messages");
        let mut body = json!({
            "content": content,
            "allowed_mentions": { "parse": [] },
            "nonce": nonce,
            "enforce_nonce": true,
        });
        let req = self.request(ctx, Method::Post, &path)?;
        let req = if files.is_empty() {
            req.body(Body::json(&body))
        } else {
            body["attachments"] = attachments_json(files);
            req.body(multipart(&body, files))
        };
        self.call(ctx, req, "post the message")
    }

    /// A new post in a forum channel. Its outcome is never guessed: if the answer is lost, or
    /// Discord fails on its side, the share is not tried again.
    fn start_post(
        &self,
        ctx: &Ctx,
        forum: &str,
        name: &str,
        content: &str,
        files: &[&Attachment],
    ) -> Result<Value, SendError> {
        let mut message = json!({
            "content": content,
            "allowed_mentions": { "parse": [] },
        });
        if !files.is_empty() {
            message["attachments"] = attachments_json(files);
        }
        let body = json!({ "name": name, "message": message });
        let req = self
            .request(ctx, Method::Post, &format!("/channels/{forum}/threads"))?
            .body(if files.is_empty() {
                Body::json(&body)
            } else {
                multipart(&body, files)
            });
        let maybe = |mut e: SendError| {
            e.kind = FailureKind::Rejected(0);
            e.message = format!(
                "{}. Discord may or may not have created the post, so CargoFrame did not retry: look in the forum",
                e.message
            );
            e
        };
        let resp = ctx.http.send(&req).map_err(|e| maybe(from_net(&e)))?;
        if resp.is_success() {
            return Ok(resp.json().unwrap_or(Value::Null));
        }
        let err = discord_error(&resp, "start the forum post");
        Err(match err.kind {
            FailureKind::Server(_) | FailureKind::Timeout | FailureKind::NoNetwork => maybe(err),
            _ => err,
        })
    }
}

/// What Discord says about a webhook: the server and channel it posts in.
struct HookInfo {
    guild: Option<String>,
    channel: Option<String>,
}

impl DiscordConnector {
    /// The id and token inside the link pasted for this slot.
    fn hook_link(&self, ctx: &Ctx, slot: u8) -> Result<(String, String), SendError> {
        let name =
            model::hook_secret(slot).ok_or_else(|| invalid("That webhook slot does not exist"))?;
        let raw = ctx
            .secret(name)?
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                not_set_up("That channel has no webhook link yet: open Connections, paste it and press Save")
            })?;
        parse_webhook(&raw).map_err(invalid)
    }

    /// The channel a share is for: the one picked (`hook3`), else the only one there is.
    fn hook_for(&self, chosen: Option<&str>) -> Result<&model::Hook, SendError> {
        let hooks = &self.settings.hooks;
        match chosen {
            Some(c) => c
                .strip_prefix("hook")
                .and_then(|n| n.parse::<u8>().ok())
                .and_then(|slot| hooks.iter().find(|h| h.slot == slot))
                .ok_or_else(|| invalid("Pick one of the channels you added a webhook for")),
            None => match hooks.as_slice() {
                [one] => Ok(one),
                [] => Err(not_set_up("Discord has no channel yet: open Connections and add a webhook")),
                _ => Err(invalid("Pick the channel to post in: map one to the field \"channel\" on the sharing button")),
            },
        }
    }

    fn hook_request(&self, ctx: &Ctx, method: Method, path: &str) -> Request {
        Request::new(method, format!("{}{path}", self.ep.api))
            .header("User-Agent", USER_AGENT)
            .header("Accept", "application/json")
            .timeout(ctx.timeout)
    }

    fn hook_info(&self, ctx: &Ctx, id: &str, token: &str) -> Result<HookInfo, SendError> {
        let req = self.hook_request(ctx, Method::Get, &format!("/webhooks/{id}/{token}"));
        let v = self.call(ctx, req, "look at the webhook")?;
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        Ok(HookInfo {
            guild: text("guild_id"),
            channel: text("channel_id"),
        })
    }

    /// One post through a webhook. A forum channel answers a plain post with "a forum needs a
    /// post name"; that is a clean refusal, so the caller may try again with a name. Once Discord
    /// might have taken the post, it is never sent again.
    fn hook_post(
        &self,
        ctx: &Ctx,
        link: &(String, String),
        thread: Option<&str>,
        post_name: Option<&str>,
        content: &str,
        files: &[&Attachment],
    ) -> Result<Option<Value>, SendError> {
        let mut path = format!("/webhooks/{}/{}?wait=true", link.0, link.1);
        if let Some(t) = thread {
            path.push_str(&format!("&thread_id={t}"));
        }
        let mut body = json!({ "content": content, "allowed_mentions": { "parse": [] } });
        if let Some(who) = Some(self.settings.shown_as.trim()).filter(|w| !w.is_empty()) {
            body["username"] = json!(who);
        }
        if let Some(n) = post_name {
            body["thread_name"] = json!(n);
        }
        if !files.is_empty() {
            body["attachments"] = attachments_json(files);
        }
        let req = self
            .hook_request(ctx, Method::Post, &path)
            .body(if files.is_empty() {
                Body::json(&body)
            } else {
                multipart(&body, files)
            });
        let unknown = |mut e: SendError| {
            e.kind = FailureKind::Rejected(0);
            e.message = format!(
                "{}. Discord may or may not have posted it, so CargoFrame did not retry: look in the channel",
                e.message
            );
            e
        };
        let resp = ctx.http.send(&req).map_err(|e| unknown(from_net(&e)))?;
        if resp.is_success() {
            return Ok(Some(resp.json().unwrap_or(Value::Null)));
        }
        let code = resp
            .json()
            .and_then(|v| v.get("code").and_then(Value::as_i64));
        if code == Some(220003) && post_name.is_none() && thread.is_none() {
            return Ok(None);
        }
        let err = discord_error(&resp, "post the message");
        Err(match err.kind {
            FailureKind::Server(_) | FailureKind::Timeout | FailureKind::NoNetwork => unknown(err),
            _ => err,
        })
    }

    fn send_webhook(
        &self,
        ctx: &Ctx,
        payload: &Payload,
        files: &[&Attachment],
    ) -> Result<SendOutcome, SendError> {
        let hook = self.hook_for(payload.value("channel"))?;
        let link = self.hook_link(ctx, hook.slot)?;
        let thread = payload
            .value("thread")
            .map(|t| parse_place(t).map_err(invalid))
            .transpose()?;
        let (content, cut) = compose(payload, true);
        if content.is_empty() && files.is_empty() {
            return Err(invalid(
                "There is nothing to send to Discord: no text, no screenshot and no files",
            ));
        }
        // Asks Discord where the webhook posts (for the link back), and finds out early when the
        // webhook has been deleted.
        let info = self.hook_info(ctx, &link.0, &link.1)?;

        let mut notes = Vec::new();
        if cut {
            notes.push(format!(
                "The message was cut to Discord's {MAX_CONTENT} characters"
            ));
        }
        let (posted, started) =
            match self.hook_post(ctx, &link, thread.as_deref(), None, &content, files)? {
                Some(v) => (v, false),
                None => {
                    // A forum: the first message of a new post has no bold title line; its name does that.
                    let (body, _) = compose(payload, false);
                    let name = post_name(payload);
                    let v = self
                        .hook_post(ctx, &link, None, Some(&name), &body, files)?
                        .ok_or_else(|| invalid("Discord would not start a post in that channel"))?;
                    (v, true)
                }
            };
        let place = posted
            .get("channel_id")
            .and_then(Value::as_str)
            .map(String::from)
            .or(thread.clone())
            .or(info.channel.clone())
            .unwrap_or_default();
        let message = posted.get("id").and_then(Value::as_str);
        let url = if started {
            link_to(info.guild.as_deref(), &place, None)
        } else {
            link_to(info.guild.as_deref(), &place, message)
        };
        let summary = if started {
            "Started a post in the Discord forum"
        } else {
            "Posted to Discord"
        }
        .to_string();
        ctx.progress.set("discord.summary", &summary);
        ctx.progress.set("discord.url", &url);
        Ok(SendOutcome {
            summary,
            url: Some(url),
            notes,
        })
    }
}

struct ChannelInfo {
    id: String,
    name: String,
    forum: bool,
    guild: String,
    /// The category it sits in, if any.
    category: Option<String>,
    /// Where Discord shows it: the category's place, then its own.
    order: (i64, i64),
}

fn attachments_json(files: &[&Attachment]) -> Value {
    Value::Array(
        files
            .iter()
            .enumerate()
            .map(|(i, f)| json!({ "id": i, "filename": f.name }))
            .collect(),
    )
}

fn multipart(body: &Value, files: &[&Attachment]) -> Body {
    let mut m = Multipart::form_data().field("payload_json", &body.to_string());
    for (i, f) in files.iter().enumerate() {
        m = m.file(&format!("files[{i}]"), &f.name, &f.mime, f.data.clone());
    }
    Body::multipart(m)
}

fn is_snowflake(s: &str) -> bool {
    (17..=20).contains(&s.len()) && s.chars().all(|c| c.is_ascii_digit())
}

/// A channel or thread id, or a link to a channel, a thread or a message
/// (`https://discord.com/channels/<server>/<channel or thread>/<message>`).
pub fn parse_place(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if is_snowflake(raw) {
        return Ok(raw.to_string());
    }
    let bad = || {
        format!(
            "\"{raw}\" is not a Discord channel or thread: pick one from the list, or paste its link (Copy Link in Discord)"
        )
    };
    let rest = raw.strip_prefix("https://").ok_or_else(bad)?;
    let (authority, path) = rest.split_once('/').ok_or_else(bad)?;
    let host = authority.to_lowercase();
    let known = ["discord.com", "discordapp.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")));
    if !known {
        return Err(bad());
    }
    let path = path.split(['?', '#']).next().unwrap_or("");
    let mut parts = path.trim_end_matches('/').split('/');
    if parts.next() != Some("channels") {
        return Err(bad());
    }
    let server = parts.next().ok_or_else(bad)?;
    if !(is_snowflake(server) || server == "@me") {
        return Err(bad());
    }
    let channel = parts.next().filter(|c| is_snowflake(c)).ok_or_else(bad)?;
    Ok(channel.to_string())
}

/// A webhook link (`https://discord.com/api/webhooks/<id>/<token>`) split into its id and token.
/// Only Discord's own addresses are taken, so a mistyped or planted link is refused.
pub fn parse_webhook(raw: &str) -> Result<(String, String), String> {
    let bad = || {
        "That is not a Discord webhook link: in Discord, open the channel's settings, then Integrations, Webhooks, and choose Copy Webhook URL".to_string()
    };
    let rest = raw.trim().strip_prefix("https://").ok_or_else(bad)?;
    let (authority, path) = rest.split_once('/').ok_or_else(bad)?;
    let host = authority.to_lowercase();
    let known = ["discord.com", "discordapp.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")));
    if !known
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
    {
        return Err(bad());
    }
    let path = path.split(['?', '#']).next().unwrap_or("");
    let mut parts = path.trim_end_matches('/').split('/');
    if parts.next() != Some("api") {
        return Err(bad());
    }
    let mut next = parts.next().ok_or_else(bad)?;
    let is_version =
        next.len() > 1 && next.starts_with('v') && next[1..].chars().all(|c| c.is_ascii_digit());
    if is_version {
        next = parts.next().ok_or_else(bad)?;
    }
    if next != "webhooks" {
        return Err(bad());
    }
    let id = parts.next().filter(|i| is_snowflake(i)).ok_or_else(bad)?;
    let token = parts
        .next()
        .filter(|t| {
            !t.is_empty()
                && t.len() <= 100
                && t.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .ok_or_else(bad)?;
    Ok((id.to_string(), token.to_string()))
}

/// The words of the message: an optional bold first line, then the message, then (when nothing
/// else was written) the window title. At most Discord's 2000 characters; says if it cut.
pub fn compose(payload: &Payload, with_title: bool) -> (String, bool) {
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let explicit_title = payload
        .value("title")
        .map(one_line)
        .map(|t| t.replace('*', ""))
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
                .filter(|t| !t.is_empty() && !(with_title && explicit_title.is_some()))
        });
    let mut parts = Vec::new();
    if with_title {
        if let Some(t) = &explicit_title {
            parts.push(format!("**{t}**"));
        }
    }
    if let Some(b) = body {
        parts.push(b);
    }
    let text = parts.join("\n");
    if text.chars().count() > MAX_CONTENT {
        let cut: String = text.chars().take(MAX_CONTENT - 1).collect();
        (format!("{cut}…"), true)
    } else {
        (text, false)
    }
}

/// The name of a forum post: the title field, else the window title, on one line.
pub fn post_name(payload: &Payload) -> String {
    let raw = payload
        .value("title")
        .map(str::to_string)
        .unwrap_or_else(|| payload.title.clone());
    let one: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = if one.is_empty() {
        "CargoFrame capture".to_string()
    } else {
        one
    };
    if name.chars().count() > MAX_POST_NAME {
        format!(
            "{}…",
            name.chars().take(MAX_POST_NAME - 1).collect::<String>()
        )
    } else {
        name
    }
}

/// What a refusal means, in words for the person, and how it is treated (retried or not).
pub fn discord_error(resp: &Response, what: &str) -> SendError {
    let v = resp.json().unwrap_or(Value::Null);
    let code = v.get("code").and_then(Value::as_i64).unwrap_or(0);
    let message = v
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let status = resp.status;
    let (kind, text) = match (status, code) {
        (401, _) => (
            FailureKind::AuthExpired,
            "Discord does not accept the bot token any more: open Connections and paste a new one".to_string(),
        ),
        (_, 10015 | 50027) => (
            FailureKind::AuthExpired,
            "Discord does not know that webhook any more: make a new one in the channel's settings and paste its link".to_string(),
        ),
        (429, _) => (
            FailureKind::Throttled(429),
            "Discord says to slow down".to_string(),
        ),
        (_, 50001) => (
            FailureKind::Rejected(status),
            "The bot cannot see that channel: add the bot to the server and let it View Channel".to_string(),
        ),
        (_, 50013) => (
            FailureKind::Rejected(status),
            "The bot is missing a permission in that channel (it needs Send Messages and Attach Files; for a forum, Create Public Threads)".to_string(),
        ),
        (_, 10003) => (
            FailureKind::Rejected(status),
            "Discord cannot find that channel".to_string(),
        ),
        (_, 40005) => (
            FailureKind::Rejected(status),
            "The files are bigger than this server allows for uploads".to_string(),
        ),
        (_, 50035) => (
            FailureKind::Rejected(status),
            if message.is_empty() {
                "Discord refused the message as malformed".to_string()
            } else {
                format!("Discord refused the message: {message}")
            },
        ),
        (s, _) if s >= 500 => (
            FailureKind::Server(s),
            "Discord had a problem on its side".to_string(),
        ),
        (s, _) => (
            FailureKind::Rejected(s),
            if message.is_empty() {
                format!("Discord answered HTTP {s}")
            } else {
                format!("Discord said \"{message}\"")
            },
        ),
    };
    let mut err = SendError::new(kind, format!("Could not {what}: {text}"));
    if status == 429 {
        let wait = resp.retry_after().or_else(|| {
            v.get("retry_after")
                .and_then(Value::as_f64)
                .filter(|s| *s >= 0.0 && *s < 3600.0)
                .map(|s| Duration::from_millis((s * 1000.0).ceil() as u64))
        });
        if let Some(w) = wait {
            err = err.with_retry_after(w);
        }
    }
    err
}

/// A value that stays the same between attempts of one share and differs between shares, for
/// Discord to recognise a message it has already taken (at most 25 characters).
fn make_nonce(ctx: &Ctx, channel: &str, content: &str) -> String {
    if let Some(n) = ctx.progress.get("discord.nonce") {
        return n;
    }
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in channel
        .bytes()
        .chain(content.bytes())
        .chain((ctx.now)().to_le_bytes())
    {
        hash ^= u64::from(b);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let nonce = format!("cf{hash:016x}");
    ctx.progress.set("discord.nonce", &nonce);
    nonce
}

fn link_to(guild: Option<&str>, channel: &str, message: Option<&str>) -> String {
    let server = guild.unwrap_or("@me");
    match message {
        Some(m) => format!("https://discord.com/channels/{server}/{channel}/{m}"),
        None => format!("https://discord.com/channels/{server}/{channel}"),
    }
}

impl Connector for DiscordConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint, list| FieldSpec {
            key,
            label,
            hint,
            list,
        };
        let hooks = self.webhooks();
        Describe {
            fields: vec![
                f(
                    "channel",
                    "Channel",
                    if hooks {
                        "One of the channels you added a webhook for"
                    } else {
                        "A text channel, or a forum to start a post in. The bot must be able to see it"
                    },
                    Some("channels"),
                ),
                f(
                    "thread",
                    "Thread or forum post",
                    if hooks {
                        "To reply in an existing one: paste its link (Copy Link in Discord). A webhook cannot list threads. Overrides the channel"
                    } else {
                        "To reply in an existing one: pick it, or paste its link. Overrides the channel"
                    },
                    Some("threads"),
                ),
                f("message", "Message", "Defaults to the prompt text, then the window title. 2000 characters at most", None),
                f("title", "Title", "The name of a forum post; a bold first line in a channel. Defaults to the window title for a forum", None),
            ],
            lists: vec![
                ListSpec {
                    id: "channels".into(),
                    label: if hooks {
                        "Channels you added a webhook for".into()
                    } else {
                        "Channels and forums the bot can see".into()
                    },
                    param: None,
                },
                ListSpec {
                    id: "threads".into(),
                    label: "Active threads and forum posts".into(),
                    param: None,
                },
            ],
            summary: "Posts a message into a channel or thread, or starts a forum post, with the screenshot and a zip of the ticked files attached. Nobody is pinged.".into(),
        }
    }

    fn status(&self, ctx: &Ctx) -> Status {
        let has = |name: &str| matches!(ctx.secret(name), Ok(Some(t)) if !t.trim().is_empty());
        let connected = if self.webhooks() {
            self.settings
                .hooks
                .iter()
                .any(|h| model::hook_secret(h.slot).is_some_and(has))
        } else {
            has(TOKEN)
        };
        if connected {
            Status::Connected
        } else {
            Status::Disconnected
        }
    }

    fn disconnect(&self, ctx: &Ctx) {
        for name in model::Kind::Discord.secret_names() {
            let _ = ctx
                .secrets
                .delete(&crate::secrets::key(ctx.connection_id, name));
        }
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        if self.webhooks() {
            return self.test_webhooks(ctx);
        }
        let me = self.get(ctx, "/users/@me", "check the sign-in")?;
        let name = ["global_name", "username"]
            .iter()
            .find_map(|k| me.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()))
            .unwrap_or("your bot");
        // Whether the bot has been added anywhere is the usual first mistake.
        Ok(match self.guilds(ctx) {
            Ok(g) if g.is_empty() => format!(
                "Signed in as {name}, but the bot is not in any server yet: open its invite link and add it to one"
            ),
            Ok(g) => format!(
                "Signed in as {name}, in {} server{}",
                g.len(),
                if g.len() == 1 { "" } else { "s" }
            ),
            Err(_) => format!("Signed in as {name}"),
        })
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        if self.webhooks() {
            return match list {
                "channels" => self.hook_options(ctx),
                // A webhook cannot look at the server; a thread is pasted as a link instead.
                "threads" => Ok(Vec::new()),
                other => Err(invalid(format!("Discord has no list called \"{other}\""))),
            };
        }
        match list {
            "channels" => {
                let mut channels = self.all_channels(ctx)?;
                channels.sort_by(|a, b| {
                    (&a.guild, a.order, &a.name).cmp(&(&b.guild, b.order, &b.name))
                });
                Ok(channels
                    .into_iter()
                    .map(|c| {
                        let label = if c.forum {
                            format!("#{} (forum)", c.name)
                        } else {
                            format!("#{}", c.name)
                        };
                        OptionItem::new(c.id, label).within(&c.guild, c.category.as_deref())
                    })
                    .collect())
            }
            "threads" => {
                let guilds = self.guilds(ctx)?;
                let names: std::collections::BTreeMap<String, String> = self
                    .channels_in(ctx, &guilds)?
                    .into_iter()
                    .map(|c| (c.id, c.name))
                    .collect();
                let mut out = Vec::new();
                for g in &guilds {
                    let v = match self.get(
                        ctx,
                        &format!("/guilds/{}/threads/active", g.id),
                        "list the threads",
                    ) {
                        Ok(v) => v,
                        Err(e) if Self::skippable(&e) => continue,
                        Err(e) => return Err(e),
                    };
                    for t in v
                        .get("threads")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let (Some(id), Some(name)) = (
                            t.get("id").and_then(Value::as_str),
                            t.get("name").and_then(Value::as_str),
                        ) else {
                            continue;
                        };
                        // The channel or forum a thread belongs to narrows the list.
                        let parent = t
                            .get("parent_id")
                            .and_then(Value::as_str)
                            .and_then(|p| names.get(p))
                            .map(|n| format!("#{n}"));
                        out.push(OptionItem::new(id, name).within(&g.name, parent.as_deref()));
                    }
                }
                out.sort_by(|a, b| {
                    (&a.server, &a.category, &a.label).cmp(&(&b.server, &b.category, &b.label))
                });
                Ok(out)
            }
            other => Err(invalid(format!("Discord has no list called \"{other}\""))),
        }
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let files: Vec<&Attachment> = [
            payload.image.as_ref(),
            payload.still.as_ref(),
            payload.files_zip.as_ref(),
        ]
        .into_iter()
        .flatten()
        .collect();

        // A share that already went through is not repeated.
        if let Some(done) = ctx.progress.get("discord.url") {
            return Ok(SendOutcome {
                summary: ctx.progress.get("discord.summary").unwrap_or_default(),
                url: Some(done),
                notes: Vec::new(),
            });
        }
        if self.webhooks() {
            return self.send_webhook(ctx, payload, &files);
        }

        let raw = payload
            .value("thread")
            .or_else(|| payload.value("channel"))
            .ok_or_else(|| {
                invalid(
                    "Discord needs a channel: map one to the field \"channel\" on the sharing button",
                )
            })?
            .to_string();

        let target = self.resolve(ctx, &raw)?;
        let forum = target.place == Place::Forum;
        let (content, cut) = compose(payload, !forum);
        if content.is_empty() && files.is_empty() {
            return Err(invalid(
                "There is nothing to send to Discord: no text, no screenshot and no files",
            ));
        }
        let mut notes = Vec::new();
        if cut {
            notes.push(format!(
                "The message was cut to Discord's {MAX_CONTENT} characters"
            ));
        }

        let (summary, url) = if forum {
            let name = post_name(payload);
            let v = self.start_post(ctx, &target.id, &name, &content, &files)?;
            let thread = v
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or(&target.id)
                .to_string();
            let guild = v
                .get("guild_id")
                .and_then(Value::as_str)
                .map(String::from)
                .or(target.guild.clone());
            (
                "Started a post in the Discord forum".to_string(),
                link_to(guild.as_deref(), &thread, None),
            )
        } else {
            let nonce = make_nonce(ctx, &target.id, &content);
            let v = self.post_message(ctx, &target.id, &content, &files, &nonce)?;
            let message = v.get("id").and_then(Value::as_str);
            (
                "Posted to Discord".to_string(),
                link_to(target.guild.as_deref(), &target.id, message),
            )
        };
        ctx.progress.set("discord.summary", &summary);
        ctx.progress.set("discord.url", &url);
        Ok(SendOutcome {
            summary,
            url: Some(url),
            notes,
        })
    }
}

impl DiscordConnector {
    /// The channels that have a webhook link stored, in the order they were added.
    fn hook_options(&self, ctx: &Ctx) -> Result<Vec<OptionItem>, SendError> {
        let mut out = Vec::new();
        for h in &self.settings.hooks {
            let Some(name) = model::hook_secret(h.slot) else {
                continue;
            };
            if !matches!(ctx.secret(name)?, Some(t) if !t.trim().is_empty()) {
                continue;
            }
            let label = Some(h.name.trim())
                .filter(|n| !n.is_empty())
                .unwrap_or("Channel");
            let item = OptionItem::new(format!("hook{}", h.slot), label);
            out.push(match h.server.trim() {
                "" => item,
                server => item.within(server, None),
            });
        }
        Ok(out)
    }

    /// Asks Discord about every webhook, so a deleted or mistyped link shows up at once.
    fn test_webhooks(&self, ctx: &Ctx) -> Result<String, SendError> {
        let (mut good, mut bad) = (Vec::new(), Vec::new());
        for h in &self.settings.hooks {
            let name = Some(h.name.trim())
                .filter(|n| !n.is_empty())
                .unwrap_or("A channel");
            match self
                .hook_link(ctx, h.slot)
                .and_then(|(id, token)| self.hook_info(ctx, &id, &token))
            {
                Ok(_) => good.push(name.to_string()),
                Err(e)
                    if matches!(
                        e.kind,
                        FailureKind::NoNetwork | FailureKind::Timeout | FailureKind::Certificate
                    ) =>
                {
                    return Err(e)
                }
                Err(e) => bad.push(format!("{name}: {}", e.message)),
            }
        }
        match (good.is_empty(), bad.is_empty()) {
            (true, true) => Err(not_set_up(
                "Discord has no channel yet: open Connections, add a webhook and paste its link",
            )),
            (true, false) => Err(not_set_up(bad.join("; "))),
            (false, true) => Ok(format!(
                "{} channel{} work: {}",
                good.len(),
                if good.len() == 1 { "" } else { "s" },
                good.join(", ")
            )),
            (false, false) => Ok(format!(
                "Works: {}. Needs a look: {}",
                good.join(", "),
                bad.join("; ")
            )),
        }
    }
}

#[cfg(test)]
mod tests;
