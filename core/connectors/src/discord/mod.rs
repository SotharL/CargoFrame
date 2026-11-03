//! Discord: post a message into a text channel or a thread, or start a post in a forum channel,
//! with the screenshot and a zip of the ticked files attached. The person creates a bot in the
//! Discord Developer Portal, adds it to their server and pastes the bot token once. The token is
//! kept in the credential store and sent to Discord's API address only.
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
use crate::model;
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
    pub fn new(_settings: model::Discord) -> Self {
        Self {
            ep: Endpoints::default(),
        }
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
        let mut out = Vec::new();
        for g in self.guilds(ctx)? {
            let list = match self.get(
                ctx,
                &format!("/guilds/{}/channels", g.id),
                "list the channels",
            ) {
                Ok(v) => v,
                Err(e) if Self::skippable(&e) => continue,
                Err(e) => return Err(e),
            };
            for c in list.as_array().into_iter().flatten() {
                let kind = c.get("type").and_then(Value::as_i64).unwrap_or(-1);
                if !matches!(kind, 0 | 5 | 15) {
                    continue;
                }
                let (Some(id), Some(name)) = (
                    c.get("id").and_then(Value::as_str),
                    c.get("name").and_then(Value::as_str),
                ) else {
                    continue;
                };
                out.push(ChannelInfo {
                    id: id.into(),
                    name: name.to_lowercase(),
                    forum: kind == 15,
                    guild: g.name.clone(),
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

struct ChannelInfo {
    id: String,
    name: String,
    forum: bool,
    guild: String,
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

fn link(guild: Option<&str>, channel: &str, message: Option<&str>) -> String {
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
        Describe {
            fields: vec![
                f("channel", "Channel", "A text channel, or a forum to start a post in. The bot must be able to see it", Some("channels")),
                f("thread", "Thread or forum post", "To reply in an existing one: pick it, or paste its link. Overrides the channel", Some("threads")),
                f("message", "Message", "Defaults to the prompt text, then the window title. 2000 characters at most", None),
                f("title", "Title", "The name of a forum post; a bold first line in a channel. Defaults to the window title for a forum", None),
            ],
            lists: vec![
                ListSpec {
                    id: "channels".into(),
                    label: "Channels and forums the bot can see".into(),
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
        match ctx.secret(TOKEN) {
            Ok(Some(t)) if !t.trim().is_empty() => Status::Connected,
            _ => Status::Disconnected,
        }
    }

    fn disconnect(&self, ctx: &Ctx) {
        let _ = ctx
            .secrets
            .delete(&crate::secrets::key(ctx.connection_id, TOKEN));
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
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
        match list {
            "channels" => {
                let mut channels = self.all_channels(ctx)?;
                channels.sort_by(|a, b| (&a.guild, &a.name).cmp(&(&b.guild, &b.name)));
                Ok(channels
                    .into_iter()
                    .map(|c| {
                        let label = if c.forum {
                            format!("#{} (forum)", c.name)
                        } else {
                            format!("#{}", c.name)
                        };
                        let mut item = OptionItem::new(c.id, label);
                        item.group = Some(c.guild);
                        item
                    })
                    .collect())
            }
            "threads" => {
                let mut out = Vec::new();
                for g in self.guilds(ctx)? {
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
                        let mut item = OptionItem::new(id, name);
                        item.group = Some(g.name.clone());
                        out.push(item);
                    }
                }
                out.sort_by(|a, b| (&a.group, &a.label).cmp(&(&b.group, &b.label)));
                Ok(out)
            }
            other => Err(invalid(format!("Discord has no list called \"{other}\""))),
        }
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let raw = payload
            .value("thread")
            .or_else(|| payload.value("channel"))
            .ok_or_else(|| {
                invalid(
                    "Discord needs a channel: map one to the field \"channel\" on the sharing button",
                )
            })?
            .to_string();
        let files: Vec<&Attachment> = [payload.image.as_ref(), payload.files_zip.as_ref()]
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
                link(guild.as_deref(), &thread, None),
            )
        } else {
            let nonce = make_nonce(ctx, &target.id, &content);
            let v = self.post_message(ctx, &target.id, &content, &files, &nonce)?;
            let message = v.get("id").and_then(Value::as_str);
            (
                "Posted to Discord".to_string(),
                link(target.guild.as_deref(), &target.id, message),
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

#[cfg(test)]
mod tests;
