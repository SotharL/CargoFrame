//! Instagram: a post with the screenshot, on a professional (business or creator) account.
//!
//! Instagram takes a picture only from a public web address: it fetches the file itself and
//! has no way to receive one. So a share first puts the picture, as a JPEG, on a web server the
//! person or their company owns (an HTTP PUT to the *upload address*), then asks Instagram to
//! make a post from the *public address* of that file, waits until Instagram has fetched it, and
//! publishes it.
//!
//! Sign-in is an access token made in the Meta app dashboard (Instagram API, *Generate token*).
//! It lasts 60 days and is renewed for another 60 each time the connection is tested or used.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::media::{plain_text, to_jpeg};
use crate::model;
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, Status};
use cargoframe_net::{check_url, encode_component, Body, Method, Request};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Value};
use std::time::Duration;

/// Most characters in a caption.
pub const MAX_CAPTION: usize = 2200;
/// Largest picture Instagram takes.
pub const MAX_PICTURE: usize = 8_000_000;

#[derive(Debug, Clone)]
pub struct Endpoints {
    /// The Graph API, with its version.
    pub graph: String,
    /// Where a token is renewed (not versioned).
    pub refresh: String,
    /// How long to wait between two looks at whether Instagram has fetched the picture.
    pub poll: Duration,
    /// How many looks before giving up.
    pub polls: u32,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            graph: "https://graph.instagram.com/v23.0".into(),
            refresh: "https://graph.instagram.com/refresh_access_token".into(),
            poll: Duration::from_secs(2),
            polls: 30,
        }
    }
}

pub struct InstagramConnector {
    settings: model::Instagram,
    ep: Endpoints,
}

impl InstagramConnector {
    pub fn new(settings: model::Instagram) -> Self {
        Self {
            settings,
            ep: Endpoints::default(),
        }
    }

    pub fn with_endpoints(mut self, ep: Endpoints) -> Self {
        self.ep = ep;
        self
    }

