//! Bluesky: a post with the screenshot. The person types their handle and an *app password*
//! (Settings, Privacy and security, App passwords in Bluesky): no developer app to register.
//!
//! Each share signs in (`createSession`), uploads the picture (`uploadBlob`, shrunk to fit), and
//! writes the post (`createRecord`). Links in the text are marked so Bluesky shows them as links.
//! The post is never written twice: if an earlier attempt may have posted it, it is not retried.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::media::{fit, plain_text};
use crate::model;
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, Status};
use cargoframe_net::{check_url, Body, Method, Request};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};

/// Most characters in a post (Bluesky counts 300 graphemes; a character is never more than one).
pub const MAX_TEXT: usize = 300;
/// Largest picture Bluesky keeps.
pub const MAX_PICTURE: usize = 1_000_000;

pub struct BlueskyConnector {
    settings: model::Bluesky,
}

/// A signed-in session: who, and the key for the next calls.
struct Session {
    did: String,
    handle: String,
    jwt: String,
}

impl BlueskyConnector {
    pub fn new(settings: model::Bluesky) -> Self {
        Self { settings }
    }

    fn service(&self) -> Result<String, SendError> {
        let s = self.settings.service.trim().trim_end_matches('/');
        let s = if s.is_empty() {
            "https://bsky.social"
        } else {
            s
        };
        check_url(s).map_err(|e| invalid(format!("The Bluesky server address: {e}")))?;
        Ok(s.to_string())
    }

    fn handle(&self) -> Result<String, SendError> {
        let h = clean_handle(&self.settings.handle).map_err(not_set_up)?;
        Ok(h)
    }

    fn sign_in(&self, ctx: &Ctx) -> Result<Session, SendError> {
        let handle = self.handle()?;
        let password = ctx
            .secret("app_password")?
            .filter(|p| !p.trim().is_empty())
            .ok_or_else(|| not_set_up("Bluesky needs an app password: add it to the connection"))?;
        let req = Request::post(format!(
            "{}/xrpc/com.atproto.server.createSession",
            self.service()?
        ))
        .timeout(ctx.timeout)
        .body(Body::json(
            &json!({"identifier": handle, "password": password.trim()}),
        ));
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if matches!(resp.status, 400 | 401) {
            return Err(SendError::new(
                FailureKind::AuthExpired,
                "Bluesky did not accept the handle or the app password: check both, or make a new app password",
            ));
        }
        if !resp.is_success() {
            return Err(from_response(&resp, "sign in to Bluesky"));
        }
        let v = resp
            .json()
            .ok_or_else(|| invalid("Bluesky answered the sign-in with something unreadable"))?;
        let get = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        match (get("did"), get("accessJwt")) {
            (Some(did), Some(jwt)) => Ok(Session {
                did,
                handle: get("handle").unwrap_or(handle),
                jwt,
            }),
            _ => Err(invalid("Bluesky's sign-in answer had no account")),
        }
    }

    fn call(&self, session: &Session, ctx: &Ctx, method: &str) -> Result<Request, SendError> {
        Ok(
            Request::new(Method::Post, format!("{}/xrpc/{method}", self.service()?))
                .header("Authorization", format!("Bearer {}", session.jwt))
                .timeout(ctx.timeout),
        )
    }
}

/// `@jane.bsky.social`, `jane.bsky.social` and `https://bsky.app/profile/jane.bsky.social` all
/// mean `jane.bsky.social`. A handle is a domain name.
pub fn clean_handle(raw: &str) -> Result<String, String> {
    let t = raw.trim();
    let t = t
        .strip_prefix("https://bsky.app/profile/")
        .unwrap_or(t)
        .trim_start_matches('@')
        .trim_end_matches('/')
        .to_lowercase();
    let ok = t.contains('.')
        && t.len() <= 253
        && t.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if ok {
        Ok(t)
    } else if t.is_empty() {
        Err("Bluesky needs your handle, like jane.bsky.social".into())
    } else {
        Err(format!(
            "\"{raw}\" is not a Bluesky handle: it looks like jane.bsky.social"
        ))
    }
}

/// The links in `text`, as Bluesky wants them marked: where each starts and ends, counted in
/// bytes of the text, and the address.
pub fn link_facets(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = ["https://", "http://"]
        .iter()
        .filter_map(|p| text[from..].find(p).map(|i| from + i))
        .min()
    {
        let rest = &text[found..];
        let len = rest
            .find(|c: char| c.is_whitespace() || "<>\"".contains(c))
            .unwrap_or(rest.len());
        // a full stop or a bracket that ends a sentence is not part of the link
        let link = rest[..len].trim_end_matches(['.', ',', ')', ';', ':', '!', '?']);
        if link.len() > "https://".len() {
            out.push(json!({
                "index": {"byteStart": found, "byteEnd": found + link.len()},
                "features": [{"$type": "app.bsky.richtext.facet#link", "uri": link}],
            }));
        }
        from = found + len.max(1);
    }
    out
}

/// `at://did:plc:abc/app.bsky.feed.post/3k4` and a handle make the post's web address.
pub fn post_url(uri: &str, handle: &str) -> Option<String> {
    let rkey = uri.rsplit('/').next().filter(|k| !k.is_empty())?;
    uri.contains("/app.bsky.feed.post/")
        .then(|| format!("https://bsky.app/profile/{handle}/post/{rkey}"))
}

