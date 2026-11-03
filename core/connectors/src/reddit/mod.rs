//! Reddit: submit a text, link or image post. The person registers an "installed app" at
//! reddit.com/prefs/apps and signs in once through the browser; CargoFrame keeps only the
//! refresh token, in the credential store.
//!
//! Reddit answers many failures with HTTP 200 and an `errors` list, so the body is always read.
//! Posting is the one step that must not be repeated blindly: if its outcome is unknown (the
//! connection dropped after it was sent) it is not retried, so a post can never appear twice.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid};
use crate::model;
use crate::oauth::{self, ClientAuth, OAuthClient};
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use cargoframe_net::{check_url, Body, Method, Multipart, Request, Response};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::Value;
use std::time::Duration;

const SCOPES: [&str; 5] = ["identity", "submit", "mysubscriptions", "read", "flair"];

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
            auth: "https://www.reddit.com/api/v1/authorize".into(),
            token: "https://www.reddit.com/api/v1/access_token".into(),
            revoke: "https://www.reddit.com/api/v1/revoke_token".into(),
            api: "https://oauth.reddit.com".into(),
        }
    }
}

pub struct RedditConnector {
    settings: model::Reddit,
    ep: Endpoints,
    wait: Duration,
}

impl RedditConnector {
    pub fn new(settings: model::Reddit) -> Self {
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
            extra_auth_params: vec![("duration".into(), "permanent".into())],
            pkce: false,
            client_auth: ClientAuth::Basic,
            redirect_port: Some(self.settings.redirect_port),
            redirect: oauth::Redirect::Ip,
            wait: self.wait,
        }
    }

    fn api(&self, ctx: &Ctx, method: Method, path: &str) -> Result<Request, SendError> {
        let token = oauth::access_token(&self.client(), ctx)?;
        Ok(Request::new(method, format!("{}{path}", self.ep.api))
            .header("Authorization", format!("bearer {token}"))
            .header("Accept", "application/json")
            .timeout(ctx.timeout))
    }

    fn get_json(&self, ctx: &Ctx, path: &str, what: &str) -> Result<Value, SendError> {
        let resp = ctx
            .http
            .send(&self.api(ctx, Method::Get, path)?)
            .map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(from_response(&resp, what));
        }
        resp.json()
            .ok_or_else(|| invalid(format!("Reddit answered {what} with something unreadable")))
    }
}

/// `r/rust`, `/r/rust/` and `rust` all mean the subreddit `rust`.
pub fn clean_subreddit(raw: &str) -> Result<String, String> {
    let s = raw.trim().trim_matches('/');
    let s = s
        .strip_prefix("r/")
        .or_else(|| s.strip_prefix("R/"))
        .unwrap_or(s)
        .trim_matches('/');
    if (2..=21).contains(&s.len()) && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        Ok(s.to_string())
    } else {
        Err(format!(
            "\"{raw}\" is not a subreddit name (2 to 21 letters, digits or _)"
        ))
    }
}

/// "you are doing that too much. try again in 9 minutes." -> 9 minutes
pub fn parse_wait(message: &str) -> Option<Duration> {
    let lower = message.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for pair in words.windows(2) {
        if let Ok(n) = pair[0].parse::<u64>() {
            if pair[1].starts_with("minute") {
                return Some(Duration::from_secs(n * 60));
            }
            if pair[1].starts_with("second") {
                return Some(Duration::from_secs(n));
            }
        }
    }
    None
}

/// Reddit's `json.errors`, as a failure when there are any.
fn body_errors(resp: &Response) -> Option<SendError> {
    let json = resp.json()?;
    let errors = json.pointer("/json/errors")?.as_array()?;
    let first = errors.first()?.as_array()?;
    let code = first.first().and_then(Value::as_str).unwrap_or("ERROR");
    let text = first.get(1).and_then(Value::as_str).unwrap_or("");
    let message = format!("Reddit refused the post ({code}: {text})");
    if code == "RATELIMIT" {
        let mut e = SendError::new(FailureKind::Throttled(429), message);
        if let Some(w) = parse_wait(text) {
            e = e.with_retry_after(w);
        }
        return Some(e);
    }
    Some(invalid(message))
}

