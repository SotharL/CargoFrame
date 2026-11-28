//! X (Twitter): a post with the screenshot. The person makes a "Native App" in the X developer
//! portal (OAuth 2.0, no client secret), adds the callback `http://127.0.0.1:<port>/callback`,
//! pastes its Client ID, and signs in once in the browser. The refresh token is kept in the
//! credential store; X gives a new one each time it is used.
//!
//! A share uploads the picture (`/2/media/upload`, shrunk under 5 MB) or the recording (the
//! chunked upload: `/2/media/upload/initialize`, `/{id}/append` in pieces, `/{id}/finalize`, then
//! the status until X has processed it), then writes the post (`/2/tweets`). A post holds a video
//! or pictures, not both: a recording goes as the video. The post is never written twice: if an
//! earlier attempt may have posted it, it is not retried.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid};
use crate::media::{fit, plain_text};
use crate::model;
use crate::oauth::{self, ClientAuth, OAuthClient, Redirect};
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, Status};
use cargoframe_i18n::{msg, t, tf};
use cargoframe_net::{encode_component, Body, Method, Multipart, Request};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};
use std::time::Duration;

const SCOPES: [&str; 5] = [
    "tweet.read",
    "tweet.write",
    "users.read",
    "media.write",
    "offline.access",
];

/// Most characters in a post on a free or Basic account.
pub const MAX_TEXT: usize = 280;
/// Largest picture X takes.
pub const MAX_PICTURE: usize = 5_000_000;
/// Each piece of a video upload; X takes up to 5 MB.
pub const CHUNK: usize = 4 * 1024 * 1024;
/// Largest video a free account may post (X's limit for `tweet_video`).
pub const MAX_VIDEO: usize = 512 * 1024 * 1024;
/// How long X may take to process a video before the share gives up (it can be retried).
const MAX_PROCESSING_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub auth: String,
    pub token: String,
    pub revoke: String,
    pub api: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth: "https://x.com/i/oauth2/authorize".into(),
            token: "https://api.x.com/2/oauth2/token".into(),
            revoke: "https://api.x.com/2/oauth2/revoke".into(),
            api: "https://api.x.com".into(),
        }
    }
}

pub struct XConnector {
    settings: model::X,
    ep: Endpoints,
    wait: Duration,
    /// One second of X's `check_after_secs` (shorter in tests).
    second: Duration,
}

impl XConnector {
    pub fn new(settings: model::X) -> Self {
        Self {
            settings,
            ep: Endpoints::default(),
            wait: Duration::from_secs(180),
            second: Duration::from_secs(1),
        }
    }

    /// Wait this long for each second X asks to wait (tests: none).
    #[cfg(test)]
    pub fn with_second(mut self, second: Duration) -> Self {
        self.second = second;
        self
    }

    pub fn with_endpoints(mut self, ep: Endpoints, wait: Duration) -> Self {
        self.ep = ep;
        self.wait = wait;
        self
    }

    fn client(&self) -> OAuthClient {
        OAuthClient {
            auth_url: self.ep.auth.clone(),
            token_url: self.ep.token.clone(),
            revoke_url: Some(self.ep.revoke.clone()),
            client_id: self.settings.client_id.trim().to_string(),
            client_secret: None,
            scopes: SCOPES.iter().map(|s| s.to_string()).collect(),
            extra_auth_params: Vec::new(),
            pkce: true,
            client_auth: ClientAuth::Body,
            redirect_port: Some(self.settings.redirect_port),
            redirect: Redirect::Ip,
            wait: self.wait,
        }
    }

    fn api(&self, ctx: &Ctx, method: Method, path: &str) -> Result<Request, SendError> {
        if self.settings.client_id.trim().is_empty() {
            return Err(crate::error::not_set_up(t(
                "X needs the Client ID of your app: add it to the connection",
            )));
        }
        let token = oauth::access_token(&self.client(), ctx)?;
        Ok(Request::new(method, format!("{}{path}", self.ep.api))
            .header("Authorization", format!("Bearer {token}"))
            .timeout(ctx.timeout))
    }

