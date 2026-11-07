//! Discord through webhooks: no bot, one link per channel. A webhook cannot list servers,
//! channels or threads, so the person adds each channel they post to; a forum is posted to by a
//! thread name, an existing thread by its link. The bot way is in the parent module.

use super::{
    attachments_json, compose, discord_error, is_snowflake, link_to, multipart, parse_place,
    post_name, DiscordConnector, MAX_CONTENT, USER_AGENT,
};
use crate::ctx::Ctx;
use crate::error::{from_net, invalid, not_set_up};
use crate::model;
use crate::payload::{Attachment, OptionItem, Payload, SendOutcome};
use cargoframe_net::{Body, Method, Request};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};

/// What came of one post through a webhook.
enum Posted {
    /// Discord took it.
    Done(Value),
    /// The channel is a forum and wants a post name.
    NeedsName,
    /// Discord does not know that thread in this webhook's channel.
    WrongChannel,
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

    /// The channels a share may go to, the likeliest first: the one picked (`hook3`), else the
    /// only one there is. A thread is not told which channel it is in, so when one is given the
    /// other channels come after, and the first that knows the thread is the one used.
    fn hooks_for(
        &self,
        chosen: Option<&str>,
        thread: bool,
    ) -> Result<Vec<&model::Hook>, SendError> {
        let hooks = &self.settings.hooks;
        let first = match chosen {
            Some(c) => Some(
                c.strip_prefix("hook")
                    .and_then(|n| n.parse::<u8>().ok())
                    .and_then(|slot| hooks.iter().find(|h| h.slot == slot))
                    .ok_or_else(|| invalid("Pick one of the channels you added a webhook for"))?,
            ),
            None => match hooks.as_slice() {
                [] => {
                    return Err(not_set_up(
                        "Discord has no channel yet: open Connections and add a webhook",
                    ))
                }
                [one] => Some(one),
                // several channels and nothing picked: only a thread can say where to post
                _ if thread => None,
                _ => {
                    return Err(invalid(
                        "Pick the channel to post in with the \"Where to post\" dropdown",
                    ))
                }
            },
        };
        let mut out: Vec<&model::Hook> = first.into_iter().collect();
        if thread {
            out.extend(
                hooks
                    .iter()
                    .filter(|h| Some(h.slot) != first.map(|f| f.slot)),
            );
        }
        Ok(out)
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
    /// post name", and a thread of another channel with "unknown channel"; both are clean
    /// refusals, so the caller may try again (with a name, in another channel). Once Discord
    /// might have taken the post, it is never sent again.
    fn hook_post(
        &self,
        ctx: &Ctx,
        link: &(String, String),
        thread: Option<&str>,
        post_name: Option<&str>,
        content: &str,
        files: &[&Attachment],
    ) -> Result<Posted, SendError> {
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
            return Ok(Posted::Done(resp.json().unwrap_or(Value::Null)));
        }
        let code = resp
            .json()
            .and_then(|v| v.get("code").and_then(Value::as_i64));
        if code == Some(220003) && post_name.is_none() && thread.is_none() {
            return Ok(Posted::NeedsName);
        }
        if code == Some(10003) && thread.is_some() {
            return Ok(Posted::WrongChannel);
        }
        let err = discord_error(&resp, "post the message");
        Err(match err.kind {
            FailureKind::Server(_) | FailureKind::Timeout | FailureKind::NoNetwork => unknown(err),
            _ => err,
        })
    }

    pub(super) fn send_webhook(
        &self,
        ctx: &Ctx,
        payload: &Payload,
        files: &[&Attachment],
    ) -> Result<SendOutcome, SendError> {
        let thread = payload
            .value("thread")
            .map(|t| parse_place(t).map_err(invalid))
            .transpose()?;
        let candidates = self.hooks_for(payload.value("channel"), thread.is_some())?;
        let (content, cut) = compose(payload, true);
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
        // The first channel that takes the post. Only a thread is ever tried in more than one:
        // a refusal for it ("unknown channel") means the thread is somewhere else, and nothing
        // was posted.
        let mut taken = None;
        for hook in &candidates {
            let link = self.hook_link(ctx, hook.slot)?;
            // Asks Discord where the webhook posts (for the link back), and finds out early when
            // the webhook has been deleted.
            let info = self.hook_info(ctx, &link.0, &link.1)?;
            match self.hook_post(ctx, &link, thread.as_deref(), None, &content, files)? {
                Posted::Done(v) => {
                    taken = Some((v, false, link, info));
                    break;
                }
                Posted::NeedsName => {
                    // A forum: the first message of a new post has no bold title line; its name does that.
                    let (body, _) = compose(payload, false);
                    let name = post_name(payload);
                    let Posted::Done(v) =
                        self.hook_post(ctx, &link, None, Some(&name), &body, files)?
                    else {
                        return Err(invalid("Discord would not start a post in that channel"));
                    };
                    taken = Some((v, true, link, info));
                    break;
                }
                Posted::WrongChannel => {}
            }
        }
        let Some((posted, started, _link, info)) = taken else {
            return Err(invalid(if candidates.len() > 1 {
                "That thread is not in any channel you added a webhook for. A webhook can only post in threads of its own channel: add a webhook for the thread's channel in Connections, or pick another thread"
            } else {
                "That thread is not in the channel of this webhook. A webhook can only post in threads of its own channel: pick that channel, or another thread"
            }));
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
        // A webhook cannot list threads, so the ones it posted in are handed back to be offered
        // next time. A post it started is known by its name; a thread that was pasted, by its end.
        let remembered = if started {
            Some(OptionItem::new(&place, post_name(payload)))
        } else {
            thread.as_deref().map(|t| {
                let end: String = t
                    .chars()
                    .rev()
                    .take(4)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();
                OptionItem::new(t, format!("Thread …{end}"))
            })
        };
        ctx.progress.set("discord.summary", &summary);
        ctx.progress.set("discord.url", &url);
        Ok(SendOutcome {
            summary,
            url: Some(url),
            notes,
            place: remembered,
        })
    }
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

impl DiscordConnector {
    /// The channels that have a webhook link stored, in the order they were added.
    pub(super) fn hook_options(&self, ctx: &Ctx) -> Result<Vec<OptionItem>, SendError> {
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
    pub(super) fn test_webhooks(&self, ctx: &Ctx) -> Result<String, SendError> {
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
