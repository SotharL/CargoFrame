//! Jira: the reference destination. Works with Jira Cloud (e-mail + API token) and Jira Server or
//! Data Center (personal access token) through REST API v2, which both understand.
//!
//! A share creates an issue, or updates the one the `issue` field names, and attaches the
//! screenshot and, if files were ticked, their zip. An update does to each field what its mapping
//! says (replace it, add below it, add one line, add labels, count one more, leave it); a
//! `comment` field adds a comment either way; `react` votes for an issue that was there. A layout
//! of 0.7.0 or before says none of this: its description becomes a comment, as then. What it has
//! already done is recorded in [`Progress`], so a retry after a failed upload never creates a
//! second issue or updates twice.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::model::{self, JiraAuth};
use crate::payload::{first_line, Field, FieldKind, OptionItem, Payload, SendOutcome, Update};
use crate::richtext::{render, Style};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cargoframe_i18n::{msg, t, tf};
use cargoframe_net::{check_url, encode_component, Body, Method, Multipart, Request, Response};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Map, Value};

pub struct JiraConnector {
    settings: model::Jira,
}

impl JiraConnector {
    pub fn new(settings: model::Jira) -> Self {
        Self { settings }
    }

    fn cloud(&self) -> bool {
        self.settings.auth == JiraAuth::ApiToken
    }

    fn base(&self) -> Result<String, SendError> {
        check_url(&self.settings.site_url)
            .map_err(|e| invalid(tf("Jira address: {e}", &[("e", &e)])))?;
        Ok(self
            .settings
            .site_url
            .trim()
            .trim_end_matches('/')
            .to_string())
    }

    fn authorization(&self, ctx: &Ctx) -> Result<String, SendError> {
        let token = ctx
            .secret("token")?
            .filter(|t| !t.is_empty())
            .ok_or_else(|| {
                not_set_up(t(
                    "Jira has no API token yet: open Connections, paste it and press Save",
                ))
            })?;
        Ok(match self.settings.auth {
            JiraAuth::ApiToken => format!(
                "Basic {}",
                STANDARD.encode(format!("{}:{token}", self.settings.email.trim()))
            ),
            JiraAuth::Pat => format!("Bearer {token}"),
        })
    }

    fn request(&self, ctx: &Ctx, method: Method, path: &str) -> Result<Request, SendError> {
        let url = format!("{}{path}", self.base()?);
        Ok(Request::new(method, url)
            .header("Authorization", self.authorization(ctx)?)
            .header("Accept", "application/json")
            .timeout(ctx.timeout))
    }

    fn call(&self, ctx: &Ctx, req: Request, what: &str) -> Result<Response, SendError> {
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if resp.is_success() {
            Ok(resp)
        } else {
            Err(from_response(&resp, what))
        }
    }

    fn get_json(&self, ctx: &Ctx, path: &str, what: &str) -> Result<Value, SendError> {
        let resp = self.call(ctx, self.request(ctx, Method::Get, path)?, what)?;
        resp.json().ok_or_else(|| {
            invalid(tf(
                "Jira answered {what} with something unreadable",
                &[("what", &t(what))],
            ))
        })
    }

