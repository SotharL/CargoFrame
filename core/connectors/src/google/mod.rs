//! Google: upload the screenshot (and the zip of ticked files) to Drive, and optionally add a
//! row to a spreadsheet. The person creates a "Desktop app" OAuth client in their own Google
//! Cloud project and signs in once through the browser.
//!
//! By default only the narrow `drive.file` permission is asked for: CargoFrame can then see only
//! files and folders it created itself, so it keeps its uploads in a folder named "CargoFrame".
//! Turning on "full Drive" lets it use any folder the person names.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::model;
use crate::oauth::{self, ClientAuth, OAuthClient};
use crate::payload::{Attachment, OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use cargoframe_net::{check_url, encode_component, Body, Method, Multipart, Request};
use cargoframe_safe::SendError;
use serde_json::{json, Value};
use std::time::Duration;

const FOLDER_MIME: &str = "application/vnd.google-apps.folder";
const APP_FOLDER: &str = "CargoFrame";

#[derive(Debug, Clone)]
pub struct Endpoints {
    pub auth: String,
    pub token: String,
    pub revoke: String,
    /// `https://www.googleapis.com`
    pub api: String,
    /// `https://sheets.googleapis.com`
    pub sheets: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            auth: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token: "https://oauth2.googleapis.com/token".into(),
            revoke: "https://oauth2.googleapis.com/revoke".into(),
            api: "https://www.googleapis.com".into(),
            sheets: "https://sheets.googleapis.com".into(),
        }
    }
}

pub struct GoogleConnector {
    settings: model::Google,
    ep: Endpoints,
    wait: Duration,
}

impl GoogleConnector {
    pub fn new(settings: model::Google) -> Self {
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

    fn scopes(&self) -> Vec<String> {
        let drive = if self.settings.full_drive {
            "https://www.googleapis.com/auth/drive"
        } else {
            "https://www.googleapis.com/auth/drive.file"
        };
        let mut s = vec![drive.to_string()];
        if self.settings.sheets {
            s.push("https://www.googleapis.com/auth/spreadsheets".into());
        }
        s
    }

    fn client(&self, ctx: &Ctx) -> Result<OAuthClient, SendError> {
        let secret = ctx.secret("client_secret")?.filter(|s| !s.is_empty()).ok_or_else(|| {
            not_set_up("Google needs the client secret of your OAuth client: open Connections, paste it and press Save")
        })?;
        Ok(OAuthClient {
            auth_url: self.ep.auth.clone(),
            token_url: self.ep.token.clone(),
            revoke_url: Some(self.ep.revoke.clone()),
            client_id: self.settings.client_id.trim().to_string(),
            client_secret: Some(secret),
            scopes: self.scopes(),
            extra_auth_params: vec![
                ("access_type".into(), "offline".into()),
                ("prompt".into(), "consent".into()),
            ],
            pkce: true,
            client_auth: ClientAuth::Body,
            redirect_port: None,
            redirect: oauth::Redirect::Ip,
            wait: self.wait,
        })
    }

    fn request(&self, ctx: &Ctx, method: Method, url: String) -> Result<Request, SendError> {
        let token = oauth::access_token(&self.client(ctx)?, ctx)?;
        Ok(Request::new(method, url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .timeout(ctx.timeout))
    }

    fn call_json(&self, ctx: &Ctx, req: Request, what: &str) -> Result<Value, SendError> {
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(from_response(&resp, what));
        }
        resp.json()
            .ok_or_else(|| invalid(format!("Google answered {what} with something unreadable")))
    }

    fn drive(&self, path: &str) -> String {
        format!("{}/drive/v3{path}", self.ep.api)
    }

    /// The folder uploads go to.
    fn folder(&self, ctx: &Ctx, payload: &Payload) -> Result<String, SendError> {
        if let Some(raw) = payload.value("folder") {
            let id = id_from_link(raw, "folders")
                .ok_or_else(|| invalid(format!("\"{raw}\" is not a Drive folder id or link")))?;
            return Ok(id);
        }
        if let Some(id) = ctx.progress.get("google.folder") {
            return Ok(id);
        }
        let q = format!("name = '{APP_FOLDER}' and mimeType = '{FOLDER_MIME}' and trashed = false");
        let found = self.call_json(
            ctx,
            self.request(
                ctx,
                Method::Get,
                self.drive(&format!(
                    "/files?q={}&fields=files(id)&pageSize=1&spaces=drive",
                    encode_component(&q)
                )),
            )?,
            "look for the CargoFrame folder",
        )?;
        let id = match found.pointer("/files/0/id").and_then(Value::as_str) {
            Some(id) => id.to_string(),
            None => {
                let created = self.call_json(
                    ctx,
                    self.request(ctx, Method::Post, self.drive("/files?fields=id"))?
                        .body(Body::json(
                            &json!({"name": APP_FOLDER, "mimeType": FOLDER_MIME}),
                        )),
                    "create the CargoFrame folder",
                )?;
                created
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid("Google did not say which folder it created"))?
                    .to_string()
            }
        };
        ctx.progress.set("google.folder", &id);
        Ok(id)
    }