impl Connector for BlueskyConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint| FieldSpec {
            key,
            label,
            hint,
            list: None,
        };
        Describe {
            fields: vec![
                f("text", "Text", "The post, at most 300 characters. Defaults to the prompt, then the window title"),
                f("alt", "Picture description", "Read out to people who cannot see the picture. Defaults to the window title"),
            ],
            lists: Vec::new(),
            summary: "Posts the text and the screenshot (made smaller when it is over 1 MB) to your Bluesky account. Links become links; formatting is shown as plain text. Files and videos are not sent.".into(),
        }
    }

    fn status(&self, ctx: &Ctx) -> Status {
        let has = ctx
            .secret("app_password")
            .ok()
            .flatten()
            .is_some_and(|p| !p.trim().is_empty());
        if has && self.handle().is_ok() {
            Status::Connected
        } else {
            Status::Disconnected
        }
    }

    fn disconnect(&self, ctx: &Ctx) {
        let _ = ctx
            .secrets
            .delete(&crate::secrets::key(ctx.connection_id, "app_password"));
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let s = self.sign_in(ctx)?;
        Ok(format!("Signed in to Bluesky as @{}", s.handle))
    }

    fn options(&self, _ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        Err(invalid(format!("Bluesky has no list called \"{list}\"")))
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        if ctx.progress.done("bluesky.posted") {
            return Err(invalid(
                "An earlier attempt may already have posted this. Check your Bluesky profile before trying again",
            ));
        }
        let mut notes = Vec::new();
        let source = payload
            .value("text")
            .map(str::to_string)
            .or_else(|| payload.prompt.clone().filter(|p| !p.trim().is_empty()))
            .unwrap_or_else(|| payload.title.clone());
        let (text, cut) = plain_text(&source, MAX_TEXT);
        if cut {
            notes.push(format!(
                "The text was cut to {MAX_TEXT} characters, the most Bluesky shows"
            ));
        }

        // The picture: the screenshot, or the still that goes with a recording.
        let picture = payload
            .image
            .as_ref()
            .filter(|a| a.mime.starts_with("image/"))
            .or(payload.still.as_ref());
        if payload
            .image
            .as_ref()
            .is_some_and(|a| a.mime.starts_with("video/"))
        {
            notes.push("Bluesky gets the screenshot, not the recording".to_string());
        }
        if payload.files_zip.is_some() {
            notes.push("Bluesky cannot take files, so the ticked files were not sent".to_string());
        }
        if picture.is_none() && text.trim().is_empty() {
            return Err(invalid("There is nothing to post: no text and no picture"));
        }

        let session = self.sign_in(ctx)?;
        let mut embed = Value::Null;
        if let Some(pic) = picture {
            let fitted = fit(pic, MAX_PICTURE, "Bluesky").map_err(invalid)?;
            if fitted.shrunk {
                notes.push("The picture was made smaller to fit Bluesky's 1 MB".to_string());
            }
            let blob = match ctx.progress.get("bluesky.blob") {
                Some(saved) => serde_json::from_str(&saved).unwrap_or(Value::Null),
                None => Value::Null,
            };
            let blob = if blob.is_null() {
                let req = self
                    .call(&session, ctx, "com.atproto.repo.uploadBlob")?
                    .body(Body::Bytes {
                        content_type: fitted.picture.mime.clone(),
                        data: fitted.picture.data.clone(),
                    });
                let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
                if !resp.is_success() {
                    return Err(from_response(&resp, "upload the picture to Bluesky"));
                }
                let blob = resp
                    .json()
                    .and_then(|v| v.get("blob").cloned())
                    .ok_or_else(|| invalid("Bluesky did not say where it kept the picture"))?;
                ctx.progress.set("bluesky.blob", &blob.to_string());
                blob
            } else {
                blob
            };
            let alt = payload
                .value("alt")
                .map(str::to_string)
                .unwrap_or_else(|| payload.title.trim().to_string());
            let mut image = json!({"alt": plain_text(&alt, 1000).0, "image": blob});
            if let (Some(w), Some(h)) = (fitted.width, fitted.height) {
                image["aspectRatio"] = json!({"width": w, "height": h});
            }
            embed = json!({"$type": "app.bsky.embed.images", "images": [image]});
        }

        let mut record = json!({
            "$type": "app.bsky.feed.post",
            "text": text,
            "createdAt": iso_time((ctx.now)()),
        });
        let facets = link_facets(&text);
        if !facets.is_empty() {
            record["facets"] = Value::Array(facets);
        }
        if !embed.is_null() {
            record["embed"] = embed;
        }
        let req = self
            .call(&session, ctx, "com.atproto.repo.createRecord")?
            .body(Body::json(&json!({
                "repo": session.did,
                "collection": "app.bsky.feed.post",
                "record": record,
            })));
        ctx.progress.set("bluesky.posted", "1");
        let resp = match ctx.http.send(&req) {
            Ok(r) => r,
            Err(e) => {
                let mut err = from_net(&e);
                err.kind = FailureKind::Rejected(0);
                err.message = format!(
                    "{}. Bluesky may or may not have posted it, so CargoFrame did not retry: check your profile",
                    err.message
                );
                return Err(err);
            }
        };
        if !resp.is_success() {
            let err = from_response(&resp, "post to Bluesky");
            // a refusal posted nothing, so a later attempt may try again
            if resp.status < 500 {
                ctx.progress.clear("bluesky.posted");
            }
            return Err(err);
        }
        let url = resp
            .json()
            .and_then(|v| v.get("uri").and_then(Value::as_str).map(String::from))
            .and_then(|uri| post_url(&uri, &session.handle));
        Ok(SendOutcome {
            summary: format!("Posted to Bluesky as @{}", session.handle),
            url,
            notes,
            place: None,
        })
    }
}

/// Seconds since 1970 as `2026-11-08T10:30:00Z`.
pub fn iso_time(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // the civil date from a day count (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests;