    /// People found by what is typed: anyone, or those who can be assigned in `project`. Jira
    /// Cloud names them by account id, Server and Data Center by user name. With nothing typed,
    /// the first page (Server: everyone, by the `.` that matches all). "Me" comes first, as
    /// `@me`, which a share turns into the person signed in.
    fn people(
        &self,
        ctx: &Ctx,
        project: Option<&str>,
        typed: &str,
    ) -> Result<Vec<OptionItem>, SendError> {
        let cloud = self.cloud();
        let q = encode_component(if typed.is_empty() && !cloud {
            "."
        } else {
            typed
        });
        let by = if cloud { "query" } else { "username" };
        let path = match project {
            Some(p) => format!(
                "/rest/api/2/user/assignable/search?project={}&{by}={q}&maxResults=50",
                encode_component(p)
            ),
            None if cloud && typed.is_empty() => "/rest/api/2/users/search?maxResults=50".into(),
            None => format!("/rest/api/2/user/search?{by}={q}&maxResults=50"),
        };
        let v = self.get_json(ctx, &path, msg!("search people"))?;
        let mut out = Vec::new();
        let me = t("Me");
        if typed.is_empty() || me.to_lowercase().starts_with(&typed.to_lowercase()) {
            out.push(OptionItem::new(ME, me));
        }
        for u in v.as_array().into_iter().flatten() {
            // apps and bots are listed by /users/search too: people only
            let person = u
                .get("accountType")
                .and_then(Value::as_str)
                .is_none_or(|t| t == "atlassian");
            if !person || u.get("active").and_then(Value::as_bool) == Some(false) {
                continue;
            }
            let id = u
                .get(if cloud { "accountId" } else { "name" })
                .and_then(Value::as_str);
            let Some(id) = id else { continue };
            let name = u.get("displayName").and_then(Value::as_str).unwrap_or(id);
            let label = match u.get("emailAddress").and_then(Value::as_str) {
                Some(mail) if !mail.is_empty() => format!("{name} ({mail})"),
                _ => name.to_string(),
            };
            out.push(OptionItem::new(id, label));
        }
        Ok(out)
    }

    /// Teams found by what is typed, through the Team field's own search (the suggestions Jira
    /// gives when a query names the field): the value is the team's id, which the field takes.
    fn teams(&self, ctx: &Ctx, typed: &str) -> Result<Vec<OptionItem>, SendError> {
        let fields = self.get_json(ctx, "/rest/api/2/field", msg!("find the Team field"))?;
        let clause = fields.as_array().into_iter().flatten().find_map(|f| {
            let custom = f.pointer("/schema/custom").and_then(Value::as_str)?;
            if !custom.to_lowercase().contains("team") {
                return None;
            }
            f.get("clauseNames")
                .and_then(Value::as_array)
                .and_then(|c| {
                    c.iter()
                        .filter_map(Value::as_str)
                        .find(|c| c.starts_with("cf["))
                })
                .map(str::to_string)
        });
        let Some(clause) = clause else {
            return Err(invalid(t("This Jira has no Team field")));
        };
        let v = self.get_json(
            ctx,
            &format!(
                "/rest/api/2/jql/autocompletedata/suggestions?fieldName={}&fieldValue={}",
                encode_component(&clause),
                encode_component(typed)
            ),
            msg!("search teams"),
        )?;
        Ok(v.get("results")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|r| {
                let id = r.get("value").and_then(Value::as_str)?;
                let label = r.get("displayName").and_then(Value::as_str).unwrap_or(id);
                Some(OptionItem::new(id, strip_tags(label)))
            })
            .collect())
    }

    /// The person signed in, as a field names them (Cloud: account id; Server: user name).
    fn me(&self, ctx: &Ctx) -> Result<String, SendError> {
        let me = self.get_json(ctx, "/rest/api/2/myself", msg!("check the sign-in"))?;
        me.get(if self.cloud() { "accountId" } else { "name" })
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| invalid(t("Jira did not say who you are")))
    }

    /// Update the issue `key` as the mapped fields say: read what is there when something is
    /// added below it or counted, then one PUT.
    fn update(&self, ctx: &Ctx, key: &str, payload: &Payload) -> Result<(), SendError> {
        let wanted: Vec<&str> = payload
            .fields
            .iter()
            .filter(|f| {
                matches!(
                    f.update,
                    Some(Update::Append | Update::AppendLine | Update::Increment)
                )
            })
            .map(|f| f.key.as_str())
            .collect();
        let now = if wanted.is_empty() {
            Map::new()
        } else {
            let req = self.request(
                ctx,
                Method::Get,
                &format!(
                    "/rest/api/2/issue/{}?fields={}",
                    encode_component(key),
                    encode_component(&wanted.join(","))
                ),
            )?;
            let resp = self.call(ctx, req, &tf("read {key}", &[("key", &key)]))?;
            resp.json()
                .and_then(|v| v.get("fields").and_then(Value::as_object).cloned())
                .unwrap_or_default()
        };
        let (mut set, ops) = build_update(payload, self.cloud(), &now)?;
        if names_me(payload) {
            put_me(&mut set, &self.me(ctx)?);
        }
        if set.is_empty() && ops.is_empty() {
            return Ok(());
        }
        let mut body = Map::new();
        if !set.is_empty() {
            body.insert("fields".into(), Value::Object(set));
        }
        if !ops.is_empty() {
            body.insert("update".into(), Value::Object(ops));
        }
        let req = self
            .request(
                ctx,
                Method::Put,
                &format!("/rest/api/2/issue/{}", encode_component(key)),
            )?
            .body(Body::json(&Value::Object(body)));
        self.call(ctx, req, &tf("update {key}", &[("key", &key)]))?;
        Ok(())
    }

    fn attach(
        &self,
        ctx: &Ctx,
        key: &str,
        name: &str,
        mime: &str,
        data: &[u8],
    ) -> Result<(), SendError> {
        let req = self
            .request(
                ctx,
                Method::Post,
                &format!("/rest/api/2/issue/{}/attachments", encode_component(key)),
            )?
            .header("X-Atlassian-Token", "no-check")
            .body(Body::multipart(Multipart::form_data().file(
                "file",
                name,
                mime,
                data.to_vec(),
            )));
        self.call(
            ctx,
            req,
            &tf("attach {name} to {key}", &[("name", &name), ("key", &key)]),
        )
        .map(|_| ())
    }
}

