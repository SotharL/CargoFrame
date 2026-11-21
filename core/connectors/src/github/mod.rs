//! GitHub (and GitHub Enterprise Server): a share opens an issue in a repository, or updates one
//! (each field as its mapping says, a comment, a 👍), with a fine-grained personal access token.
//! A layout of 0.7.0 or before says none of this: its text becomes a comment, as then.
//!
//! GitHub's API cannot attach a file to an issue. So, unless the connection says not to, the
//! screenshot (or recording) and the zip of the ticked files are first put in a branch of the
//! repository kept for them (`cargoframe-attachments`), and the issue shows the picture and links
//! the files from there. What a share has already done is recorded in [`Progress`], so a retry
//! never puts a file twice or opens a second issue.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::model::{self, repository_ok, GithubFiles};
use crate::payload::{first_line, Attachment, OptionItem, Payload, SendOutcome, Update};
use crate::richtext::{render, Style};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cargoframe_i18n::{msg, t, tf};
use cargoframe_net::{check_url, encode_component, Body, Method, Request, Response};
use cargoframe_safe::{FailureKind, SendError};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

pub struct GithubConnector {
    settings: model::Github,
}

/// GitHub's title limit is 256 characters; one line.
fn clean_title(s: &str) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 256 {
        format!("{}…", one.chars().take(255).collect::<String>())
    } else {
        one
    }
}