    /// Upload one file; returns its link.
    fn upload(&self, ctx: &Ctx, folder: &str, att: &Attachment) -> Result<String, SendError> {
        let meta = json!({"name": att.name, "parents": [folder]});
        let body = Multipart::related()
            .raw(
                "application/json; charset=UTF-8",
                serde_json::to_vec(&meta).unwrap_or_default(),
            )
            .raw(&att.mime, att.data.clone());
        let req = self
            .request(
                ctx,
                Method::Post,
                format!(
                    "{}/upload/drive/v3/files?uploadType=multipart&fields=id,name,webViewLink",
                    self.ep.api
                ),
            )?
            .body(Body::multipart(body));
        let v = self.call_json(ctx, req, &format!("upload {}", att.name))?;
        Ok(v.get("webViewLink")
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| {
                v.get("id")
                    .and_then(Value::as_str)
                    .map(|id| format!("https://drive.google.com/file/d/{id}/view"))
            })
            .unwrap_or_default())
    }
}

/// `https://drive.google.com/drive/folders/<id>?usp=sharing` or a bare id.
fn id_from_link(raw: &str, segment: &str) -> Option<String> {
    let raw = raw.trim();
    let candidate = if raw.contains('/') {
        let after = raw.split(&format!("/{segment}/")).nth(1)?;
        after.split(['/', '?', '#']).next()?.to_string()
    } else {
        raw.to_string()
    };
    let ok = !candidate.is_empty()
        && candidate.len() <= 128
        && candidate
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    ok.then_some(candidate)
}

