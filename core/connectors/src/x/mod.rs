//! X (Twitter): a post with the screenshot. The person makes a "Native App" in the X developer
//! portal (OAuth 2.0, no client secret), adds the callback `http://127.0.0.1:<port>/callback`,
//! pastes its Client ID, and signs in once in the browser. The refresh token is kept in the
//! credential store; X gives a new one each time it is used.
//!
//! A share uploads the picture (`/2/media/upload`, shrunk under 5 MB), then writes the post
//! (`/2/tweets`). The post is never written twice: if an earlier attempt may have posted it, it
//! is not retried.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid};
use crate::media::{fit, plain_text};
use crate::model;
use crate::oauth::{self, ClientAuth, OAuthClient, Redirect};
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, Status};
use cargoframe_net::{Body, Method, Multipart, Request};
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
}

impl XConnector {
    pub fn new(settings: model::X) -> Self {
        Self {
            settings,
            ep: Endpoints::default(),
            wait: Duration::from_secs(180),
        }
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
            return Err(crate::error::not_set_up(
                "X needs the Client ID of your app: add it to the connection",
            ));
        }
        let token = oauth::access_token(&self.client(), ctx)?;
        Ok(Request::new(method, format!("{}{path}", self.ep.api))
            .header("Authorization", format!("Bearer {token}"))
            .timeout(ctx.timeout))
    }
}

/// X's answer to a refused post often names the reason in `detail` or `errors[0].message`.
fn refusal(resp: &cargoframe_net::Response, what: &str) -> SendError {
    let mut err = from_response(resp, what);
    if resp.status == 403 {
        err.message.push_str(
            ". If the app's access is read only, set it to read and write in the X developer portal, then sign in again",
        );
    }
    err
}

impl Connector for XConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint| FieldSpec {
            key,
            label,
            hint,
            list: None,
        };
        Describe {
            fields: vec![
                f("text", "Text", "The post, at most 280 characters. Defaults to the prompt, then the window title"),
                f("alt", "Picture description", "Read out to people who cannot see the picture"),
            ],
            lists: Vec::new(),
            summary: "Posts the text and the screenshot (made smaller when it is over 5 MB) to your X account. Formatting is shown as plain text. Files and videos are not sent.".into(),
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
            return Err(crate::error::not_set_up(
                "Paste the Client ID of your X app first",
            ));
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
            return Err(refusal(&resp, "check the sign-in"));
        }
        let name = resp
            .json()
            .and_then(|v| {
                v.pointer("/data/username")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .unwrap_or_else(|| "your account".into());
        Ok(format!("Signed in to X as @{name}"))
    }

    fn options(&self, _ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        Err(invalid(format!("X has no list called \"{list}\"")))
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        if ctx.progress.done("x.posted") {
            return Err(invalid(
                "An earlier attempt may already have posted this. Check your X profile before trying again",
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
                "The text was cut to {MAX_TEXT} characters, the most X shows"
            ));
        }
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
            notes.push("X gets the screenshot, not the recording".to_string());
        }
        if payload.files_zip.is_some() {
            notes.push("X cannot take files, so the ticked files were not sent".to_string());
        }
        if picture.is_none() && text.trim().is_empty() {
            return Err(invalid("There is nothing to post: no text and no picture"));
        }

        // Step 1: the picture, once.
        let mut media_id = ctx.progress.get("x.media");
        if let (Some(pic), None) = (picture, &media_id) {
            let fitted = fit(pic, MAX_PICTURE, "X").map_err(invalid)?;
            if fitted.shrunk {
                notes.push("The picture was made smaller to fit X's 5 MB".to_string());
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
                return Err(refusal(&resp, "upload the picture to X"));
            }
            let id = resp
                .json()
                .and_then(|v| {
                    v.pointer("/data/id")
                        .and_then(Value::as_str)
                        .map(String::from)
                })
                .ok_or_else(|| invalid("X did not say which picture it kept"))?;
            if let Some(alt) = payload.value("alt") {
                // a description is a nicety: a failure here does not stop the post
                let alt_req = self
                    .api(ctx, Method::Post, "/2/media/metadata")?
                    .body(Body::json(&json!({
                        "id": id,
                        "metadata": {"alt_text": {"text": plain_text(alt, 1000).0}},
                    })));
                if !ctx.http.send(&alt_req).is_ok_and(|r| r.is_success()) {
                    notes.push("The picture description could not be added".to_string());
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
                err.message = format!(
                    "{}. X may or may not have posted it, so CargoFrame did not retry: check your profile",
                    err.message
                );
                return Err(err);
            }
        };
        if !resp.is_success() {
            if resp.status < 500 {
                ctx.progress.clear("x.posted");
            }
            return Err(refusal(&resp, "post to X"));
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
            summary: "Posted to X".into(),
            url,
            notes,
            place: None,
        })
    }
}

#[cfg(test)]
mod tests;