    /// The recording through X's chunked upload, and its id once X has processed it. An upload
    /// cut off starts again from the beginning; one that X is still processing is waited for.
    fn upload_video(
        &self,
        ctx: &Ctx,
        clip: &crate::payload::Attachment,
    ) -> Result<String, SendError> {
        if clip.data.len() > MAX_VIDEO {
            return Err(invalid(t(
                "The recording is larger than X takes (512 MB): trim it in the window",
            )));
        }
        let id = match ctx.progress.get("x.video") {
            // uploaded before: only the processing is left to wait for
            Some(id) => id,
            None => {
                let init = self
                    .api(ctx, Method::Post, "/2/media/upload/initialize")?
                    .body(Body::json(&json!({
                        "media_type": clip.mime,
                        "total_bytes": clip.data.len(),
                        "media_category": "tweet_video",
                    })));
                let resp = ctx.http.send(&init).map_err(|e| from_net(&e))?;
                if !resp.is_success() {
                    return Err(refusal(&resp, msg!("start the video upload to X")));
                }
                let id = data_id(&resp)
                    .ok_or_else(|| invalid(t("X did not say where to upload the video")))?;
                let id = encode_component(&id);
                for (n, piece) in clip.data.chunks(CHUNK).enumerate() {
                    if (ctx.cancelled)() {
                        return Err(stopped());
                    }
                    let form = Multipart::form_data()
                        .field("segment_index", &n.to_string())
                        .file("media", &clip.name, &clip.mime, piece.to_vec());
                    let req = self
                        .api(ctx, Method::Post, &format!("/2/media/upload/{id}/append"))?
                        .body(Body::multipart(form));
                    let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
                    if !resp.is_success() {
                        return Err(refusal(&resp, msg!("upload the video to X")));
                    }
                }
                let req = self.api(ctx, Method::Post, &format!("/2/media/upload/{id}/finalize"))?;
                let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
                if !resp.is_success() {
                    return Err(refusal(&resp, msg!("finish the video upload to X")));
                }
                ctx.progress.set("x.video", &id);
                let ready = resp
                    .json()
                    .is_none_or(|v| v.pointer("/data/processing_info").is_none());
                if ready {
                    return Ok(id);
                }
                id
            }
        };
        // X processes the video before it can be posted: ask how far it is, as often as it says
        let mut waited = 0u64;
        loop {
            let req = self.api(
                ctx,
                Method::Get,
                &format!("/2/media/upload?command=STATUS&media_id={id}"),
            )?;
            let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
            if !resp.is_success() {
                return Err(refusal(&resp, msg!("ask X whether the video is ready")));
            }
            let info = resp
                .json()
                .and_then(|v| v.pointer("/data/processing_info").cloned());
            let state = info
                .as_ref()
                .and_then(|i| i.get("state"))
                .and_then(Value::as_str)
                .unwrap_or("succeeded");
            match state {
                "succeeded" => return Ok(id),
                "failed" => {
                    ctx.progress.clear("x.video");
                    let why = info
                        .as_ref()
                        .and_then(|i| i.pointer("/error/message"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    return Err(invalid(if why.is_empty() {
                        t("X could not process the video").to_string()
                    } else {
                        tf("X could not process the video: {why}", &[("why", &why)])
                    }));
                }
                _ => {}
            }
            let after = info
                .as_ref()
                .and_then(|i| i.get("check_after_secs"))
                .and_then(Value::as_u64)
                .unwrap_or(2)
                .clamp(1, 30);
            waited += after;
            if waited > MAX_PROCESSING_SECS {
                return Err(SendError::new(
                    FailureKind::Timeout,
                    t("X is still processing the video; try again in a minute"),
                ));
            }
            if (ctx.cancelled)() {
                return Err(stopped());
            }
            std::thread::sleep(self.second * after as u32);
        }
    }
}

/// The window was closed during an upload: stop, and do not retry.
fn stopped() -> SendError {
    SendError::new(
        FailureKind::Rejected(0),
        t("The window was closed, so the upload stopped"),
    )
}

/// `data.id` of an answer from X's media endpoints.
fn data_id(resp: &cargoframe_net::Response) -> Option<String> {
    resp.json().and_then(|v| {
        v.pointer("/data/id")
            .and_then(Value::as_str)
            .map(String::from)
    })
}

/// X's answer to a refused post often names the reason in `detail` or `errors[0].message`.
fn refusal(resp: &cargoframe_net::Response, what: &str) -> SendError {
    let mut err = from_response(resp, what);
    if resp.status == 403 {
        err.message.push_str(
            t(". If the app's access is read only, set it to read and write in the X developer portal, then sign in again"),
        );
    }
    err
}

impl Connector for XConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint| FieldSpec::new(key, label, hint, None);
        Describe {
            fields: vec![
                f("text", msg!("Text"), msg!("The post, at most 280 characters. Defaults to the prompt, then the window title")),
                f("alt", msg!("Picture description"), msg!("Read out to people who cannot see the picture")),
                f("hashtags", msg!("Hashtags"), msg!("Added at the end as #tags. Separate several with commas")),
            ],
            lists: Vec::new(),
            summary: msg!("Posts the text and the screenshot (made smaller when it is over 5 MB) to your X account. Formatting is shown as plain text. Files and videos are not sent.").into(),
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
            return Err(crate::error::not_set_up(t(
                "Paste the Client ID of your X app first",
            )));
        }
        oauth::authorize(&self.client(), ctx)?;
        self.test(ctx)
    }