// ----- fields -----

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_digit())
}

fn id_or_name(v: &str) -> Value {
    if is_digits(v) {
        json!({ "id": v })
    } else {
        json!({ "name": v })
    }
}

fn split_list(v: &str) -> Vec<&str> {
    v.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// The screenshot and the recording shown where the text puts them: they are attached to the
/// issue under these names, and Jira shows an attachment named in the text in its place.
fn with_media(text: &str, payload: &Payload) -> String {
    let embed = |a: Option<&crate::payload::Attachment>, how: &str, in_table: bool| {
        a.map(|a| {
            // in a table a `|` would end the cell: the file is shown at its own size there
            if in_table {
                format!("!{}!", a.name)
            } else {
                format!("!{}|{how}!", a.name)
            }
        })
        .unwrap_or_default()
    };
    text.split('\n')
        .map(|line| {
            let t = line.trim_start().starts_with('|');
            crate::payload::place_media(
                line,
                &embed(payload.screenshot(), "thumbnail", t),
                &embed(payload.recording(), "width=640", t),
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The value of a person field that means the person signed in.
pub const ME: &str = "@me";

/// Does a person field of the share say "Me"?
fn names_me(payload: &Payload) -> bool {
    payload
        .fields
        .iter()
        .any(|f| f.value.trim() == ME && matches!(f.key.as_str(), "assignee" | "reporter"))
}

/// Put the person signed in where a person field said "Me" (`{"accountId": "@me"}`).
fn put_me(fields: &mut Map<String, Value>, me: &str) {
    for key in ["assignee", "reporter"] {
        if let Some(Value::Object(who)) = fields.get_mut(key) {
            for v in who.values_mut() {
                if v.as_str() == Some(ME) {
                    *v = Value::String(me.to_string());
                }
            }
        }
    }
}

/// Jira's suggestions mark the matched part in bold: the text alone.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            c if !inside => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

/// A summary must be one line of at most 255 characters.
fn clean_summary(s: &str) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 255 {
        format!("{}…", one.chars().take(254).collect::<String>())
    } else {
        one
    }
}

/// How one field's value is shaped for Jira's API.
fn shape(f: &Field, cloud: bool) -> Result<Value, String> {
    let v = f.value.trim();
    Ok(match f.key.as_str() {
        "project" => {
            if is_digits(v) {
                json!({ "id": v })
            } else {
                json!({ "key": v })
            }
        }
        "issuetype" | "priority" => id_or_name(v),
        "summary" => Value::String(clean_summary(v)),
        // the description is Jira's wiki markup: the text's bold, italic, code, quotes and lists
        // are written that way (see `richtext`)
        "description" => Value::String(render(&f.value, Style::Jira)),
        "environment" | "duedate" => Value::String(f.value.trim_end().to_string()),
        "labels" => Value::Array(
            split_list(v)
                .into_iter()
                // Jira labels cannot contain spaces
                .map(|l| Value::String(l.split_whitespace().collect::<Vec<_>>().join("-")))
                .collect(),
        ),
        "components" | "fixVersions" | "versions" => {
            Value::Array(split_list(v).into_iter().map(id_or_name).collect())
        }
        "assignee" | "reporter" => {
            if cloud {
                json!({ "accountId": v })
            } else {
                json!({ "name": v })
            }
        }
        "parent" => json!({ "key": v }),
        _ => match f.kind {
            FieldKind::Text => Value::String(f.value.trim_end().to_string()),
            FieldKind::Number => {
                let n: f64 = v.replace(',', ".").parse().map_err(|_| {
                    tf(
                        "\"{v}\" is not a number (field {field})",
                        &[("v", &v), ("field", &f.key)],
                    )
                })?;
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .ok_or_else(|| {
                        tf(
                            "\"{v}\" is not a number (field {key})",
                            &[("v", &v), ("key", &f.key)],
                        )
                    })?
            }
            FieldKind::Flag => Value::Bool(matches!(
                v.to_lowercase().as_str(),
                "true" | "yes" | "1" | "on"
            )),
            FieldKind::Choice => {
                if f.from_list {
                    json!({ "id": v })
                } else {
                    json!({ "value": v })
                }
            }
            FieldKind::MultiChoice => Value::Array(
                split_list(v)
                    .into_iter()
                    .map(|o| {
                        if f.from_list {
                            json!({ "id": o })
                        } else {
                            json!({ "value": o })
                        }
                    })
                    .collect(),
            ),
        },
    })
}

/// The `fields` object for creating an issue.
pub fn build_fields(payload: &Payload, cloud: bool) -> Result<Map<String, Value>, SendError> {
    let mut out = Map::new();
    for f in &payload.fields {
        if f.key == "issue" || f.key == "comment" || f.value.trim().is_empty() {
            continue;
        }
        let v = shape(f, cloud).map_err(invalid)?;
        out.insert(f.key.clone(), v);
    }
    if !out.contains_key("project") || !out.contains_key("issuetype") {
        return Err(invalid(t("Jira needs a project and an issue type: map them to the fields \"project\" and \"issuetype\" on the sharing button")));
    }
    if !out.contains_key("summary") {
        let title = if payload.title.trim().is_empty() {
            t("CargoFrame capture")
        } else {
            payload.title.trim()
        };
        out.insert("summary".into(), Value::String(clean_summary(title)));
    }
    if !out.contains_key("description") {
        if let Some(p) = payload.prompt.as_deref().filter(|p| !p.trim().is_empty()) {
            out.insert("description".into(), Value::String(render(p, Style::Jira)));
        }
    }
    if let Some(Value::String(d)) = out.get_mut("description") {
        *d = with_media(d, payload);
    }
    Ok(out)
}

/// The fields a list of values is: an update adds to them.
const LISTS: [&str; 4] = ["labels", "components", "fixVersions", "versions"];

/// Jira's fields (or update operations), by key.
type Fields = Map<String, Value>;

/// What an update of an existing issue sends (`fields` set, `update` operations), from the
/// mapped fields and the issue as it is (`now`, its fields, for what is added below or counted).
pub fn build_update(
    payload: &Payload,
    cloud: bool,
    now: &Map<String, Value>,
) -> Result<(Fields, Fields), SendError> {
    let mut set = Map::new();
    let mut ops = Map::new();
    for f in &payload.fields {
        if matches!(
            f.key.as_str(),
            "issue" | "comment" | "project" | "issuetype"
        ) {
            continue;
        }
        let how = f.update.unwrap_or(Update::Replace);
        if how == Update::Skip || (f.value.trim().is_empty() && how != Update::Increment) {
            continue;
        }
        match how {
            Update::Add if LISTS.contains(&f.key.as_str()) => {
                let Value::Array(items) = shape(f, cloud).map_err(invalid)? else {
                    continue;
                };
                let adds: Vec<Value> = items.into_iter().map(|i| json!({ "add": i })).collect();
                ops.insert(f.key.clone(), Value::Array(adds));
            }
            Update::Append | Update::AppendLine => {
                let text = if how == Update::AppendLine {
                    first_line(&f.value).to_string()
                } else {
                    f.value.clone()
                };
                let mut added = render(&text, Style::Jira);
                if f.key == "description" {
                    added = with_media(&added, payload);
                }
                let before = now
                    .get(&f.key)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim_end();
                let joined = if before.is_empty() {
                    added
                } else if f.key == "summary" {
                    clean_summary(&format!("{before} {added}"))
                } else {
                    format!("{before}\n\n{added}")
                };
                set.insert(f.key.clone(), Value::String(joined));
            }
            Update::Increment => {
                let step: f64 = f.value.trim().replace(',', ".").parse().unwrap_or(1.0);
                let before = now.get(&f.key).and_then(Value::as_f64).unwrap_or(0.0);
                set.insert(f.key.clone(), json!(before + step));
            }
            _ => {
                let mut v = shape(f, cloud).map_err(invalid)?;
                if f.key == "description" {
                    if let Value::String(d) = &mut v {
                        *d = with_media(d, payload);
                    }
                }
                set.insert(f.key.clone(), v);
            }
        }
    }
    Ok((set, ops))
}

/// The fields Jira's create screen requires and fills in no other way (no default value), from
/// the answer of `createmeta/{project}/issuetypes/{type}`: Jira Cloud lists them under
/// `fields`, Server and Data Center under `values`. The project and the issue type are what was
/// asked about, so they are never missing; nor is the reporter, which Jira sets to the person
/// signed in when it is left out (Jira Cloud marks it required, and not every Jira says it has a
/// default).
pub fn required_fields(v: &Value) -> Vec<OptionItem> {
    let list = v
        .get("fields")
        .or_else(|| v.get("values"))
        .and_then(Value::as_array);
    list.into_iter()
        .flatten()
        .filter(|f| f.get("required").and_then(Value::as_bool) == Some(true))
        .filter(|f| f.get("hasDefaultValue").and_then(Value::as_bool) != Some(true))
        .filter_map(|f| {
            let id = f
                .get("fieldId")
                .or_else(|| f.get("key"))
                .and_then(Value::as_str)?;
            if matches!(id, "project" | "issuetype" | "reporter") {
                return None;
            }
            let name = f.get("name").and_then(Value::as_str).unwrap_or(id);
            Some(OptionItem::new(id, name))
        })
        .collect()
}

fn issue_key_ok(k: &str) -> bool {
    // PROJ-123: letters/digits/underscore, a dash, digits
    let Some((p, n)) = k.rsplit_once('-') else {
        return false;
    };
    !p.is_empty() && is_digits(n) && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

impl Connector for JiraConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint, list| FieldSpec {
            key,
            label,
            hint,
            list,
        };
        let l = |id: &str, label: &str, param| ListSpec {
            id: id.into(),
            label: label.into(),
            param,
            inside: None,
            search: false,
        };
        // too long to list whole: Jira searches them for what is typed
        let searched = |id: &str, label: &str, param| ListSpec {
            search: true,
            ..l(id, label, param)
        };
        Describe {
            fields: vec![
                f("project", msg!("Project"), msg!("A project key such as PROJ"), Some("projects")),
                f("issuetype", msg!("Issue type"), msg!("Bug, Task, Story..."), Some("issue_types")),
                f("summary", msg!("Summary"), msg!("One line. Defaults to the window title"), None),
                f("description", msg!("Description"), msg!("Defaults to the prompt text"), None),
                f("priority", msg!("Priority"), "", Some("priorities")),
                f("labels", msg!("Labels"), msg!("Separate several with commas"), Some("labels")),
                f("assignee", msg!("Assignee"), msg!("Me, or anyone found by name"), Some("users")),
                f("components", msg!("Components"), msg!("Separate several with commas"), Some("components")),
                f("fixVersions", msg!("Fix versions"), msg!("Separate several with commas"), Some("versions")),
                f("versions", msg!("Affects versions"), msg!("Separate several with commas"), Some("versions")),
                f("duedate", msg!("Due date"), msg!("YYYY-MM-DD"), None),
                f("environment", msg!("Environment"), "", None),
                f("parent", msg!("Parent issue"), msg!("An issue key, for sub-tasks"), Some("recent_issues")),
                f(
                    "issue",
                    msg!("Update an existing issue"),
                    msg!("An issue key: updates that issue (each field as its mapping says) and attaches the files, instead of creating one. Empty: a new issue"),
                    Some("recent_issues"),
                ),
                f(
                    "comment",
                    msg!("Comment"),
                    msg!("Added as a comment, on the new issue or on the one updated"),
                    None,
                ),
            ],
            lists: vec![
                l("projects", msg!("Projects"), None),
                l("issue_types", msg!("Issue types"), None),
                l("priorities", msg!("Priorities"), None),
                l("labels", msg!("Labels"), None),
                l("recent_issues", msg!("Issues I reported recently"), None),
                searched("users", msg!("People (search by name; Me first)"), None),
                searched(
                    "assignable_users",
                    msg!("People who can be assigned in a project (search by name)"),
                    Some(msg!("project key")),
                ),
                searched("teams", msg!("Teams (search by name)"), None),
                l("components", msg!("Components of a project"), Some(msg!("project key"))),
                l("versions", msg!("Versions of a project"), Some(msg!("project key"))),
            ],
            summary: msg!("Creates an issue, or updates the one picked, and attaches the screenshot (and a zip of the ticked files); a comment can go with either. Custom fields use their key, such as customfield_10042.").into(),
        }
    }

    fn status(&self, ctx: &Ctx) -> Status {
        match ctx.secret("token") {
            Ok(Some(t)) if !t.is_empty() => Status::Connected,
            _ => Status::Disconnected,
        }
    }

    fn disconnect(&self, ctx: &Ctx) {
        let _ = ctx
            .secrets
            .delete(&crate::secrets::key(ctx.connection_id, "token"));
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let me = self.get_json(ctx, "/rest/api/2/myself", msg!("check the sign-in"))?;
        let who = ["displayName", "emailAddress", "name"]
            .iter()
            .find_map(|k| me.get(k).and_then(Value::as_str))
            .unwrap_or(t("your account"));
        let host = check_url(&self.settings.site_url).unwrap_or_default();
        Ok(tf(
            "Signed in to {host} as {who}",
            &[("host", &host), ("who", &who)],
        ))
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        // a searched list carries what is typed after `?`
        let (list, typed) = match list.split_once('?') {
            Some((l, q)) => (l, q.trim()),
            None => (list, ""),
        };
        let (name, param) = match list.split_once(':') {
            Some((n, p)) => (n, Some(p.trim())),
            None => (list, None),
        };
        let need_project = || {
            let key = param.filter(|p| !p.is_empty()).ok_or_else(|| {
                invalid(tf(
                    "The list \"{name}\" needs a project key, like {name}:PROJ",
                    &[("name", &name)],
                ))
            })?;
            if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                Ok(key.to_string())
            } else {
                Err(invalid(tf(
                    "\"{key}\" is not a project key (letters, digits and _ only)",
                    &[("key", &key)],
                )))
            }
        };
        let strings = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(String::from);
        match name {
            "projects" => {
                let v = match self.get_json(
                    ctx,
                    "/rest/api/2/project/search?maxResults=100&orderBy=name",
                    msg!("list projects"),
                ) {
                    Ok(v) => v.get("values").cloned().unwrap_or(v),
                    // Server and Data Center have no search endpoint
                    Err(e) if matches!(e.kind, FailureKind::Rejected(404)) => {
                        self.get_json(ctx, "/rest/api/2/project", msg!("list projects"))?
                    }
                    Err(e) => return Err(e),
                };
                Ok(v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| {
                                let key = strings(p, "key")?;
                                let label = strings(p, "name")
                                    .map(|n| format!("{n} ({key})"))
                                    .unwrap_or_else(|| key.clone());
                                Some(OptionItem::new(key, label))
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            "issue_types" | "priorities" => {
                let path = if name == "issue_types" {
                    "/rest/api/2/issuetype"
                } else {
                    "/rest/api/2/priority"
                };
                let v = self.get_json(
                    ctx,
                    path,
                    &tf("list {list}", &[("list", &name.replace('_', " "))]),
                )?;
                Ok(v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|t| t.get("subtask").and_then(Value::as_bool) != Some(true))
                            .filter_map(|t| {
                                Some(OptionItem::new(strings(t, "id")?, strings(t, "name")?))
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            "labels" => {
                let v =
                    self.get_json(ctx, "/rest/api/2/label?maxResults=200", msg!("list labels"))?;
                Ok(v.get("values")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(|l| OptionItem::new(l, l))
                            .collect()
                    })
                    .unwrap_or_default())
            }
            "components" => {
                let v = self.get_json(
                    ctx,
                    &format!("/rest/api/2/project/{}/components", need_project()?),
                    msg!("list components"),
                )?;
                Ok(v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|c| {
                                Some(OptionItem::new(strings(c, "id")?, strings(c, "name")?))
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            "versions" => {
                let v = self.get_json(
                    ctx,
                    &format!("/rest/api/2/project/{}/versions", need_project()?),
                    msg!("list versions"),
                )?;
                Ok(v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|c| c.get("archived").and_then(Value::as_bool) != Some(true))
                            .filter_map(|c| {
                                Some(OptionItem::new(strings(c, "id")?, strings(c, "name")?))
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            "users" | "assignable_users" => {
                let project = param
                    .filter(|p| !p.is_empty())
                    .map(|_| need_project())
                    .transpose()?;
                self.people(ctx, project.as_deref(), typed)
            }
            "teams" => self.teams(ctx, typed),
            // what Jira needs to create an issue of a type in a project: `required:PROJ/10001`
            "required" => {
                let (project, kind) = param
                    .and_then(|p| p.split_once('/'))
                    .filter(|(p, k)| {
                        !p.is_empty()
                            && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                            && is_digits(k)
                    })
                    .ok_or_else(|| {
                        invalid(t(
                            "The list \"required\" needs a project key and an issue type id, like required:PROJ/10001",
                        ))
                    })?;
                let v = self.get_json(
                    ctx,
                    &format!(
                        "/rest/api/2/issue/createmeta/{project}/issuetypes/{kind}?maxResults=200"
                    ),
                    msg!("read the fields Jira needs"),
                )?;
                Ok(required_fields(&v))
            }
            "recent_issues" => {
                let jql = encode_component("reporter = currentUser() ORDER BY updated DESC");
                // Jira Cloud replaced /search with /search/jql
                let path = if self.cloud() {
                    format!("/rest/api/2/search/jql?jql={jql}&fields=summary&maxResults=25")
                } else {
                    format!("/rest/api/2/search?jql={jql}&fields=summary&maxResults=25")
                };
                let v = self.get_json(ctx, &path, msg!("list recent issues"))?;
                Ok(v.get("issues")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(|i| {
                                let key = strings(i, "key")?;
                                let summary = i
                                    .pointer("/fields/summary")
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                Some(OptionItem::new(key.clone(), format!("{key}  {summary}")))
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            other => Err(invalid(tf(
                "Jira has no list called \"{other}\"",
                &[("other", &other)],
            ))),
        }
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let existing = payload.value("issue").map(str::to_string);
        if let Some(k) = &existing {
            if !issue_key_ok(k) {
                return Err(invalid(tf(
                    "\"{k}\" is not an issue key (like PROJ-123)",
                    &[("k", &k)],
                )));
            }
        }
        let base = self.base()?;
        // 1. the issue: made now, found from an earlier attempt, or the one the person named
        let (key, created) = match (&existing, ctx.progress.get("jira.issue")) {
            (Some(k), _) => (k.clone(), false),
            (None, Some(k)) => (k, true),
            (None, None) => {
                let mut fields = build_fields(payload, self.cloud())?;
                if names_me(payload) {
                    put_me(&mut fields, &self.me(ctx)?);
                }
                let req = self
                    .request(ctx, Method::Post, "/rest/api/2/issue")?
                    .body(Body::json(&json!({ "fields": fields })));
                let resp = self.call(ctx, req, msg!("create the issue"))?;
                let key = resp
                    .json()
                    .and_then(|v| v.get("key").and_then(Value::as_str).map(String::from))
                    .ok_or_else(|| invalid(t("Jira did not say which issue it created")))?;
                ctx.progress.set("jira.issue", &key);
                (key, true)
            }
        };

        let modern = payload.says_how();
        // 2. an update of the issue that was there, as each field says
        if existing.is_some() && modern && !ctx.progress.done("jira.update") {
            self.update(ctx, &key, payload)?;
            ctx.progress.set("jira.update", "1");
        }

        // 3. a comment: the `comment` field, or (a layout of before) the description on an
        // issue that was there
        let comment = if modern {
            payload.value("comment").map(str::to_string)
        } else if existing.is_some() {
            payload
                .value("description")
                .map(str::to_string)
                .or_else(|| payload.prompt.clone().filter(|p| !p.trim().is_empty()))
        } else {
            None
        };
        if comment.is_some() && !ctx.progress.done("jira.comment") {
            // written in Jira's markup, the screenshot and recording in their places
            let text = comment.map(|t| with_media(&render(&t, Style::Jira), payload));
            if let Some(text) = text {
                let req = self
                    .request(
                        ctx,
                        Method::Post,
                        &format!("/rest/api/2/issue/{}/comment", encode_component(&key)),
                    )?
                    .body(Body::json(&json!({ "body": text })));
                self.call(ctx, req, &tf("comment on {key}", &[("key", &key)]))?;
            }
            ctx.progress.set("jira.comment", "1");
        }

        // a vote for an issue that was there (Jira refuses one on an issue you reported: said,
        // not a failure)
        let mut notes = Vec::new();
        if payload.react && existing.is_some() && !ctx.progress.done("jira.vote") {
            let req = self.request(
                ctx,
                Method::Post,
                &format!("/rest/api/2/issue/{}/votes", encode_component(&key)),
            )?;
            if self.call(ctx, req, msg!("vote for the issue")).is_err() {
                notes.push(tf("Jira did not take your vote on {key}", &[("key", &key)]));
            }
            ctx.progress.set("jira.vote", "1");
        }

        // 4. attachments, each at most once
        let uploads = [
            ("jira.att.image", payload.image.as_ref()),
            ("jira.att.still", payload.still.as_ref()),
            ("jira.att.zip", payload.files_zip.as_ref()),
        ];
        for (step, att) in uploads {
            let Some(att) = att else { continue };
            if ctx.progress.done(step) {
                continue;
            }
            if let Err(mut e) = self.attach(ctx, &key, &att.name, &att.mime, &att.data) {
                e.message = if created {
                    tf(
                        "The issue {key} was created, but {problem}",
                        &[("key", &key), ("problem", &lower_first(&e.message))],
                    )
                } else {
                    e.message
                };
                return Err(e);
            }
            ctx.progress.set(step, "1");
        }
        if existing.is_none() && payload.image.is_none() && payload.files_zip.is_none() {
            notes.push(t("There was nothing to attach").to_string());
        }

        Ok(SendOutcome {
            summary: if created {
                tf("Created {key}", &[("key", &key)])
            } else if modern {
                tf("Updated {key}", &[("key", &key)])
            } else {
                tf("Added to {key}", &[("key", &key)])
            },
            url: Some(format!("{base}/browse/{key}")),
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
