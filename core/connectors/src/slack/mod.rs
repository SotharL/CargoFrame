//! Slack: post a message into a channel or a thread, with the screenshot and a zip of the ticked
//! files attached. The person creates a Slack app in their own workspace, gives it a few
//! permissions, installs it and pastes the "Bot User OAuth Token" (it starts with `xoxb-`) once.
//! The token is kept in the credential store and sent to Slack's API address only.
//!
//! Slack answers almost every failure with HTTP 200 and `{"ok": false, "error": "..."}`, so the
//! body is always read. Files go through Slack's three-step upload (ask for an address, send the
//! bytes there, tell Slack to share them); what each step has already done is recorded in
//! [`Progress`], so a retry never uploads a file twice or posts a message twice.
//!
//! Text is escaped (`&`, `<`, `>`): in Slack `<!channel>` is a command, and the window title or
//! a data rule must not be able to write one. Addresses still become links by themselves.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::model;
use crate::payload::{Attachment, OptionItem, Payload, SendOutcome};
use crate::richtext::{render, Style};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use cargoframe_net::{encode_component, Body, Method, Request};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};

const TOKEN: &str = "token";
/// Pages of channels read at most (200 each), so a huge workspace cannot hang the window.
const MAX_PAGES: usize = 10;

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub api: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            api: "https://slack.com/api".into(),
        }
    }
}

pub struct SlackConnector {
    ep: Endpoints,
}

/// One channel the app can see.
struct Channel {
    id: String,
    name: String,
    private: bool,
}

impl SlackConnector {
    pub fn new(_settings: model::Slack) -> Self {
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
                not_set_up("Slack has no bot token yet: open Connections, paste it and press Save")
            })
    }

    /// A call to a Slack method (`chat.postMessage`, `conversations.list?limit=200`).
    fn request(&self, ctx: &Ctx, method: Method, name: &str) -> Result<Request, SendError> {
        Ok(Request::new(method, format!("{}/{name}", self.ep.api))
            .header("Authorization", format!("Bearer {}", self.token(ctx)?))
            .header("Accept", "application/json")
            .timeout(ctx.timeout))
    }

    /// Send it and read the answer; `ok: false` becomes a failure.
    fn call(&self, ctx: &Ctx, req: Request, what: &str) -> Result<Value, SendError> {
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(from_response(&resp, what));
        }
        let v = resp
            .json()
            .ok_or_else(|| invalid(format!("Slack answered {what} with something unreadable")))?;
        if v.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(v)
        } else {
            Err(api_error(&v, what))
        }
    }

    fn post_json(
        &self,
        ctx: &Ctx,
        name: &str,
        body: &Value,
        what: &str,
    ) -> Result<Value, SendError> {
        let req = self
            .request(ctx, Method::Post, name)?
            .body(Body::json(body));
        self.call(ctx, req, what)
    }

    /// Every channel the app can see, public and private, newest API page last.
    fn channels(&self, ctx: &Ctx) -> Result<Vec<Channel>, SendError> {
        let mut out = Vec::new();
        let mut cursor = String::new();
        for _ in 0..MAX_PAGES {
            let mut name = String::from(
                "conversations.list?types=public_channel,private_channel&exclude_archived=true&limit=200",
            );
            if !cursor.is_empty() {
                name.push_str(&format!("&cursor={}", encode_component(&cursor)));
            }
            let v = self.call(
                ctx,
                self.request(ctx, Method::Get, &name)?,
                "list the channels",
            )?;
            for c in v
                .get("channels")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let (Some(id), Some(name)) = (
                    c.get("id").and_then(Value::as_str),
                    c.get("name").and_then(Value::as_str),
                ) else {
                    continue;
                };
                out.push(Channel {
                    id: id.into(),
                    name: name.into(),
                    private: c.get("is_private").and_then(Value::as_bool) == Some(true),
                });
            }
            cursor = v
                .pointer("/response_metadata/next_cursor")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if cursor.is_empty() {
                break;
            }
        }
        Ok(out)
    }

    /// A channel id from what the person picked or typed: an id stays, `#name` is looked up.
    fn resolve_channel(&self, ctx: &Ctx, raw: &str) -> Result<String, SendError> {
        if let Some(id) = ctx.progress.get("slack.channel") {
            return Ok(id);
        }
        let raw = raw.trim();
        let id = if is_channel_id(raw) {
            raw.to_string()
        } else {
            let name = raw.trim_start_matches('#').trim().to_lowercase();
            if name.is_empty() {
                return Err(invalid("The Slack channel is empty"));
            }
            self.channels(ctx)?
                .into_iter()
                .find(|c| c.name == name)
                .map(|c| c.id)
                .ok_or_else(|| {
                    invalid(format!(
                        "Slack has no channel called #{name} that the app can see. For a private channel, invite the app to it first"
                    ))
                })?
        };
        ctx.progress.set("slack.channel", &id);
        Ok(id)
    }

    /// Slack hands out the address a file's bytes go to. Only Slack's own addresses are used.
    fn trusted_upload(&self, url: &str) -> bool {
        let Some(rest) = url.strip_prefix("https://") else {
            // The tests stand a local server in for Slack: its own address is trusted.
            return url.starts_with(&format!("{}/", origin(&self.ep.api)))
                && !self.ep.api.starts_with("https://");
        };
        let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
        if authority.contains('@') {
            return false;
        }
        let host = authority.split(':').next().unwrap_or("").to_lowercase();
        host == "slack.com" || host.ends_with(".slack.com")
    }

    /// Steps 1 and 2 of an upload: get an address, send the bytes. Returns Slack's file id.
    fn upload(&self, ctx: &Ctx, att: &Attachment) -> Result<String, SendError> {
        let length = att.data.len().to_string();
        let req = self
            .request(ctx, Method::Post, "files.getUploadURLExternal")?
            .body(Body::form(&[("filename", &att.name), ("length", &length)]));
        let v = self.call(ctx, req, &format!("start uploading {}", att.name))?;
        let (Some(url), Some(id)) = (
            v.get("upload_url").and_then(Value::as_str),
            v.get("file_id").and_then(Value::as_str),
        ) else {
            return Err(invalid("Slack did not say where to upload the file"));
        };
        if !self.trusted_upload(url) {
            return Err(invalid(
                "Slack gave an upload address that is not Slack's, so nothing was uploaded",
            ));
        }
        // No token goes here: the address itself is the permission.
        let put = Request::new(Method::Post, url)
            .body(Body::Bytes {
                content_type: "application/octet-stream".into(),
                data: att.data.clone(),
            })
            .timeout(ctx.timeout);
        let resp = ctx.http.send(&put).map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(from_response(&resp, &format!("upload {}", att.name)));
        }
        Ok(id.to_string())
    }
}

