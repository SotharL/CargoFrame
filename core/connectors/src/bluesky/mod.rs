//! Bluesky: a post with the screenshot. The person types their handle and an *app password*
//! (Settings, Privacy and security, App passwords in Bluesky): no developer app to register.
//!
//! Each share signs in (`createSession`), uploads the picture (`uploadBlob`, shrunk to fit) or the
//! recording, and writes the post (`createRecord`). A recording goes to Bluesky's video service,
//! as Bluesky recommends: a token from the account's own server (`getServiceAuth`), the upload
//! (`app.bsky.video.uploadVideo`), then the job's status until the video is ready; the post holds
//! it as a video. Links in the text are marked so Bluesky shows them as links. The post is never
//! written twice: if an earlier attempt may have posted it, it is not retried.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::media::{fit, mp4_size, plain_text};
use crate::model;
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, Status};
use cargoframe_i18n::{msg, t, tf};
use cargoframe_net::{check_url, encode_component, Body, Method, Request};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};

/// Most characters in a post (Bluesky counts 300 graphemes; a character is never more than one).
pub const MAX_TEXT: usize = 300;
/// Largest picture Bluesky keeps.
pub const MAX_PICTURE: usize = 1_000_000;
/// Largest video Bluesky's video service takes.
pub const MAX_VIDEO: usize = 100 * 1024 * 1024;
/// Bluesky's video service.
const VIDEO_SERVICE: &str = "https://video.bsky.app";
/// How long the video service may take before the share gives up (it can be retried).
const MAX_PROCESSING_SECS: u32 = 300;

pub struct BlueskyConnector {
    settings: model::Bluesky,
    video: String,
    /// A second between two looks at the video job (shorter in tests).
    second: std::time::Duration,
}

/// A signed-in session: who, and the key for the next calls.
struct Session {
    did: String,
    handle: String,
    jwt: String,
    /// The host of the account's own server (its PDS), which the video service uploads to.
    pds: String,
}

impl BlueskyConnector {
    pub fn new(settings: model::Bluesky) -> Self {
        Self {
            settings,
            video: VIDEO_SERVICE.into(),
            second: std::time::Duration::from_secs(1),
        }
    }

    /// Another video service, and no wait between looks (tests).
    #[cfg(test)]
    pub fn with_video_service(mut self, url: String) -> Self {
        self.video = url;
        self.second = std::time::Duration::ZERO;
        self
    }

