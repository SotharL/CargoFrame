//! Jira: the reference destination. Works with Jira Cloud (e-mail + API token) and Jira Server or
//! Data Center (personal access token) through REST API v2, which both understand.
//!
//! A share creates an issue (or, when the `issue` field has a key, adds a comment to that issue)
//! and attaches the screenshot and, if files were ticked, their zip. What it has already done is
//! recorded in [`Progress`], so a retry after a failed upload never creates a second issue.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::model::{self, JiraAuth};
use crate::payload::{Field, FieldKind, OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
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
        check_url(&self.settings.site_url).map_err(|e| invalid(format!("Jira address: {e}")))?;
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
                not_set_up("Jira has no API token yet: open Connections, paste it and press Save")
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
        resp.json()
            .ok_or_else(|| invalid(format!("Jira answered {what} with something unreadable")))
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
        self.call(ctx, req, &format!("attach {name} to {key}"))
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
        "description" | "environment" | "duedate" => Value::String(f.value.trim_end().to_string()),
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
                let n: f64 = v
                    .replace(',', ".")
                    .parse()
                    .map_err(|_| format!("\"{v}\" is not a number (field {})", f.key))?;
                serde_json::Number::from_f64(n)
                    .map(Value::Number)
                    .ok_or_else(|| format!("\"{v}\" is not a number (field {})", f.key))?
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
        if f.key == "issue" || f.value.trim().is_empty() {
            continue;
        }
        let v = shape(f, cloud).map_err(invalid)?;
        out.insert(f.key.clone(), v);
    }
    if !out.contains_key("project") || !out.contains_key("issuetype") {
        return Err(invalid(
            "Jira needs a project and an issue type: map them to the fields \"project\" and \"issuetype\" on the sharing button",
        ));
    }
    if !out.contains_key("summary") {
        let title = if payload.title.trim().is_empty() {
            "CargoFrame capture"
        } else {
            payload.title.trim()
        };
        out.insert("summary".into(), Value::String(clean_summary(title)));
    }
    if !out.contains_key("description") {
        if let Some(p) = payload.prompt.as_deref().filter(|p| !p.trim().is_empty()) {
            out.insert(
                "description".into(),
                Value::String(p.trim_end().to_string()),
            );
        }
    }
    Ok(out)
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
        };
        Describe {
            fields: vec![
                f("project", "Project", "A project key such as PROJ", Some("projects")),
                f("issuetype", "Issue type", "Bug, Task, Story...", Some("issue_types")),
                f("summary", "Summary", "One line. Defaults to the window title", None),
                f("description", "Description", "Defaults to the prompt text", None),
                f("priority", "Priority", "", Some("priorities")),
                f("labels", "Labels", "Separate several with commas", Some("labels")),
                f("assignee", "Assignee", "", Some("assignable_users")),
                f("components", "Components", "Separate several with commas", Some("components")),
                f("fixVersions", "Fix versions", "Separate several with commas", Some("versions")),
                f("versions", "Affects versions", "Separate several with commas", Some("versions")),
                f("duedate", "Due date", "YYYY-MM-DD", None),
                f("environment", "Environment", "", None),
                f("parent", "Parent issue", "An issue key, for sub-tasks", Some("recent_issues")),
                f(
                    "issue",
                    "Add to an existing issue",
                    "An issue key: adds a comment and the attachments instead of creating an issue",
                    Some("recent_issues"),
                ),
            ],
            lists: vec![
                l("projects", "Projects", None),
                l("issue_types", "Issue types", None),
                l("priorities", "Priorities", None),
                l("labels", "Labels", None),
                l("recent_issues", "Issues I reported recently", None),
                l("assignable_users", "People who can be assigned", Some("project key")),
                l("components", "Components of a project", Some("project key")),
                l("versions", "Versions of a project", Some("project key")),
            ],
            summary: "Creates an issue and attaches the screenshot (and a zip of the ticked files). Custom fields use their key, such as customfield_10042.".into(),
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
        let me = self.get_json(ctx, "/rest/api/2/myself", "check the sign-in")?;
        let who = ["displayName", "emailAddress", "name"]
            .iter()
            .find_map(|k| me.get(k).and_then(Value::as_str))
            .unwrap_or("your account");
        let host = check_url(&self.settings.site_url).unwrap_or_default();
        Ok(format!("Signed in to {host} as {who}"))
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        let (name, param) = match list.split_once(':') {
            Some((n, p)) => (n, Some(p.trim())),
            None => (list, None),
        };
        let need_project = || {
            let key = param.filter(|p| !p.is_empty()).ok_or_else(|| {
                invalid(format!(
                    "The list \"{name}\" needs a project key, like {name}:PROJ"
                ))
            })?;
            if key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                Ok(key.to_string())
            } else {
                Err(invalid(format!(
                    "\"{key}\" is not a project key (letters, digits and _ only)"
                )))
            }
        };
        let strings = |v: &Value, key: &str| v.get(key).and_then(Value::as_str).map(String::from);
        match name {
            "projects" => {
                let v = match self.get_json(
                    ctx,
                    "/rest/api/2/project/search?maxResults=100&orderBy=name",
                    "list projects",
                ) {
                    Ok(v) => v.get("values").cloned().unwrap_or(v),
                    // Server and Data Center have no search endpoint
                    Err(e) if matches!(e.kind, FailureKind::Rejected(404)) => {
                        self.get_json(ctx, "/rest/api/2/project", "list projects")?
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
                let v = self.get_json(ctx, path, &format!("list {}", name.replace('_', " ")))?;
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
                let v = self.get_json(ctx, "/rest/api/2/label?maxResults=200", "list labels")?;
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
                    "list components",
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
                    "list versions",
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
            "assignable_users" => {
                let v = self.get_json(
                    ctx,
                    &format!(
                        "/rest/api/2/user/assignable/search?project={}&maxResults=100",
                        need_project()?
                    ),
                    "list people",
                )?;
                Ok(v.as_array()
                    .map(|a| {
                        a.iter()
                            .filter(|u| u.get("active").and_then(Value::as_bool) != Some(false))
                            .filter_map(|u| {
                                let id =
                                    strings(u, if self.cloud() { "accountId" } else { "name" })?;
                                let label = strings(u, "displayName").unwrap_or_else(|| id.clone());
                                Some(OptionItem::new(id, label))
                            })
                            .collect()
                    })
                    .unwrap_or_default())
            }
            "recent_issues" => {
                let jql = encode_component("reporter = currentUser() ORDER BY updated DESC");
                // Jira Cloud replaced /search with /search/jql
                let path = if self.cloud() {
                    format!("/rest/api/2/search/jql?jql={jql}&fields=summary&maxResults=25")
                } else {
                    format!("/rest/api/2/search?jql={jql}&fields=summary&maxResults=25")
                };
                let v = self.get_json(ctx, &path, "list recent issues")?;
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
            other => Err(invalid(format!("Jira has no list called \"{other}\""))),
        }
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let existing = payload.value("issue").map(str::to_string);
        if let Some(k) = &existing {
            if !issue_key_ok(k) {
                return Err(invalid(format!(
                    "\"{k}\" is not an issue key (like PROJ-123)"
                )));
            }
        }
        let base = self.base()?;

        // 1. the issue: made now, found from an earlier attempt, or the one the person named
        let (key, created) = match (&existing, ctx.progress.get("jira.issue")) {
            (Some(k), _) => (k.clone(), false),
            (None, Some(k)) => (k, true),
            (None, None) => {
                let fields = build_fields(payload, self.cloud())?;
                let req = self
                    .request(ctx, Method::Post, "/rest/api/2/issue")?
                    .body(Body::json(&json!({ "fields": fields })));
                let resp = self.call(ctx, req, "create the issue")?;
                let key = resp
                    .json()
                    .and_then(|v| v.get("key").and_then(Value::as_str).map(String::from))
                    .ok_or_else(|| invalid("Jira did not say which issue it created"))?;
                ctx.progress.set("jira.issue", &key);
                (key, true)
            }
        };

        // 2. a comment, when adding to an existing issue
        if existing.is_some() && !ctx.progress.done("jira.comment") {
            let text = payload
                .value("description")
                .map(str::to_string)
                .or_else(|| payload.prompt.clone().filter(|p| !p.trim().is_empty()));
            if let Some(text) = text {
                let req = self
                    .request(
                        ctx,
                        Method::Post,
                        &format!("/rest/api/2/issue/{}/comment", encode_component(&key)),
                    )?
                    .body(Body::json(&json!({ "body": text })));
                self.call(ctx, req, &format!("comment on {key}"))?;
            }
            ctx.progress.set("jira.comment", "1");
        }

        // 3. attachments, each at most once
        let mut notes = Vec::new();
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
                    format!(
                        "The issue {key} was created, but {}",
                        lower_first(&e.message)
                    )
                } else {
                    e.message
                };
                return Err(e);
            }
            ctx.progress.set(step, "1");
        }
        if existing.is_none() && payload.image.is_none() && payload.files_zip.is_none() {
            notes.push("There was nothing to attach".to_string());
        }

        Ok(SendOutcome {
            summary: if created {
                format!("Created {key}")
            } else {
                format!("Added to {key}")
            },
            url: Some(format!("{base}/browse/{key}")),
            notes,
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