fn origin(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => format!(
            "{scheme}://{}",
            rest.split(['/', '?', '#']).next().unwrap_or("")
        ),
        None => url.to_string(),
    }
}

/// `C0123ABCD`: public channels start with C, private ones with G, direct messages with D.
pub fn is_channel_id(s: &str) -> bool {
    (9..=12).contains(&s.len())
        && s.starts_with(['C', 'G', 'D'])
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
}

/// A message to reply under, and the channel the link said it is in.
#[derive(Debug, PartialEq, Eq)]
pub struct ThreadRef {
    pub channel: Option<String>,
    pub ts: String,
}

fn is_ts(s: &str) -> bool {
    match s.split_once('.') {
        Some((a, b)) => {
            a.len() == 10
                && b.len() == 6
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// A thread from what the person pasted: a message link (`…/archives/C0123/p1699999999000100`,
/// with `?thread_ts=` when the link is to a reply) or the timestamp `1699999999.000100`.
pub fn parse_thread(raw: &str) -> Result<ThreadRef, String> {
    let raw = raw.trim();
    if is_ts(raw) {
        return Ok(ThreadRef {
            channel: None,
            ts: raw.to_string(),
        });
    }
    let bad = || {
        format!(
            "\"{raw}\" is not a Slack thread: paste the link of a message (Copy link in Slack) or its timestamp like 1699999999.000100"
        )
    };
    let rest = raw.strip_prefix("https://").ok_or_else(bad)?;
    let (authority, path_and_query) = rest.split_once('/').ok_or_else(bad)?;
    let host = authority.to_lowercase();
    if !(host == "slack.com" || host.ends_with(".slack.com")) {
        return Err(bad());
    }
    let (path, query) = match path_and_query.split_once('?') {
        Some((p, q)) => (p, q),
        None => (path_and_query, ""),
    };
    let mut parts = path.trim_end_matches('/').split('/');
    if parts.next() != Some("archives") {
        return Err(bad());
    }
    let channel = parts.next().filter(|c| is_channel_id(c)).ok_or_else(bad)?;
    let from_query = query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == "thread_ts")
        .map(|(_, v)| v.to_string());
    let ts = match from_query {
        Some(t) if is_ts(&t) => t,
        Some(_) => return Err(bad()),
        None => {
            let p = parts
                .next()
                .and_then(|p| p.strip_prefix('p'))
                .ok_or_else(bad)?;
            if p.len() != 16 || !p.chars().all(|c| c.is_ascii_digit()) {
                return Err(bad());
            }
            format!("{}.{}", &p[..10], &p[10..])
        }
    };
    Ok(ThreadRef {
        channel: Some(channel.to_string()),
        ts,
    })
}

/// Slack treats `<...>` as markup (`<!channel>` notifies everyone), so captured text is escaped.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The words of the message: an optional bold title, then the message, then (when nothing else
/// was written) the window title.
pub fn compose(payload: &Payload) -> String {
    let one_line = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    let body = payload.message(payload.value("title").is_some());
    let mut parts = Vec::new();
    if let Some(t) = payload.value("title") {
        let t = one_line(t).replace('*', "");
        if !t.is_empty() {
            parts.push(format!("*{}*", escape(&t)));
        }
    }
    if let Some(b) = body {
        // the text may carry bold, italic, code, quotes and lists (see `richtext`); it is escaped too
        parts.push(render(&b, Style::Slack));
    }
    parts.join("\n")
}

/// What `ok: false` means, in words for the person, and how it is treated (retried or not).
pub fn api_error(v: &Value, what: &str) -> SendError {
    let code = v
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or("unknown_error");
    let needed = v.get("needed").and_then(Value::as_str);
    let (kind, text) = match code {
        "invalid_auth" | "not_authed" | "token_revoked" | "token_expired" | "account_inactive"
        | "no_permission" => (
            FailureKind::AuthExpired,
            "Slack does not accept the bot token any more: open Connections and paste a new one"
                .to_string(),
        ),
        "missing_scope" => (
            FailureKind::Rejected(200),
            match needed {
                Some(n) => format!(
                    "The Slack app is missing the permission \"{n}\": add it under OAuth & Permissions, then reinstall the app and paste the new token"
                ),
                None => "The Slack app is missing a permission: add it under OAuth & Permissions, then reinstall the app and paste the new token".to_string(),
            },
        ),
        "channel_not_found" => (
            FailureKind::Rejected(200),
            "Slack cannot find that channel. For a private channel, invite the app first (type /invite @ and the app's name there)".to_string(),
        ),
        "not_in_channel" => (
            FailureKind::Rejected(200),
            "The app is not in that channel. Invite it (type /invite @ and the app's name there)".to_string(),
        ),
        "is_archived" => (
            FailureKind::Rejected(200),
            "That channel is archived".to_string(),
        ),
        "msg_too_long" => (
            FailureKind::Rejected(200),
            "The message is too long for Slack".to_string(),
        ),
        "no_text" => (
            FailureKind::Rejected(200),
            "There was no text to post".to_string(),
        ),
        "restricted_action"
        | "restricted_action_read_only_channel"
        | "restricted_action_thread_only_channel"
        | "restricted_action_non_threadable_channel" => (
            FailureKind::Rejected(200),
            "Slack's settings for that channel do not let the app post there".to_string(),
        ),
        "ratelimited" | "rate_limited" => (
            FailureKind::Throttled(429),
            "Slack says to slow down".to_string(),
        ),
        "fatal_error" | "internal_error" | "service_unavailable" | "request_timeout" => (
            FailureKind::Server(500),
            "Slack had a problem on its side".to_string(),
        ),
        other => (
            FailureKind::Rejected(200),
            format!("Slack said \"{other}\""),
        ),
    };
    SendError::new(kind, format!("Could not {what}: {text}"))
}

impl Connector for SlackConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint, list| FieldSpec {
            key,
            label,
            hint,
            list,
        };
        Describe {
            fields: vec![
                f("channel", "Channel", "Where to post. The app must have been invited to it", Some("channels")),
                f("thread", "Reply in a thread", "A message link (Copy link in Slack) or its timestamp. Empty: a new message", None),
                f("message", "Message", "Defaults to the prompt text, then the window title", None),
                f("title", "Bold first line", "Empty: none", None),
                f("tags", "Tags", "Added as a last line. Separate several with commas", None),
            ],
            lists: vec![ListSpec {
                id: "channels".into(),
                label: "Channels the app can see".into(),
                param: None,
                inside: None,
            }],
            summary: "Posts a message into a channel or a thread, with the screenshot and a zip of the ticked files attached. The app has to be a member of the channel.".into(),
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
        let req = self
            .request(ctx, Method::Post, "auth.test")?
            .body(Body::form(&[]));
        let v = self.call(ctx, req, "check the sign-in")?;
        let text = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
        Ok(match (text("team"), text("user")) {
            (Some(team), Some(user)) => format!("Signed in to {team} as {user}"),
            (Some(team), None) => format!("Signed in to {team}"),
            _ => "Signed in to Slack".to_string(),
        })
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        match list {
            "channels" => {
                let mut channels = self.channels(ctx)?;
                channels.sort_by(|a, b| a.name.cmp(&b.name));
                Ok(channels
                    .into_iter()
                    .map(|c| {
                        let mut item = OptionItem::new(c.id, format!("#{}", c.name));
                        item.group = Some(
                            if c.private {
                                "Private channels"
                            } else {
                                "Public channels"
                            }
                            .to_string(),
                        );
                        item
                    })
                    .collect())
            }
            other => Err(invalid(format!("Slack has no list called \"{other}\""))),
        }
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let thread = payload
            .value("thread")
            .map(parse_thread)
            .transpose()
            .map_err(invalid)?;
        let channel_raw =
            match (
                payload.value("channel"),
                thread.as_ref().and_then(|t| t.channel.as_deref()),
            ) {
                (Some(c), _) => c.to_string(),
                (None, Some(c)) => c.to_string(),
                (None, None) => return Err(invalid(
                    "Slack needs a channel: map one to the field \"channel\" on the sharing button",
                )),
            };
        let channel = self.resolve_channel(ctx, &channel_raw)?;
        if let Some(from_link) = thread.as_ref().and_then(|t| t.channel.as_deref()) {
            if from_link != channel {
                return Err(invalid(
                    "The thread link is in a different channel than the one chosen",
                ));
            }
        }
        let thread_ts = thread.as_ref().map(|t| t.ts.clone());
        let text = compose(payload);
        let uploads: Vec<(&str, &Attachment)> = [
            ("slack.file.image", payload.image.as_ref()),
            ("slack.file.still", payload.still.as_ref()),
            ("slack.file.zip", payload.files_zip.as_ref()),
        ]
        .into_iter()
        .filter_map(|(step, a)| a.map(|a| (step, a)))
        .collect();

        let mut url = None;
        if uploads.is_empty() {
            if text.is_empty() {
                return Err(invalid(
                    "There is nothing to send to Slack: no text, no screenshot and no files",
                ));
            }
            if ctx.progress.get("slack.ts").is_none() {
                let mut body = json!({ "channel": channel, "text": text });
                if let Some(ts) = &thread_ts {
                    body["thread_ts"] = json!(ts);
                }
                let v = self.post_json(ctx, "chat.postMessage", &body, "post the message")?;
                let ts = v
                    .get("ts")
                    .and_then(Value::as_str)
                    .unwrap_or("posted")
                    .to_string();
                ctx.progress.set("slack.ts", &ts);
            }
            if let Some(ts) = ctx.progress.get("slack.ts").filter(|t| is_ts(t)) {
                let path = format!(
                    "chat.getPermalink?channel={}&message_ts={}",
                    encode_component(&channel),
                    encode_component(&ts)
                );
                // A missing link is not worth failing a share that already went through.
                url = self
                    .request(ctx, Method::Get, &path)
                    .and_then(|r| self.call(ctx, r, "get the link"))
                    .ok()
                    .and_then(|v| v.get("permalink").and_then(Value::as_str).map(String::from));
            }
        } else {
            for (step, att) in &uploads {
                if !ctx.progress.done(step) {
                    let id = self.upload(ctx, att)?;
                    ctx.progress.set(step, &id);
                }
            }
            if !ctx.progress.done("slack.complete") {
                let files: Vec<Value> = uploads
                    .iter()
                    .filter_map(|(step, att)| {
                        ctx.progress
                            .get(step)
                            .map(|id| json!({ "id": id, "title": att.name }))
                    })
                    .collect();
                let mut body = json!({ "files": files, "channel_id": channel });
                if !text.is_empty() {
                    body["initial_comment"] = json!(text);
                }
                if let Some(ts) = &thread_ts {
                    body["thread_ts"] = json!(ts);
                }
                let v = self.post_json(
                    ctx,
                    "files.completeUploadExternal",
                    &body,
                    "share the files",
                )?;
                url = v
                    .pointer("/files/0/permalink")
                    .and_then(Value::as_str)
                    .map(String::from);
                ctx.progress.set("slack.complete", "1");
            }
        }

        Ok(SendOutcome {
            summary: if thread_ts.is_some() {
                "Posted in the Slack thread".to_string()
            } else {
                "Posted to Slack".to_string()
            },
            url,
            notes: Vec::new(),
            place: None,
        })
    }
}

#[cfg(test)]
mod tests;