fn split_list(v: &str) -> Vec<String> {
    v.split(',')
        .map(|s| s.trim().trim_start_matches('@').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// `12` or `#12`.
fn issue_number(v: &str) -> Option<u64> {
    let n = v.trim().trim_start_matches('#');
    if n.is_empty() || !n.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    n.parse().ok().filter(|n| *n > 0)
}

/// The day of `secs` since 1970, as 2026-11-09 (for the folder a share's files go in).
fn day(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    // civil from days (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A file name GitHub keeps as it is: letters, digits, dot, dash, underscore.
fn safe_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || ".-_".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let s = s.trim_matches(['-', '.']).to_string();
    if s.is_empty() {
        "file".into()
    } else {
        s
    }
}

impl GithubConnector {
    pub fn new(settings: model::Github) -> Self {
        Self { settings }
    }

    fn api(&self) -> Result<String, SendError> {
        check_url(&self.settings.api_url)
            .map_err(|e| invalid(tf("GitHub API address: {e}", &[("e", &e)])))?;
        Ok(self
            .settings
            .api_url
            .trim()
            .trim_end_matches('/')
            .to_string())
    }

    fn token(&self, ctx: &Ctx) -> Result<String, SendError> {
        ctx.secret("token")?
            .filter(|t| !t.trim().is_empty())
            .ok_or_else(|| {
                not_set_up(t(
                    "GitHub has no token yet: open Connections, paste it and press Save",
                ))
            })
    }

    fn request(&self, ctx: &Ctx, method: Method, path: &str) -> Result<Request, SendError> {
        Ok(Request::new(method, format!("{}{path}", self.api()?))
            .header(
                "Authorization",
                format!("Bearer {}", self.token(ctx)?.trim()),
            )
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .timeout(ctx.timeout))
    }

    fn call(&self, ctx: &Ctx, req: Request, what: &str) -> Result<Response, SendError> {
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if resp.is_success() {
            Ok(resp)
        } else {
            let mut e = from_response(&resp, what);
            // GitHub answers 404 for a private repository the token cannot see
            if resp.status == 404 {
                e.message = tf(
                    "{problem}. If the repository exists, the token may not have access to it",
                    &[("problem", &e.message)],
                );
            }
            Err(e)
        }
    }

    fn get_json(&self, ctx: &Ctx, path: &str, what: &str) -> Result<Value, SendError> {
        self.call(ctx, self.request(ctx, Method::Get, path)?, what)?
            .json()
            .ok_or_else(|| {
                invalid(tf(
                    "GitHub answered {what} with something unreadable",
                    &[("what", &t(what))],
                ))
            })
    }

    /// The repository a share goes to: the field `repo`, else the connection's.
    fn repository(&self, payload: Option<&Payload>) -> Result<String, SendError> {
        let r = payload
            .and_then(|p| p.value("repo"))
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .unwrap_or(self.settings.repository.trim())
            .to_string();
        if repository_ok(&r) {
            Ok(r)
        } else {
            Err(invalid(tf(
                "\"{r}\" is not a repository: write it as owner/name",
                &[("r", &r)],
            )))
        }
    }

    /// Make the branch for pictures and files when it is not there yet, from the default branch.
    fn ensure_branch(&self, ctx: &Ctx, repo: &str) -> Result<(), SendError> {
        if ctx.progress.done("github.branch") {
            return Ok(());
        }
        let branch = self.settings.branch.trim();
        let path = format!("/repos/{repo}/git/ref/heads/{}", encode_component(branch));
        let resp = ctx
            .http
            .send(&self.request(ctx, Method::Get, &path)?)
            .map_err(|e| from_net(&e))?;
        if resp.status == 404 {
            let info =
                self.get_json(ctx, &format!("/repos/{repo}"), msg!("read the repository"))?;
            let default = info
                .get("default_branch")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(t("GitHub did not say which branch the repository uses")))?;
            let head = self.get_json(
                ctx,
                &format!("/repos/{repo}/git/ref/heads/{}", encode_component(default)),
                msg!("read the default branch"),
            )?;
            let sha = head
                .pointer("/object/sha")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    invalid(t("The repository has no commit yet to start a branch from"))
                })?;
            let req = self
                .request(ctx, Method::Post, &format!("/repos/{repo}/git/refs"))?
                .body(Body::json(
                    &json!({ "ref": format!("refs/heads/{branch}"), "sha": sha }),
                ));
            self.call(
                ctx,
                req,
                &tf(
                    "make the branch {branch} for pictures",
                    &[("branch", &branch)],
                ),
            )?;
        } else if !resp.is_success() {
            return Err(from_response(
                &resp,
                &tf("find the branch {branch}", &[("branch", &branch)]),
            ));
        }
        ctx.progress.set("github.branch", "1");
        Ok(())
    }

    /// Put one file in the branch; returns the address that shows it.
    fn put_file(&self, ctx: &Ctx, repo: &str, att: &Attachment) -> Result<String, SendError> {
        let hash: String = Sha256::digest(&att.data)
            .iter()
            .take(6)
            .map(|b| format!("{b:02x}"))
            .collect();
        let path = format!(
            "cargoframe/{}/{hash}-{}",
            day((ctx.now)()),
            safe_name(&att.name)
        );
        let req = self
            .request(ctx, Method::Put, &format!("/repos/{repo}/contents/{path}"))?
            .body(Body::json(&json!({
                "message": tf("CargoFrame: {name}", &[("name", &att.name)]),
                "content": STANDARD.encode(&att.data),
                "branch": self.settings.branch.trim(),
            })));
        let resp = match self.call(
            ctx,
            req,
            &tf("put {name} in the repository", &[("name", &att.name)]),
        ) {
            Ok(r) => r,
            // the same file is already there (a retry after the answer was lost): link it
            Err(e) if matches!(e.kind, FailureKind::Rejected(422)) => {
                return Ok(format!(
                    "{}/{repo}/blob/{}/{path}?raw=true",
                    self.web(),
                    self.settings.branch.trim()
                ));
            }
            Err(e) => return Err(e),
        };
        let html = resp
            .json()
            .and_then(|v| {
                v.pointer("/content/html_url")
                    .and_then(Value::as_str)
                    .map(String::from)
            })
            .ok_or_else(|| invalid(t("GitHub did not say where it put the file")))?;
        Ok(format!("{html}?raw=true"))
    }

    /// The lines that show the picture and link the files, each file put at most once: the
    /// screenshot and the recording where the text has a place for them, the rest at the end.
    fn files_part(
        &self,
        ctx: &Ctx,
        repo: &str,
        payload: &Payload,
        text: &str,
    ) -> Result<(String, Vec<String>), SendError> {
        let (shot_place, rec_place) = crate::payload::media_places(text);
        let mut lines = Vec::new();
        let mut shot = String::new();
        let mut rec = String::new();
        let items = [
            ("github.att.image", payload.image.as_ref()),
            ("github.att.still", payload.still.as_ref()),
            ("github.att.zip", payload.files_zip.as_ref()),
        ];
        for (step, att) in items {
            let Some(att) = att else { continue };
            let url = match ctx.progress.get(step) {
                Some(u) => u,
                None => {
                    self.ensure_branch(ctx, repo)?;
                    let u = self.put_file(ctx, repo, att)?;
                    ctx.progress.set(step, &u);
                    u
                }
            };
            let shown = if att.mime.starts_with("image/") {
                format!("![{}]({url})", att.name)
            } else {
                format!("[{}]({url})", att.name)
            };
            let is_shot = payload.screenshot().is_some_and(|s| std::ptr::eq(s, att));
            let is_rec = payload.recording().is_some_and(|r| std::ptr::eq(r, att));
            if is_shot && shot_place {
                shot = shown;
            } else if is_rec && rec_place {
                rec = shown;
            } else {
                lines.push(shown);
            }
        }
        Ok((crate::payload::place_media(text, &shot, &rec), lines))
    }
}