    fn service(&self) -> Result<String, SendError> {
        let s = self.settings.service.trim().trim_end_matches('/');
        let s = if s.is_empty() {
            "https://bsky.social"
        } else {
            s
        };
        check_url(s).map_err(|e| invalid(tf("The Bluesky server address: {e}", &[("e", &e)])))?;
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
            .ok_or_else(|| {
                not_set_up(t("Bluesky needs an app password: add it to the connection"))
            })?;
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
            return Err(SendError::new(FailureKind::AuthExpired, t("Bluesky did not accept the handle or the app password: check both, or make a new app password")));
        }
        if !resp.is_success() {
            return Err(from_response(&resp, msg!("sign in to Bluesky")));
        }
        let v = resp
            .json()
            .ok_or_else(|| invalid(t("Bluesky answered the sign-in with something unreadable")))?;
        let get = |k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        // the account's own server, from its DID document; else the one signed in to
        let pds = v
            .pointer("/didDoc/service")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|s| {
                s.get("id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| id.ends_with("#atproto_pds"))
            })
            .and_then(|s| s.get("serviceEndpoint").and_then(Value::as_str))
            .map(str::to_string)
            .unwrap_or(self.service()?);
        let pds = host_of(&pds);
        match (get("did"), get("accessJwt")) {
            (Some(did), Some(jwt)) => Ok(Session {
                did,
                handle: get("handle").unwrap_or(handle),
                jwt,
                pds,
            }),
            _ => Err(invalid(t("Bluesky's sign-in answer had no account"))),
        }
    }

    /// The recording through Bluesky's video service, and the blob to put in the post once the
    /// service has processed it. A job already started is looked up again rather than redone.
    fn upload_video(
        &self,
        ctx: &Ctx,
        session: &Session,
        clip: &crate::payload::Attachment,
    ) -> Result<Value, SendError> {
        if clip.data.len() > MAX_VIDEO {
            return Err(invalid(t(
                "The recording is larger than Bluesky takes (100 MB): trim it in the window",
            )));
        }
        let job = match ctx.progress.get("bluesky.job") {
            Some(job) => job,
            None => {
                // a token for the video service to put the video in the account's own server
                let exp = (ctx.now)() + 30 * 60;
                let req = Request::new(
                    Method::Get,
                    format!(
                        "{}/xrpc/com.atproto.server.getServiceAuth?aud={}&lxm=com.atproto.repo.uploadBlob&exp={exp}",
                        self.service()?,
                        encode_component(&format!("did:web:{}", session.pds)),
                    ),
                )
                .header("Authorization", format!("Bearer {}", session.jwt))
                .timeout(ctx.timeout);
                let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
                if !resp.is_success() {
                    return Err(from_response(
                        &resp,
                        msg!("get a key for Bluesky's video service"),
                    ));
                }
                let token = resp
                    .json()
                    .and_then(|v| v.get("token").and_then(Value::as_str).map(String::from))
                    .ok_or_else(|| invalid(t("Bluesky gave no key for its video service")))?;
                let req = Request::new(
                    Method::Post,
                    format!(
                        "{}/xrpc/app.bsky.video.uploadVideo?did={}&name={}",
                        self.video,
                        encode_component(&session.did),
                        encode_component(&clip.name),
                    ),
                )
                .header("Authorization", format!("Bearer {token}"))
                .timeout(ctx.timeout.max(std::time::Duration::from_secs(300)))
                .body(Body::Bytes {
                    content_type: clip.mime.clone(),
                    data: clip.data.clone(),
                });
                let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
                let v = resp.json().unwrap_or(Value::Null);
                // the same video sent before answers with its job too
                let status = v.get("jobStatus").unwrap_or(&v);
                let job = status
                    .get("jobId")
                    .and_then(Value::as_str)
                    .map(String::from);
                match job {
                    Some(job) if resp.is_success() || resp.status == 409 => {
                        if let Some(blob) = status.get("blob").filter(|b| b.is_object()) {
                            return Ok(blob.clone());
                        }
                        ctx.progress.set("bluesky.job", &job);
                        job
                    }
                    _ => return Err(from_response(&resp, msg!("upload the video to Bluesky"))),
                }
            }
        };
        for _ in 0..MAX_PROCESSING_SECS {
            let req = Request::new(
                Method::Get,
                format!(
                    "{}/xrpc/app.bsky.video.getJobStatus?jobId={}",
                    self.video,
                    encode_component(&job)
                ),
            )
            .timeout(ctx.timeout);
            let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
            if !resp.is_success() {
                return Err(from_response(
                    &resp,
                    msg!("ask Bluesky whether the video is ready"),
                ));
            }
            let v = resp.json().unwrap_or(Value::Null);
            let status = v.get("jobStatus").unwrap_or(&v);
            if let Some(blob) = status.get("blob").filter(|b| b.is_object()) {
                return Ok(blob.clone());
            }
            if status.get("state").and_then(Value::as_str) == Some("JOB_STATE_FAILED") {
                ctx.progress.clear("bluesky.job");
                let why = status
                    .get("message")
                    .or_else(|| status.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                return Err(invalid(if why.is_empty() {
                    t("Bluesky could not process the video").to_string()
                } else {
                    tf(
                        "Bluesky could not process the video: {why}",
                        &[("why", &why)],
                    )
                }));
            }
            if (ctx.cancelled)() {
                return Err(SendError::new(
                    FailureKind::Rejected(0),
                    t("The window was closed, so the upload stopped"),
                ));
            }
            std::thread::sleep(self.second);
        }
        Err(SendError::new(
            FailureKind::Timeout,
            t("Bluesky is still processing the video; try again in a minute"),
        ))
    }

    fn call(&self, session: &Session, ctx: &Ctx, method: &str) -> Result<Request, SendError> {
        Ok(
            Request::new(Method::Post, format!("{}/xrpc/{method}", self.service()?))
                .header("Authorization", format!("Bearer {}", session.jwt))
                .timeout(ctx.timeout),
        )
    }
}