impl Connector for GoogleConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint, list| FieldSpec {
            key,
            label,
            hint,
            list,
        };
        Describe {
            fields: vec![
                f("folder", "Drive folder", "A folder id or link. Empty: the CargoFrame folder", Some("folders")),
                f("spreadsheet", "Spreadsheet", "An id or link: adds a row there (needs the spreadsheet option)", None),
                f("sheet", "Sheet tab", "Defaults to Sheet1", None),
                f("col1", "Column 1", "Value for the first column after the date", None),
                f("col2", "Column 2", "", None),
                f("col3", "Column 3", "", None),
                f("col4", "Column 4", "", None),
                f("col5", "Column 5", "", None),
            ],
            lists: vec![ListSpec { id: "folders".into(), label: "Drive folders".into(), param: None, inside: None }],
            summary: "Uploads the screenshot and the zip of ticked files to Drive. A spreadsheet row holds the date, the columns you map, and the links.".into(),
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
        oauth::authorize(&self.client(ctx)?, ctx)?;
        self.test(ctx)
    }

    fn disconnect(&self, ctx: &Ctx) {
        match self.client(ctx) {
            Ok(c) => oauth::sign_out(&c, ctx),
            Err(_) => {
                let _ = ctx
                    .secrets
                    .delete(&crate::secrets::key(ctx.connection_id, "oauth"));
            }
        }
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let v = self.call_json(
            ctx,
            self.request(
                ctx,
                Method::Get,
                self.drive("/about?fields=user(displayName,emailAddress)"),
            )?,
            "check the sign-in",
        )?;
        let who = ["/user/emailAddress", "/user/displayName"]
            .iter()
            .find_map(|p| v.pointer(p).and_then(Value::as_str))
            .unwrap_or("your account");
        Ok(format!("Signed in to Google as {who}"))
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        if list != "folders" {
            return Err(invalid(format!("Google has no list called \"{list}\"")));
        }
        let q = format!("mimeType = '{FOLDER_MIME}' and trashed = false");
        let v = self.call_json(
            ctx,
            self.request(
                ctx,
                Method::Get,
                self.drive(&format!(
                    "/files?q={}&fields=files(id,name)&pageSize=100&orderBy=name&spaces=drive",
                    encode_component(&q)
                )),
            )?,
            "list folders",
        )?;
        Ok(v.get("files")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|f| {
                        Some(OptionItem::new(
                            f.get("id")?.as_str()?,
                            f.get("name")?.as_str()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let sheet_id = match payload.value("spreadsheet") {
            None => None,
            Some(raw) => {
                if !self.settings.sheets {
                    return Err(invalid(
                        "A spreadsheet is mapped, but this Google connection has the spreadsheet option off: turn it on and sign in again",
                    ));
                }
                Some(
                    id_from_link(raw, "d").ok_or_else(|| {
                        invalid(format!("\"{raw}\" is not a spreadsheet id or link"))
                    })?,
                )
            }
        };
        if payload.image.is_none() && payload.files_zip.is_none() && sheet_id.is_none() {
            return Err(invalid("There is nothing to send to Google"));
        }

        let mut links: Vec<(&str, String)> = Vec::new();
        let mut notes = Vec::new();
        if payload.image.is_some() || payload.files_zip.is_some() {
            let folder = self.folder(ctx, payload)?;
            for (step, att) in [
                ("google.image", payload.image.as_ref()),
                ("google.still", payload.still.as_ref()),
                ("google.zip", payload.files_zip.as_ref()),
            ] {
                let Some(att) = att else { continue };
                let link = match ctx.progress.get(step) {
                    Some(l) => l,
                    None => {
                        let l = self.upload(ctx, &folder, att).map_err(|mut e| {
                            if !self.settings.full_drive && payload.value("folder").is_some() {
                                e.message.push_str(
                                    " (without the full Drive option CargoFrame can only use folders it created; leave the folder empty or turn that option on)",
                                );
                            }
                            e
                        })?;
                        ctx.progress.set(step, &l);
                        l
                    }
                };
                links.push((if step == "google.zip" { "zip" } else { "image" }, link));
            }
        }

        if let Some(id) = &sheet_id {
            if !ctx.progress.done("google.row") {
                let tab = payload.value("sheet").unwrap_or("Sheet1");
                let when = format!(
                    "{} {}",
                    payload
                        .vars
                        .get("capture.date")
                        .map(String::as_str)
                        .unwrap_or(""),
                    payload
                        .vars
                        .get("capture.time")
                        .map(String::as_str)
                        .unwrap_or("")
                )
                .trim()
                .to_string();
                let mut row: Vec<Value> = vec![Value::String(when)];
                for n in 1..=20 {
                    match payload.fields.iter().find(|f| f.key == format!("col{n}")) {
                        Some(f) => row.push(Value::String(f.value.trim().to_string())),
                        None if n <= 5 => row.push(Value::String(String::new())),
                        None => break,
                    }
                }
                for (_, l) in &links {
                    row.push(Value::String(l.clone()));
                }
                let range = encode_component(&format!("{tab}!A1"));
                let req = self
                    .request(
                        ctx,
                        Method::Post,
                        format!(
                            "{}/v4/spreadsheets/{id}/values/{range}:append?valueInputOption=USER_ENTERED&insertDataOption=INSERT_ROWS",
                            self.ep.sheets
                        ),
                    )?
                    .body(Body::json(&json!({ "values": [row] })));
                let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
                if !resp.is_success() {
                    let mut e = from_response(&resp, "add the row to the spreadsheet");
                    if !links.is_empty() {
                        e.message =
                            format!("The files were uploaded, but {}", lower_first(&e.message));
                    }
                    return Err(e);
                }
                ctx.progress.set("google.row", "1");
            }
            notes.push("Added a row to the spreadsheet".to_string());
        }

        let url = links
            .iter()
            .map(|(_, l)| l.clone())
            .find(|l| check_url(l).is_ok());
        let summary = match (links.is_empty(), sheet_id.is_some()) {
            (false, false) => "Uploaded to Google Drive".to_string(),
            (false, true) => "Uploaded to Google Drive and added a row".to_string(),
            (true, _) => "Added a row to the spreadsheet".to_string(),
        };
        if !links.is_empty() && sheet_id.is_some() {
            notes.clear();
        }
        Ok(SendOutcome {
            summary,
            url,
            notes,
            place: None,
        })
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests;