impl Connector for GithubConnector {
    fn describe(&self) -> Describe {
        let f = |key, label, hint, list| FieldSpec {
            key,
            label,
            hint,
            list,
        };
        let l = |id: &str, label: &str| ListSpec {
            id: id.into(),
            label: label.into(),
            param: None,
            inside: None,
            search: false,
        };
        Describe {
            fields: vec![
                f("repo", msg!("Repository"), msg!("owner/name. Defaults to the connection's"), Some("repositories")),
                f("title", msg!("Title"), msg!("One line. Defaults to the window title"), None),
                f("body", msg!("Description"), msg!("Defaults to the prompt text"), None),
                f("labels", msg!("Labels"), msg!("Separate several with commas"), Some("labels")),
                f("assignees", msg!("Assignees"), msg!("GitHub user names. Separate several with commas"), Some("assignees")),
                f("milestone", msg!("Milestone"), "", Some("milestones")),
                f("type", msg!("Issue type"), msg!("For organisations that use issue types: Bug, Task…"), None),
                f(
                    "issue",
                    msg!("Update an existing issue"),
                    msg!("An issue number: updates that issue (each field as its mapping says) instead of opening one. Empty: a new issue"),
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
                l("repositories", msg!("My repositories")),
                l("labels", msg!("Labels of the repository")),
                l("assignees", msg!("People who can be assigned")),
                l("milestones", msg!("Open milestones")),
                l("recent_issues", msg!("Open issues, recently updated")),
            ],
            summary: match self.settings.files {
                GithubFiles::Branch => tf(
                    "Opens an issue in {repo}. The screenshot and the ticked files go in the branch {branch} and are shown in the issue.",
                    &[
                        ("repo", &self.settings.repository.trim()),
                        ("branch", &self.settings.branch.trim()),
                    ],
                ),
                GithubFiles::None => tf(
                    "Opens an issue in {repo}, with the text only: pictures and files are not sent.",
                    &[("repo", &self.settings.repository.trim())],
                ),
            },
        }
    }

    fn status(&self, ctx: &Ctx) -> Status {
        match ctx.secret("token") {
            Ok(Some(t)) if !t.trim().is_empty() => Status::Connected,
            _ => Status::Disconnected,
        }
    }

    fn disconnect(&self, ctx: &Ctx) {
        let _ = ctx
            .secrets
            .delete(&crate::secrets::key(ctx.connection_id, "token"));
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let me = self.get_json(ctx, "/user", msg!("check the token"))?;
        let login = me
            .get("login")
            .and_then(Value::as_str)
            .unwrap_or(t("your account"))
            .to_string();
        let repo = self.repository(None)?;
        let info = self.get_json(
            ctx,
            &format!("/repos/{repo}"),
            &tf("open {repo}", &[("repo", &repo)]),
        )?;
        if info.pointer("/has_issues").and_then(Value::as_bool) == Some(false) {
            return Err(invalid(tf(
                "Issues are turned off in {repo}: turn them on in its settings",
                &[("repo", &repo)],
            )));
        }
        Ok(tf(
            "Signed in as @{login}, opening issues in {repo}",
            &[("login", &login), ("repo", &repo)],
        ))
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        // `labels:owner/name` reads another repository's list
        let (name, repo) = match list.split_once(':') {
            Some((n, r)) if repository_ok(r) => (n, r.trim().to_string()),
            Some((_, r)) => {
                return Err(invalid(tf(
                    "\"{r}\" is not a repository: write it as owner/name",
                    &[("r", &r)],
                )))
            }
            None => (list, self.settings.repository.trim().to_string()),
        };
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(String::from);
        let arr = |v: Value| v.as_array().cloned().unwrap_or_default();
        Ok(match name {
            "repositories" => arr(self.get_json(
                ctx,
                "/user/repos?per_page=100&sort=updated",
                msg!("list repositories"),
            )?)
            .iter()
            .filter(|r| r.get("has_issues").and_then(Value::as_bool) != Some(false))
            .filter_map(|r| s(r, "full_name").map(|n| OptionItem::new(n.clone(), n)))
            .collect(),
            "labels" => arr(self.get_json(
                ctx,
                &format!("/repos/{repo}/labels?per_page=100"),
                msg!("list labels"),
            )?)
            .iter()
            .filter_map(|l| s(l, "name").map(|n| OptionItem::new(n.clone(), n)))
            .collect(),
            "assignees" => arr(self.get_json(
                ctx,
                &format!("/repos/{repo}/assignees?per_page=100"),
                msg!("list people"),
            )?)
            .iter()
            .filter_map(|u| s(u, "login").map(|n| OptionItem::new(n.clone(), format!("@{n}"))))
            .collect(),
            "milestones" => arr(self.get_json(
                ctx,
                &format!("/repos/{repo}/milestones?state=open&per_page=100"),
                msg!("list milestones"),
            )?)
            .iter()
            .filter_map(|m| {
                let n = m.get("number").and_then(Value::as_u64)?;
                Some(OptionItem::new(
                    n.to_string(),
                    s(m, "title").unwrap_or_else(|| n.to_string()),
                ))
            })
            .collect(),
            "recent_issues" => arr(self.get_json(
                ctx,
                &format!("/repos/{repo}/issues?state=open&sort=updated&per_page=30"),
                msg!("list issues"),
            )?)
            .iter()
            // the issues list has the pull requests in it too
            .filter(|i| i.get("pull_request").is_none())
            .filter_map(|i| {
                let n = i.get("number").and_then(Value::as_u64)?;
                Some(OptionItem::new(
                    n.to_string(),
                    format!("#{n}  {}", s(i, "title").unwrap_or_default()),
                ))
            })
            .collect(),
            other => {
                return Err(invalid(tf(
                    "GitHub has no list called \"{other}\"",
                    &[("other", &other)],
                )))
            }
        })
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let repo = self.repository(Some(payload))?;
        let existing = match payload
            .value("issue")
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            Some(v) => Some(issue_number(v).ok_or_else(|| {
                invalid(tf("\"{v}\" is not an issue number, like 12", &[("v", &v)]))
            })?),
            None => None,
        };
        if let Some(n) = existing.filter(|_| payload.says_how()) {
            return self.update(ctx, &repo, n, payload);
        }
        let labels = payload.value("labels").map(split_list).unwrap_or_default();

        // 1. the text, written the way GitHub shows it
        let text = payload
            .value("body")
            .map(str::to_string)
            .or_else(|| payload.prompt.clone())
            .filter(|t| !t.trim().is_empty())
            .map(|t| render(&t, Style::Github))
            .unwrap_or_default();

        // 2. the pictures and files, in their branch: shown where the text says, or at the end
        let mut notes = Vec::new();
        let (text, files) = match self.settings.files {
            GithubFiles::Branch => self.files_part(ctx, &repo, payload, &text)?,
            GithubFiles::None => {
                if payload.image.is_some() || payload.files_zip.is_some() {
                    notes.push(
                        t("The picture and files were not sent: the connection sends text only")
                            .to_string(),
                    );
                }
                (crate::payload::place_media(&text, "", ""), Vec::new())
            }
        };
        let body = match (text.trim().is_empty(), files.is_empty()) {
            (_, true) => text,
            (true, false) => files.join("\n\n"),
            (false, false) => format!("{text}\n\n{}", files.join("\n\n")),
        };
        let web = |n: &str| format!("/repos/{repo}/issues/{n}");

        // 3a. a comment on an issue there already
        if let Some(n) = existing {
            if !ctx.progress.done("github.comment") {
                if !body.trim().is_empty() {
                    let req = self
                        .request(
                            ctx,
                            Method::Post,
                            &format!("{}/comments", web(&n.to_string())),
                        )?
                        .body(Body::json(&json!({ "body": body })));
                    self.call(ctx, req, &tf("comment on #{n}", &[("n", &n)]))?;
                }
                ctx.progress.set("github.comment", "1");
            }
            if !labels.is_empty() && !ctx.progress.done("github.labels") {
                let req = self
                    .request(
                        ctx,
                        Method::Post,
                        &format!("{}/labels", web(&n.to_string())),
                    )?
                    .body(Body::json(&json!({ "labels": labels })));
                self.call(ctx, req, &tf("label #{n}", &[("n", &n)]))?;
                ctx.progress.set("github.labels", "1");
            }
            return Ok(SendOutcome {
                summary: tf("Added to #{n} in {repo}", &[("n", &n), ("repo", &repo)]),
                url: Some(self.html(&repo, n)),
                notes,
                place: None,
            });
        }

        // 3b. a new issue, made once
        let (number, url) = match ctx.progress.get("github.issue") {
            Some(n) => {
                let n: u64 = n.parse().unwrap_or(0);
                (n, self.html(&repo, n))
            }
            None => {
                let title = payload
                    .value("title")
                    .map(str::to_string)
                    .filter(|t| !t.trim().is_empty())
                    .unwrap_or_else(|| {
                        if payload.title.trim().is_empty() {
                            t("CargoFrame capture").into()
                        } else {
                            payload.title.clone()
                        }
                    });
                let mut issue = Map::new();
                issue.insert("title".into(), Value::String(clean_title(&title)));
                if !body.trim().is_empty() {
                    issue.insert("body".into(), Value::String(body));
                }
                if !labels.is_empty() {
                    issue.insert("labels".into(), json!(labels));
                }
                let people = payload
                    .value("assignees")
                    .map(split_list)
                    .unwrap_or_default();
                if !people.is_empty() {
                    issue.insert("assignees".into(), json!(people));
                }
                if let Some(m) = payload
                    .value("milestone")
                    .map(str::trim)
                    .filter(|m| !m.is_empty())
                {
                    let n = issue_number(m).ok_or_else(|| {
                        invalid(tf(
                            "\"{m}\" is not a milestone number: pick it from the list",
                            &[("m", &m)],
                        ))
                    })?;
                    issue.insert("milestone".into(), json!(n));
                }
                if let Some(t) = payload
                    .value("type")
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                {
                    issue.insert("type".into(), Value::String(t.to_string()));
                }
                let req = self
                    .request(ctx, Method::Post, &format!("/repos/{repo}/issues"))?
                    .body(Body::json(&Value::Object(issue)));
                let made = self
                    .call(ctx, req, &tf("open an issue in {repo}", &[("repo", &repo)]))?
                    .json()
                    .ok_or_else(|| invalid(t("GitHub did not say which issue it opened")))?;
                let n = made
                    .get("number")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| invalid(t("GitHub did not say which issue it opened")))?;
                ctx.progress.set("github.issue", &n.to_string());
                let url = made
                    .get("html_url")
                    .and_then(Value::as_str)
                    .map(String::from)
                    .unwrap_or_else(|| self.html(&repo, n));
                (n, url)
            }
        };
        if let Some(c) = payload.value("comment") {
            if !ctx.progress.done("github.comment") {
                let req = self
                    .request(
                        ctx,
                        Method::Post,
                        &format!("{}/comments", web(&number.to_string())),
                    )?
                    .body(Body::json(&json!({ "body": render(c, Style::Github) })));
                self.call(ctx, req, &tf("comment on #{n}", &[("n", &number)]))?;
                ctx.progress.set("github.comment", "1");
            }
        }
        Ok(SendOutcome {
            summary: tf(
                "Opened #{number} in {repo}",
                &[("number", &number), ("repo", &repo)],
            ),
            url: Some(url),
            notes,
            place: None,
        })
    }
}