/// `https://morel.us-east.host.bsky.network/` is `morel.us-east.host.bsky.network`.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .to_string()
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
        Err(cargoframe_i18n::t("Bluesky needs your handle, like jane.bsky.social").into())
    } else {
        Err(tf(
            "\"{raw}\" is not a Bluesky handle: it looks like jane.bsky.social",
            &[("raw", &raw)],
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

/// The hashtags of a post (`#crash`), so Bluesky shows them as tags: a word that starts with `#`
/// after a space or at the start, made of letters, digits and `_`.
pub fn tag_facets(text: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices('#') {
        let before = text[..at].chars().next_back();
        if before.is_some_and(|c| !c.is_whitespace()) {
            continue;
        }
        let rest = &text[at + 1..];
        let len = rest
            .find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let tag = &rest[..len];
        // a tag of digits only is not one (`#1`)
        if tag.is_empty() || tag.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        out.push(json!({
            "index": {"byteStart": at, "byteEnd": at + 1 + len},
            "features": [{"$type": "app.bsky.richtext.facet#tag", "tag": tag}],
        }));
    }
    out
}

#[cfg(test)]
#[test]
fn hashtags_are_marked_as_tags_by_their_bytes() {
    let f = tag_facets("Crash é #gpu_1 and#not #123 #ui");
    let tags: Vec<&str> = f
        .iter()
        .map(|v| v["features"][0]["tag"].as_str().unwrap())
        .collect();
    assert_eq!(tags, ["gpu_1", "ui"]);
    // é is two bytes
    assert_eq!(f[0]["index"]["byteStart"], 9);
    assert_eq!(f[0]["index"]["byteEnd"], 15);
}

/// `at://did:plc:abc/app.bsky.feed.post/3k4` and a handle make the post's web address.
pub fn post_url(uri: &str, handle: &str) -> Option<String> {
    let rkey = uri.rsplit('/').next().filter(|k| !k.is_empty())?;
    uri.contains("/app.bsky.feed.post/")
        .then(|| format!("https://bsky.app/profile/{handle}/post/{rkey}"))
}

impl Connector for BlueskyConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint| FieldSpec::new(key, label, hint, None);
        Describe {
            fields: vec![
                f("text", msg!("Text"), msg!("The post, at most 300 characters. Defaults to the prompt, then the window title")),
                f("alt", msg!("Picture description"), msg!("Read out to people who cannot see the picture. Defaults to the window title")),
                f("hashtags", msg!("Hashtags"), msg!("Added at the end as #tags. Separate several with commas")),
            ],
            lists: Vec::new(),
            summary: msg!("Posts the text and the screenshot (made smaller when it is over 1 MB), or the recording, to your Bluesky account. Links become links; formatting is shown as plain text. Files are not sent.").into(),
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
        Ok(tf(
            "Signed in to Bluesky as @{handle}",
            &[("handle", &s.handle)],
        ))
    }

    fn options(&self, _ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        Err(invalid(tf(
            "Bluesky has no list called \"{list}\"",
            &[("list", &list)],
        )))
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        if ctx.progress.done("bluesky.posted") {
            return Err(invalid(t("An earlier attempt may already have posted this. Check your Bluesky profile before trying again")));
        }
        let mut notes = Vec::new();
        let source = payload
            .value("text")
            .map(str::to_string)
            .or_else(|| payload.prompt.clone().filter(|p| !p.trim().is_empty()))
            .unwrap_or_else(|| payload.title.clone());
        let (text, cut) = crate::payload::with_hashtags(&source, payload, MAX_TEXT);
        if cut {
            notes.push(tf(
                "The text was cut to {MAX_TEXT} characters, the most Bluesky shows",
                &[("MAX_TEXT", &MAX_TEXT)],
            ));
        }

        // A post holds a video or pictures: the recording when there is one, else the screenshot
        // (or the still that goes with a recording).
        let video = payload
            .image
            .as_ref()
            .filter(|a| a.mime.starts_with("video/"));
        let picture = payload
            .image
            .as_ref()
            .filter(|a| a.mime.starts_with("image/"))
            .or(payload.still.as_ref())
            .filter(|_| video.is_none());
        if video.is_some() && payload.still.is_some() {
            notes.push(
                t("Bluesky takes a video or pictures in a post, not both: the recording was sent")
                    .to_string(),
            );
        }
        if payload.files_zip.is_some() {
            notes.push(
                t("Bluesky cannot take files, so the ticked files were not sent").to_string(),
            );
        }
        if picture.is_none() && video.is_none() && text.trim().is_empty() {
            return Err(invalid(t(
                "There is nothing to post: no text and no picture",
            )));
        }

        let session = self.sign_in(ctx)?;
        let mut embed = Value::Null;
        if let Some(clip) = video {
            let blob = match ctx
                .progress
                .get("bluesky.blob")
                .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            {
                Some(blob) => blob,
                None => {
                    let blob = self.upload_video(ctx, &session, clip)?;
                    ctx.progress.set("bluesky.blob", &blob.to_string());
                    blob
                }
            };
            let alt = payload
                .value("alt")
                .map(str::to_string)
                .unwrap_or_else(|| payload.title.trim().to_string());
            embed = json!({"$type": "app.bsky.embed.video", "video": blob, "alt": plain_text(&alt, 1000).0});
            if let Some((w, h)) = mp4_size(&clip.data) {
                embed["aspectRatio"] = json!({"width": w, "height": h});
            }
        }
        if let Some(pic) = picture {
            let fitted = fit(pic, MAX_PICTURE, "Bluesky").map_err(invalid)?;
            if fitted.shrunk {
                notes.push(t("The picture was made smaller to fit Bluesky's 1 MB").to_string());
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
                    return Err(from_response(&resp, msg!("upload the picture to Bluesky")));
                }
                let blob = resp
                    .json()
                    .and_then(|v| v.get("blob").cloned())
                    .ok_or_else(|| invalid(t("Bluesky did not say where it kept the picture")))?;
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
        let mut facets = link_facets(&text);
        facets.extend(tag_facets(&text));
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
                err.message = tf("{message}. Bluesky may or may not have posted it, so CargoFrame did not retry: check your profile", &[("message", &err.message)]);
                return Err(err);
            }
        };
        if !resp.is_success() {
            let err = from_response(&resp, msg!("post to Bluesky"));
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
            summary: tf(
                "Posted to Bluesky as @{handle}",
                &[("handle", &session.handle)],
            ),
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