    fn token(&self, ctx: &Ctx) -> Result<String, SendError> {
        ctx.secret("token")?
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| not_set_up("Instagram needs an access token: add it to the connection"))
    }

    fn graph(&self, ctx: &Ctx, method: Method, path: &str) -> Result<Request, SendError> {
        Ok(Request::new(method, format!("{}{path}", self.ep.graph))
            .header("Authorization", format!("Bearer {}", self.token(ctx)?))
            .timeout(ctx.timeout))
    }

    fn get_json(&self, ctx: &Ctx, path: &str, what: &str) -> Result<Value, SendError> {
        let resp = ctx
            .http
            .send(&self.graph(ctx, Method::Get, path)?)
            .map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(refusal(&resp, what));
        }
        resp.json().ok_or_else(|| {
            invalid(format!(
                "Instagram answered {what} with something unreadable"
            ))
        })
    }

    /// The account's id and name.
    fn me(&self, ctx: &Ctx) -> Result<(String, String), SendError> {
        let v = self.get_json(ctx, "/me?fields=user_id,username", "check the sign-in")?;
        let id = v
            .get("user_id")
            .or_else(|| v.get("id"))
            .and_then(|i| {
                i.as_str()
                    .map(String::from)
                    .or_else(|| i.as_u64().map(|n| n.to_string()))
            })
            .ok_or_else(|| invalid("Instagram did not say which account the token is for"))?;
        let name = v
            .get("username")
            .and_then(Value::as_str)
            .unwrap_or("your account")
            .to_string();
        Ok((id, name))
    }

    /// Renew the token for another 60 days. Best effort: a token that cannot be renewed yet
    /// (less than a day old) still works.
    fn renew(&self, ctx: &Ctx) {
        let Ok(token) = self.token(ctx) else {
            return;
        };
        let url = format!(
            "{}?grant_type=ig_refresh_token&access_token={}",
            self.ep.refresh,
            encode_component(&token)
        );
        let Ok(resp) = ctx.http.send(&Request::get(url).timeout(ctx.timeout)) else {
            return;
        };
        if let Some(fresh) = resp
            .is_success()
            .then(|| resp.json())
            .flatten()
            .and_then(|v| {
                v.get("access_token")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
        {
            let _ = ctx.set_secret("token", &fresh);
        }
    }

    /// Put the picture on the person's own server; returns its public address.
    fn host(&self, ctx: &Ctx, jpeg: &[u8]) -> Result<String, SendError> {
        let (upload, public) = (
            self.settings.upload_url.trim(),
            self.settings.public_url.trim(),
        );
        if !upload.contains("{name}") || !public.contains("{name}") {
            return Err(not_set_up(
                "Instagram needs the upload address and the public address of your picture server, each with {name} in it",
            ));
        }
        let name = format!("cargoframe-{}.jpg", random_hex(8));
        let upload = upload.replace("{name}", &name);
        let public = public.replace("{name}", &name);
        check_url(&upload).map_err(|e| invalid(format!("The upload address: {e}")))?;
        check_url(&public).map_err(|e| invalid(format!("The public address: {e}")))?;
        let mut req = Request::new(Method::Put, upload)
            .timeout(ctx.timeout)
            .body(Body::Bytes {
                content_type: "image/jpeg".into(),
                data: jpeg.to_vec(),
            });
        let header = self.settings.host_header.trim();
        if !header.is_empty() {
            let key = ctx
                .secret("host_key")?
                .filter(|k| !k.trim().is_empty())
                .ok_or_else(|| {
                    not_set_up(format!(
                        "The picture server needs its key (sent as {header}): add it to the connection"
                    ))
                })?;
            req = req.header(header, key.trim());
        }
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(from_response(
                &resp,
                "put the picture on your picture server",
            ));
        }
        Ok(public)
    }
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    let _ = getrandom::fill(&mut buf);
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Meta's errors carry the reason in `error.message`; a few need a sentence of their own.
fn refusal(resp: &cargoframe_net::Response, what: &str) -> SendError {
    let mut err = from_response(resp, what);
    let code = resp
        .json()
        .and_then(|v| v.pointer("/error/code").and_then(Value::as_u64));
    if code == Some(190) {
        err.kind = FailureKind::AuthExpired;
        err.message = "The Instagram token has expired: make a new one in the Meta app dashboard and paste it into the connection".into();
    }
    err
}

impl Connector for InstagramConnector {
    fn describe(&self) -> Describe {
        Describe {
            fields: vec![FieldSpec {
                key: "caption",
                label: "Caption",
                hint: "Up to 2,200 characters. Defaults to the prompt, then the window title",
                list: None,
            }],
            lists: Vec::new(),
            summary: "Posts the screenshot with a caption to your Instagram professional account. The picture is first put on your own picture server, because Instagram only takes pictures from a public address. Files and videos are not sent.".into(),
        }
    }

    fn status(&self, ctx: &Ctx) -> Status {
        if self.token(ctx).is_ok() {
            Status::Connected
        } else {
            Status::Disconnected
        }
    }

    fn disconnect(&self, ctx: &Ctx) {
        crate::secrets::delete_all(ctx.secrets, ctx.connection_id, &["token", "host_key"]);
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let (_, name) = self.me(ctx)?;
        self.renew(ctx);
        Ok(format!("Signed in to Instagram as @{name}"))
    }

    fn options(&self, _ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        Err(invalid(format!("Instagram has no list called \"{list}\"")))
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        if ctx.progress.done("instagram.published") {
            return Err(invalid(
                "An earlier attempt may already have posted this. Check your Instagram profile before trying again",
            ));
        }
        let mut notes = Vec::new();
        let picture = payload
            .image
            .as_ref()
            .filter(|a| a.mime.starts_with("image/"))
            .or(payload.still.as_ref())
            .ok_or_else(|| {
                invalid("An Instagram post needs a picture: add a screenshot to the layout")
            })?;
        if payload
            .image
            .as_ref()
            .is_some_and(|a| a.mime.starts_with("video/"))
        {
            notes.push("Instagram gets the screenshot, not the recording".to_string());
        }
        if payload.files_zip.is_some() {
            notes
                .push("Instagram cannot take files, so the ticked files were not sent".to_string());
        }
        let source = payload
            .value("caption")
            .map(str::to_string)
            .or_else(|| payload.prompt.clone().filter(|p| !p.trim().is_empty()))
            .unwrap_or_else(|| payload.title.clone());
        let (caption, cut) = plain_text(&source, MAX_CAPTION);
        if cut {
            notes.push(format!("The caption was cut to {MAX_CAPTION} characters"));
        }

        let (account, name) = self.me(ctx)?;

        // Step 1: the picture on the person's server, once.
        let public = match ctx.progress.get("instagram.picture") {
            Some(url) => url,
            None => {
                let jpeg = to_jpeg(picture, MAX_PICTURE).map_err(invalid)?;
                if jpeg.shrunk {
                    notes.push("The picture was made smaller to fit Instagram's 8 MB".to_string());
                }
                let url = self.host(ctx, &jpeg.picture.data)?;
                ctx.progress.set("instagram.picture", &url);
                url
            }
        };

        // Step 2: Instagram fetches it into a draft ("container"), once.
        let container = match ctx.progress.get("instagram.container") {
            Some(id) => id,
            None => {
                let req = self
                    .graph(ctx, Method::Post, &format!("/{account}/media"))?
                    .body(Body::json(
                        &json!({"image_url": public, "caption": caption}),
                    ));
                let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
                if !resp.is_success() {
                    return Err(refusal(&resp, "give the picture to Instagram"));
                }
                let id = resp
                    .json()
                    .and_then(|v| v.get("id").and_then(Value::as_str).map(String::from))
                    .ok_or_else(|| invalid("Instagram did not say which draft it made"))?;
                ctx.progress.set("instagram.container", &id);
                id
            }
        };

        // Step 3: wait until Instagram has the picture.
        let mut ready = false;
        for look in 0..self.ep.polls {
            if look > 0 {
                std::thread::sleep(self.ep.poll);
            }
            let v = self.get_json(
                ctx,
                &format!("/{container}?fields=status_code"),
                "ask whether Instagram has the picture",
            )?;
            match v.get("status_code").and_then(Value::as_str) {
                Some("FINISHED") => {
                    ready = true;
                    break;
                }
                Some("ERROR") | Some("EXPIRED") => {
                    ctx.progress.clear("instagram.container");
                    return Err(invalid(
                        "Instagram could not fetch the picture from your picture server: check that its public address opens in a browser without signing in",
                    ));
                }
                _ => {}
            }
        }
        if !ready {
            return Err(SendError::new(
                FailureKind::Timeout,
                "Instagram is still fetching the picture; try again in a minute",
            ));
        }

        // Step 4: publish, never repeated when an earlier attempt may have gone through.
        let req = self
            .graph(ctx, Method::Post, &format!("/{account}/media_publish"))?
            .body(Body::json(&json!({"creation_id": container})));
        ctx.progress.set("instagram.published", "1");
        let resp = match ctx.http.send(&req) {
            Ok(r) => r,
            Err(e) => {
                let mut err = from_net(&e);
                err.kind = FailureKind::Rejected(0);
                err.message = format!(
                    "{}. Instagram may or may not have posted it, so CargoFrame did not retry: check your profile",
                    err.message
                );
                return Err(err);
            }
        };
        if !resp.is_success() {
            if resp.status < 500 {
                ctx.progress.clear("instagram.published");
            }
            return Err(refusal(&resp, "publish the post"));
        }
        let media = resp
            .json()
            .and_then(|v| v.get("id").and_then(Value::as_str).map(String::from));
        let url = media
            .and_then(|m| {
                self.get_json(
                    ctx,
                    &format!("/{m}?fields=permalink"),
                    "find the post's address",
                )
                .ok()
            })
            .and_then(|v| v.get("permalink").and_then(Value::as_str).map(String::from))
            .filter(|u| check_url(u).is_ok());
        self.renew(ctx);
        Ok(SendOutcome {
            summary: format!("Posted to Instagram as @{name}"),
            url,
            notes,
            place: None,
        })
    }
}

#[cfg(test)]
mod tests;