impl GithubConnector {
    /// Update issue `n` as the mapped fields say. The pictures and files go with the description
    /// when it is written, otherwise with the comment.
    fn update(
        &self,
        ctx: &Ctx,
        repo: &str,
        n: u64,
        payload: &Payload,
    ) -> Result<SendOutcome, SendError> {
        let path = format!("/repos/{repo}/issues/{n}");
        let body_how = payload.update_of("body");
        let line = |s: &str, how| {
            if how == Update::AppendLine {
                first_line(s).to_string()
            } else {
                s.to_string()
            }
        };
        let host = match body_how {
            Some(how) => render(
                &line(payload.value("body").unwrap_or(""), how),
                Style::Github,
            ),
            None => payload
                .value("comment")
                .map(|c| render(c, Style::Github))
                .unwrap_or_default(),
        };
        let mut notes = Vec::new();
        let (placed, files) = match self.settings.files {
            GithubFiles::Branch => self.files_part(ctx, repo, payload, &host)?,
            GithubFiles::None => {
                if payload.image.is_some() || payload.files_zip.is_some() {
                    notes.push(
                        t("The picture and files were not sent: the connection sends text only")
                            .to_string(),
                    );
                }
                (crate::payload::place_media(&host, "", ""), Vec::new())
            }
        };
        let placed = [placed.trim().to_string(), files.join("\n\n")]
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        let (added, comment) = match body_how {
            Some(_) => (
                placed,
                payload
                    .value("comment")
                    .map(|c| render(c, Style::Github))
                    .unwrap_or_default(),
            ),
            None => (String::new(), placed),
        };

        if !ctx.progress.done("github.update") {
            let appends =
                |h: Option<Update>| matches!(h, Some(Update::Append | Update::AppendLine));
            let now = if appends(payload.update_of("title")) || appends(body_how) {
                self.get_json(ctx, &path, &tf("read #{n}", &[("n", &n)]))?
            } else {
                Value::Null
            };
            let before = |k: &str| {
                now.get(k)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim_end()
                    .to_string()
            };
            let mut patch = Map::new();
            if let Some(how) = payload.update_of("title") {
                let v = line(payload.value("title").unwrap_or(""), how);
                let title = match how {
                    Update::Append | Update::AppendLine => format!("{} {v}", before("title")),
                    _ => v,
                };
                patch.insert("title".into(), Value::String(clean_title(&title)));
            }
            if let Some(how) = body_how {
                let was = before("body");
                let body = match how {
                    Update::Append | Update::AppendLine if !was.is_empty() => {
                        format!("{was}\n\n{added}")
                    }
                    _ => added,
                };
                patch.insert("body".into(), Value::String(body));
            }
            // labels and people are added one call each (they keep what is there), or set
            for (key, what) in [
                ("labels", msg!("label #{n}")),
                ("assignees", msg!("assign #{n}")),
            ] {
                let Some(how) = payload.update_of(key) else {
                    continue;
                };
                let items = split_list(payload.value(key).unwrap_or(""));
                if how == Update::Add {
                    let req = self
                        .request(ctx, Method::Post, &format!("{path}/{key}"))?
                        .body(Body::json(&json!({ key: items })));
                    self.call(ctx, req, &tf(what, &[("n", &n)]))?;
                } else {
                    patch.insert(key.into(), json!(items));
                }
            }
            if payload.update_of("milestone").is_some() {
                let m = payload.value("milestone").unwrap_or("");
                let number = issue_number(m).ok_or_else(|| {
                    invalid(tf(
                        "\"{m}\" is not a milestone number: pick it from the list",
                        &[("m", &m)],
                    ))
                })?;
                patch.insert("milestone".into(), json!(number));
            }
            if !patch.is_empty() {
                let req = self
                    .request(ctx, Method::Patch, &path)?
                    .body(Body::json(&Value::Object(patch)));
                self.call(ctx, req, &tf("update #{n}", &[("n", &n)]))?;
            }
            ctx.progress.set("github.update", "1");
        }
        if !comment.trim().is_empty() && !ctx.progress.done("github.comment") {
            let req = self
                .request(ctx, Method::Post, &format!("{path}/comments"))?
                .body(Body::json(&json!({ "body": comment })));
            self.call(ctx, req, &tf("comment on #{n}", &[("n", &n)]))?;
            ctx.progress.set("github.comment", "1");
        }
        if payload.react && !ctx.progress.done("github.react") {
            let req = self
                .request(ctx, Method::Post, &format!("{path}/reactions"))?
                .body(Body::json(&json!({ "content": "+1" })));
            if self.call(ctx, req, msg!("react")).is_err() {
                notes.push(tf("GitHub did not take your 👍 on #{n}", &[("n", &n)]));
            }
            ctx.progress.set("github.react", "1");
        }
        Ok(SendOutcome {
            summary: tf("Updated #{n} in {repo}", &[("n", &n), ("repo", &repo)]),
            url: Some(self.html(repo, n)),
            notes,
            place: None,
        })
    }

    /// The web site that goes with the API: github.com, or a company's own server.
    fn web(&self) -> String {
        self.settings
            .api_url
            .trim()
            .trim_end_matches('/')
            .replace("://api.github.com", "://github.com")
            .replace("/api/v3", "")
    }

    /// The issue's web page.
    fn html(&self, repo: &str, n: u64) -> String {
        format!("{}/{repo}/issues/{n}", self.web())
    }
}

#[cfg(test)]
mod tests;