    fn disconnect(&self, ctx: &Ctx) {
        oauth::sign_out(&self.client(), ctx);
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let resp = ctx
            .http
            .send(&self.api(ctx, Method::Get, "/2/users/me")?)
            .map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(refusal(&resp, msg!("check the sign-in")));
        }
        let name = resp
            .json()
            .and_then(|v| {
                v.pointer("/data/username")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| t("your account").into());
        Ok(tf("Signed in to X as @{name}", &[("name", &name)]))
    }

    fn options(&self, _ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        Err(invalid(tf(
            "X has no list called \"{list}\"",
            &[("list", &list)],
        )))
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        if ctx.progress.done("x.posted") {
            return Err(invalid(t("An earlier attempt may already have posted this. Check your X profile before trying again")));
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
                "The text was cut to {MAX_TEXT} characters, the most X shows",
                &[("MAX_TEXT", &MAX_TEXT)],
            ));
        }
        // a post holds a video or pictures: the recording goes when there is one
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
                t("X takes a video or pictures in a post, not both: the recording was sent")
                    .to_string(),
            );
        }
        if payload.files_zip.is_some() {
            notes.push(t("X cannot take files, so the ticked files were not sent").to_string());
        }
        if picture.is_none() && video.is_none() && text.trim().is_empty() {
            return Err(invalid(t(
                "There is nothing to post: no text and no picture",
            )));
        }

        // Step 1: the recording or the picture, once.
        let mut media_id = ctx.progress.get("x.media");
        if let (Some(clip), None) = (video, &media_id) {
            let id = self.upload_video(ctx, clip)?;
            ctx.progress.set("x.media", &id);
            media_id = Some(id);
        }
        if let (Some(pic), None) = (picture, &media_id) {
            let fitted = fit(pic, MAX_PICTURE, "X").map_err(invalid)?;
            if fitted.shrunk {
                notes.push(t("The picture was made smaller to fit X's 5 MB").to_string());
            }
            let form = Multipart::form_data()
                .field("media_category", "tweet_image")
                .file(
                    "media",
                    &fitted.picture.name,
                    &fitted.picture.mime,
                    fitted.picture.data.clone(),
                );
            let req = self
                .api(ctx, Method::Post, "/2/media/upload")?
                .body(Body::multipart(form));
            let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
            if !resp.is_success() {
                return Err(refusal(&resp, msg!("upload the picture to X")));
            }
            let id = resp
                .json()
                .and_then(|v| {
                    v.pointer("/data/id")
                        .and_then(Value::as_str)
                        .map(String::from)
                })
                .ok_or_else(|| invalid(t("X did not say which picture it kept")))?;
            if let Some(alt) = payload.value("alt") {
                // a description is a nicety: a failure here does not stop the post
                let alt_req = self
                    .api(ctx, Method::Post, "/2/media/metadata")?
                    .body(Body::json(&json!({
                        "id": id,
                        "metadata": {"alt_text": {"text": plain_text(alt, 1000).0}},
                    })));
                if !ctx.http.send(&alt_req).is_ok_and(|r| r.is_success()) {
                    notes.push(t("The picture description could not be added").to_string());
                }
            }
            ctx.progress.set("x.media", &id);
            media_id = Some(id);
        }

        // Step 2: the post, never repeated when an earlier attempt may have gone through.
        let mut body = json!({"text": text});
        if let Some(id) = &media_id {
            body["media"] = json!({"media_ids": [id]});
        }
        let req = self
            .api(ctx, Method::Post, "/2/tweets")?
            .body(Body::json(&body));
        ctx.progress.set("x.posted", "1");
        let resp = match ctx.http.send(&req) {
            Ok(r) => r,
            Err(e) => {
                let mut err = from_net(&e);
                err.kind = FailureKind::Rejected(0);
                err.message = tf("{message}. X may or may not have posted it, so CargoFrame did not retry: check your profile", &[("message", &err.message)]);
                return Err(err);
            }
        };
        if !resp.is_success() {
            if resp.status < 500 {
                ctx.progress.clear("x.posted");
            }
            return Err(refusal(&resp, msg!("post to X")));
        }
        let url = resp
            .json()
            .and_then(|v| {
                v.pointer("/data/id")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .filter(|id| id.chars().all(|c| c.is_ascii_digit()))
            .map(|id| format!("https://x.com/i/web/status/{id}"));
        Ok(SendOutcome {
            summary: t("Posted to X").into(),
            url,
            notes,
            place: None,
        })
    }
}

#[cfg(test)]
mod tests;