impl Connector for RedditConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint, list| FieldSpec {
            key,
            label,
            hint,
            list,
        };
        Describe {
            fields: vec![
                f("subreddit", "Subreddit", "Where to post, for example rust", Some("subscribed")),
                f("title", "Title", "Defaults to the window title", None),
                f("text", "Text", "The post body. Defaults to the prompt text", None),
                f("url", "Link", "Makes it a link post instead of an image", None),
                f("flair_id", "Flair", "", Some("flairs")),
                f("nsfw", "NSFW", "true to mark it", None),
                f("spoiler", "Spoiler", "true to mark it", None),
            ],
            lists: vec![
                ListSpec { id: "subscribed".into(), label: "My subscribed subreddits".into(), param: None },
                ListSpec { id: "flairs".into(), label: "Flairs of a subreddit".into(), param: Some("subreddit") },
            ],
            summary: "Posts the screenshot as an image post, or the text as a self post, or a link. Files cannot be attached on Reddit.".into(),
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
        oauth::authorize(&self.client(), ctx)?;
        self.test(ctx)
    }

    fn disconnect(&self, ctx: &Ctx) {
        oauth::sign_out(&self.client(), ctx);
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let me = self.get_json(ctx, "/api/v1/me", "check the sign-in")?;
        let name = me
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("your account");
        Ok(format!("Signed in to Reddit as u/{name}"))
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        let (name, param) = match list.split_once(':') {
            Some((n, p)) => (n, Some(p)),
            None => (list, None),
        };
        match name {
            "subscribed" => {
                let v = self.get_json(
                    ctx,
                    "/subreddits/mine/subscriber?limit=100",
                    "list your subreddits",
                )?;
                let mut out: Vec<OptionItem> = v
                    .pointer("/data/children")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|c| {
                                let d = c.get("data")?;
                                let n = d.get("display_name")?.as_str()?;
                                let title = d.get("title").and_then(Value::as_str).unwrap_or("");
                                Some(OptionItem::new(
                                    n,
                                    if title.is_empty() {
                                        format!("r/{n}")
                                    } else {
                                        format!("r/{n}  {title}")
                                    },
                                ))
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                out.sort_by_key(|o| o.value.to_lowercase());
                Ok(out)
            }
            "flairs" => {
                let sub = clean_subreddit(param.unwrap_or("")).map_err(invalid)?;
                let resp = ctx
                    .http
                    .send(&self.api(ctx, Method::Get, &format!("/r/{sub}/api/link_flair_v2"))?)
                    .map_err(|e| from_net(&e))?;
                // a subreddit without flair answers 403 or 404: that is an empty list, not a failure
                if matches!(resp.status, 403 | 404) {
                    return Ok(Vec::new());
                }
                if !resp.is_success() {
                    return Err(from_response(&resp, "list flairs"));
                }
                Ok(resp
                    .json()
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|f| {
                        Some(OptionItem::new(
                            f.get("id")?.as_str()?,
                            f.get("text").and_then(Value::as_str).unwrap_or("(no text)"),
                        ))
                    })
                    .collect())
            }
            other => Err(invalid(format!("Reddit has no list called \"{other}\""))),
        }
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let sub = clean_subreddit(payload.value("subreddit").unwrap_or(""))
            .map_err(|e| if payload.value("subreddit").is_none() {
                "Reddit needs a subreddit: map one to the field \"subreddit\" on the sharing button".to_string()
            } else {
                e
            })
            .map_err(invalid)?;
        let title = payload
            .value("title")
            .map(str::to_string)
            .or_else(|| Some(payload.title.trim().to_string()).filter(|t| !t.is_empty()))
            .ok_or_else(|| {
                invalid("Reddit needs a title: map a text block to the field \"title\"")
            })?;
        if title.chars().count() > 300 {
            return Err(invalid("A Reddit title can have at most 300 characters"));
        }
        let link = payload.value("url").map(str::to_string);
        if let Some(l) = &link {
            check_url(l).map_err(|e| invalid(format!("The link: {e}")))?;
        }
        let text = payload
            .value("text")
            .map(str::to_string)
            .or_else(|| payload.prompt.clone().filter(|p| !p.trim().is_empty()));

        let mut notes = Vec::new();
        if payload.files_zip.is_some() {
            notes.push("Reddit cannot take files, so the ticked files were not sent".to_string());
        }

        // What kind of post: a link, an image (the screenshot), or text.
        enum Kind {
            Link(String),
            Image,
            SelfPost,
        }
        let kind = match (&link, &payload.image) {
            (Some(l), _) => Kind::Link(l.clone()),
            (None, Some(_)) => Kind::Image,
            (None, None) => Kind::SelfPost,
        };
        if matches!(kind, Kind::SelfPost) && text.is_none() {
            return Err(invalid(
                "There is nothing to post: no screenshot, link or text",
            ));
        }

        // Step 1 (images): put the picture on Reddit's media storage, once.
        let mut image_url: Option<String> = ctx.progress.get("reddit.image");
        if matches!(kind, Kind::Image) && image_url.is_none() {
            let img = payload.image.as_ref().expect("image kind has an image");
            let lease_req = self
                .api(ctx, Method::Post, "/api/media/asset.json")?
                .body(Body::form(&[
                    ("filepath", img.name.as_str()),
                    ("mimetype", img.mime.as_str()),
                ]));
            let lease = ctx.http.send(&lease_req).map_err(|e| from_net(&e))?;
            if !lease.is_success() {
                return Err(from_response(
                    &lease,
                    "ask Reddit where to upload the image",
                ));
            }
            let lease_json = lease
                .json()
                .ok_or_else(|| invalid("Reddit's upload answer was unreadable"))?;
            let action = lease_json
                .pointer("/args/action")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid("Reddit did not say where to upload the image"))?;
            let upload_url = if action.starts_with("//") {
                format!("https:{action}")
            } else {
                action.to_string()
            };
            check_url(&upload_url).map_err(|e| invalid(format!("Reddit's upload address: {e}")))?;
            let mut form = Multipart::form_data();
            let mut key = None;
            for f in lease_json
                .pointer("/args/fields")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let (n, v) = (
                    f.get("name").and_then(Value::as_str).unwrap_or(""),
                    f.get("value").and_then(Value::as_str).unwrap_or(""),
                );
                if n == "key" {
                    key = Some(v.to_string());
                }
                form = form.field(n, v);
            }
            let key = key.ok_or_else(|| invalid("Reddit's upload answer had no key"))?;
            let up = Request::post(&upload_url)
                .timeout(ctx.timeout)
                .body(Body::multipart(form.file(
                    "file",
                    &img.name,
                    &img.mime,
                    img.data.clone(),
                )));
            let up_resp = ctx.http.send(&up).map_err(|e| from_net(&e))?;
            if !up_resp.is_success() {
                return Err(from_response(&up_resp, "upload the image"));
            }
            let url = format!("{}/{}", upload_url.trim_end_matches('/'), key);
            ctx.progress.set("reddit.image", &url);
            image_url = Some(url);
        }

        // Step 2: the post. Never repeated when an earlier attempt may have gone through.
        if ctx.progress.done("reddit.submitted") {
            return Err(invalid(
                "An earlier attempt may already have posted this. Check your Reddit profile before trying again",
            ));
        }
        let mut fields: Vec<(&str, String)> = vec![
            ("api_type", "json".into()),
            ("sr", sub.clone()),
            ("title", title),
            ("resubmit", "true".into()),
            ("sendreplies", "true".into()),
        ];
        match &kind {
            Kind::Link(l) => {
                fields.push(("kind", "link".into()));
                fields.push(("url", l.clone()));
            }
            Kind::Image => {
                fields.push(("kind", "image".into()));
                fields.push(("url", image_url.clone().unwrap_or_default()));
            }
            Kind::SelfPost => {
                fields.push(("kind", "self".into()));
                fields.push(("text", text.clone().unwrap_or_default()));
            }
        }
        if !matches!(kind, Kind::SelfPost) && text.is_some() {
            // the API has no caption for image or link posts; say so rather than drop it quietly
            notes.push(
                "The text was not posted: Reddit image and link posts have no body".to_string(),
            );
        }
        for key in ["flair_id", "flair_text"] {
            if let Some(v) = payload.value(key) {
                fields.push((key, v.to_string()));
            }
        }
        for key in ["nsfw", "spoiler"] {
            if payload
                .value(key)
                .is_some_and(|v| matches!(v.to_lowercase().as_str(), "true" | "yes" | "1" | "on"))
            {
                fields.push((key, "true".into()));
            }
        }
        let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let req = self
            .api(ctx, Method::Post, "/api/submit")?
            .body(Body::form(&pairs));
        ctx.progress.set("reddit.submitted", "1");
        let resp = match ctx.http.send(&req) {
            Ok(r) => r,
            Err(e) => {
                // Sent, but no answer: it may have been posted.
                let mut err = from_net(&e);
                err.kind = FailureKind::Rejected(0);
                err.message = format!(
                    "{}. Reddit may or may not have posted it, so CargoFrame did not retry: check your profile",
                    err.message
                );
                return Err(err);
            }
        };
        if !resp.is_success() {
            let err = from_response(&resp, "post to Reddit");
            return Err(match err.kind {
                // definitely not posted: allow the retry
                FailureKind::Throttled(_) | FailureKind::AuthExpired | FailureKind::Rejected(_) => {
                    ctx.progress.clear("reddit.submitted");
                    err
                }
                // a server failure: unknown
                _ => SendError::new(
                    FailureKind::Rejected(0),
                    format!("{}. Reddit may or may not have posted it, so CargoFrame did not retry: check your profile", err.message),
                ),
            });
        }
        if let Some(err) = body_errors(&resp) {
            // refused: nothing was posted
            ctx.progress.clear("reddit.submitted");
            return Err(err);
        }
        let json = resp.json().unwrap_or(Value::Null);
        let url = json
            .pointer("/json/data/url")
            .or_else(|| json.pointer("/json/data/user_submitted_page"))
            .and_then(Value::as_str)
            .map(String::from)
            .filter(|u| check_url(u).is_ok());
        let summary = if json.pointer("/json/data/url").is_some() {
            format!("Posted to r/{sub}")
        } else {
            format!("Posted to r/{sub} (Reddit is still processing the image)")
        };
        Ok(SendOutcome {
            summary,
            url,
            notes,
        })
    }
}

#[cfg(test)]
mod tests;
