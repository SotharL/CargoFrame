//! A destination the person describes themselves: a base address, how to sign in, one request
//! that delivers the share and, optionally, lists for dropdowns. Covers any service with a plain
//! HTTP API (a webhook, a ticket system, Twitter/X or BlueSky until they get their own).

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use crate::model::{Custom, CustomAuth, CustomBody, Pair, SendMethod};
use crate::payload::{OptionItem, Payload, SendOutcome};
use crate::{Connector, Describe, FieldSpec, ListSpec, Status};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use cargoframe_context::template;
use cargoframe_net::{check_url, encode_component, Body, Method, Multipart, Request};
use cargoframe_safe::SendError;
use serde_json::Value;

pub struct CustomConnector {
    settings: Custom,
}

impl CustomConnector {
    pub fn new(settings: Custom) -> Self {
        Self { settings }
    }

    fn base(&self) -> Result<String, SendError> {
        check_url(&self.settings.base_url).map_err(|e| invalid(format!("Base address: {e}")))?;
        Ok(self
            .settings
            .base_url
            .trim()
            .trim_end_matches('/')
            .to_string())
    }

    fn secret(&self, ctx: &Ctx) -> Result<Option<String>, SendError> {
        if self.settings.auth == CustomAuth::None {
            return Ok(None);
        }
        ctx.secret("secret")?
            .filter(|s| !s.is_empty())
            .map(Some)
            .ok_or_else(|| {
                not_set_up(
                    "This connection has no key yet: open Connections, paste it and press Save",
                )
            })
    }

    /// Add the sign-in to a request.
    fn sign(&self, mut req: Request, secret: Option<&str>) -> Request {
        let Some(secret) = secret else { return req };
        match &self.settings.auth {
            CustomAuth::None => {}
            CustomAuth::Bearer => req = req.header("Authorization", format!("Bearer {secret}")),
            CustomAuth::Header { name } => req = req.header(name, secret),
            CustomAuth::Basic { user } => {
                req = req.header(
                    "Authorization",
                    format!("Basic {}", STANDARD.encode(format!("{user}:{secret}"))),
                )
            }
            CustomAuth::Query { param } => {
                let sep = if req.url.contains('?') { '&' } else { '?' };
                req.url = format!(
                    "{}{sep}{}={}",
                    req.url,
                    encode_component(param),
                    encode_component(secret)
                );
            }
        }
        req
    }
}

/// Dotted path into JSON: `data.items.0.id` (also `data.items[0].id`). Empty means the value.
pub fn json_path<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let normalised = path.replace('[', ".").replace(']', "");
    let mut cur = v;
    for seg in normalised.split('.').filter(|s| !s.is_empty()) {
        cur = match cur {
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            Value::Object(o) => o.get(seg)?,
            _ => return None,
        };
    }
    Some(cur)
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Escape a value to sit inside a JSON string.
fn json_escape(s: &str) -> String {
    let quoted = serde_json::to_string(s).unwrap_or_default();
    quoted
        .strip_prefix('"')
        .and_then(|q| q.strip_suffix('"'))
        .unwrap_or("")
        .to_string()
}

/// How a template's values are written into the text around them.
#[derive(Clone, Copy)]
enum Escape {
    Raw,
    Json,
    Url,
}

fn fill(text: &str, payload: &Payload, into: Escape) -> Result<String, SendError> {
    let lookup = |name: &str| -> Option<String> {
        let v = match name {
            "prompt" => Some(payload.prompt.clone().unwrap_or_default()),
            "title" => Some(payload.title.clone()),
            _ => payload
                .fields
                .iter()
                .find(|f| f.key == name)
                .map(|f| f.value.clone())
                .or_else(|| payload.vars.get(name).cloned()),
        }?;
        Some(match into {
            Escape::Raw => v,
            Escape::Json => json_escape(&v),
            Escape::Url => encode_component(&v),
        })
    };
    let r = template::render(text, lookup);
    if r.unresolved.is_empty() {
        Ok(r.text)
    } else {
        Err(invalid(format!(
            "The request uses {} but nothing fills {}",
            r.unresolved
                .iter()
                .map(|n| format!("{{{{{n}}}}}"))
                .collect::<Vec<_>>()
                .join(", "),
            if r.unresolved.len() == 1 {
                "it"
            } else {
                "them"
            }
        )))
    }
}

fn fill_pairs(pairs: &[Pair], payload: &Payload) -> Result<Vec<(String, String)>, SendError> {
    pairs
        .iter()
        .map(|p| Ok((p.name.clone(), fill(&p.value, payload, Escape::Raw)?)))
        .collect()
}

impl Connector for CustomConnector {
    fn describe(&self) -> Describe {
        let lists = self
            .settings
            .lists
            .iter()
            .map(|l| ListSpec {
                id: l.id.clone(),
                label: if l.label.trim().is_empty() {
                    l.id.clone()
                } else {
                    l.label.clone()
                },
                param: None,
            })
            .collect();
        Describe {
            fields: vec![
                FieldSpec { key: "title", label: "Title", hint: "Placeholder {{title}}: the window title", list: None },
            ],
            lists,
            summary: "Sends one request that you describe. Use {{field}} placeholders for what the sharing button maps, plus {{prompt}} and {{title}}.".into(),
        }
    }

    fn status(&self, ctx: &Ctx) -> Status {
        if self.settings.auth == CustomAuth::None {
            return Status::Connected;
        }
        match ctx.secret("secret") {
            Ok(Some(s)) if !s.is_empty() => Status::Connected,
            _ => Status::Disconnected,
        }
    }

    fn disconnect(&self, ctx: &Ctx) {
        let _ = ctx
            .secrets
            .delete(&crate::secrets::key(ctx.connection_id, "secret"));
    }

    fn test(&self, ctx: &Ctx) -> Result<String, SendError> {
        let path = self.settings.test_path.trim();
        if path.is_empty() {
            return Ok("Nothing to test: this connection has no test address".into());
        }
        let secret = self.secret(ctx)?;
        let req = self
            .sign(
                Request::get(format!("{}{}", self.base()?, with_slash(path))).timeout(ctx.timeout),
                secret.as_deref(),
            )
            .no_redirects();
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if resp.is_success() {
            Ok(format!("The server answered {}", resp.status))
        } else {
            Err(from_response(&resp, "reach the test address"))
        }
    }

    fn options(&self, ctx: &Ctx, list: &str) -> Result<Vec<OptionItem>, SendError> {
        let spec = self
            .settings
            .lists
            .iter()
            .find(|l| l.id == list)
            .ok_or_else(|| invalid(format!("This connection has no list called \"{list}\"")))?;
        let secret = self.secret(ctx)?;
        let req = self
            .sign(
                Request::get(format!("{}{}", self.base()?, with_slash(&spec.path)))
                    .header("Accept", "application/json")
                    .timeout(ctx.timeout),
                secret.as_deref(),
            )
            .no_redirects();
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(from_response(&resp, &format!("load the list \"{list}\"")));
        }
        let json = resp
            .json()
            .ok_or_else(|| invalid(format!("The list \"{list}\" did not answer with JSON")))?;
        let items = json_path(&json, &spec.items_path)
            .and_then(Value::as_array)
            .ok_or_else(|| {
                invalid(format!(
                    "No list of items was found at \"{}\" in the answer",
                    spec.items_path
                ))
            })?;
        Ok(items
            .iter()
            .filter_map(|item| {
                let value = scalar(json_path(item, &spec.value_path)?)?;
                let label = if spec.label_path.trim().is_empty() {
                    value.clone()
                } else {
                    json_path(item, &spec.label_path)
                        .and_then(scalar)
                        .unwrap_or_else(|| value.clone())
                };
                Some(OptionItem::new(value, label))
            })
            .collect())
    }

    fn send(&self, ctx: &Ctx, payload: &Payload) -> Result<SendOutcome, SendError> {
        let secret = self.secret(ctx)?;
        let s = &self.settings.send;
        let path = fill(&s.path, payload, Escape::Url)?;
        let method = match s.method {
            SendMethod::Post => Method::Post,
            SendMethod::Put => Method::Put,
            SendMethod::Patch => Method::Patch,
        };
        let mut req = Request::new(method, format!("{}{}", self.base()?, with_slash(&path)))
            .header("Accept", "application/json")
            .timeout(ctx.timeout);
        for (name, value) in fill_pairs(&s.headers, payload)? {
            req = req.header(&name, value.trim());
        }
        let mut notes = Vec::new();
        req = req.body(match &s.body {
            CustomBody::None => Body::None,
            CustomBody::Json { template } => {
                let text = fill(template, payload, Escape::Json)?;
                if serde_json::from_str::<Value>(&text).is_err() {
                    return Err(invalid(
                        "The JSON body is not valid once the placeholders are filled in",
                    ));
                }
                Body::text("application/json", text)
            }
            CustomBody::Form { fields } => {
                let owned = fill_pairs(fields, payload)?;
                let refs: Vec<(&str, &str)> = owned
                    .iter()
                    .map(|(k, v)| (k.as_str(), v.as_str()))
                    .collect();
                Body::form(&refs)
            }
            CustomBody::Multipart {
                fields,
                image_field,
                files_field,
            } => {
                let mut m = Multipart::form_data();
                for (k, v) in fill_pairs(fields, payload)? {
                    m = m.field(&k, &v);
                }
                if !image_field.trim().is_empty() {
                    match &payload.image {
                        Some(a) => m = m.file(image_field, &a.name, &a.mime, a.data.clone()),
                        None => notes.push("There was no screenshot to send".to_string()),
                    }
                }
                if !files_field.trim().is_empty() {
                    if let Some(a) = &payload.files_zip {
                        m = m.file(files_field, &a.name, &a.mime, a.data.clone());
                    }
                }
                Body::multipart(m)
            }
        });
        let req = self.sign(req, secret.as_deref()).no_redirects();
        let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
        if !resp.is_success() {
            return Err(from_response(&resp, "send the share"));
        }
        let url = resp
            .json()
            .and_then(|j| json_path(&j, &self.settings.result_url_path).and_then(scalar))
            .filter(|_| !self.settings.result_url_path.trim().is_empty())
            .filter(|u| check_url(u).is_ok());
        Ok(SendOutcome {
            summary: format!("Sent (the server answered {})", resp.status),
            url,
            notes,
        })
    }
}

fn with_slash(path: &str) -> String {
    let p = path.trim();
    if p.is_empty() || p.starts_with('/') || p.starts_with('?') {
        p.to_string()
    } else {
        format!("/{p}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CustomList, CustomRequest};
    use crate::payload::{Attachment, Field};
    use crate::testkit::World;
    use cargoframe_net::mock::{MockResponse, MockServer};
    use cargoframe_safe::FailureKind;
    use serde_json::json;

    fn connector(server: &MockServer, f: impl FnOnce(&mut Custom)) -> CustomConnector {
        let mut c = Custom {
            base_url: server.base(),
            ..Custom::default()
        };
        f(&mut c);
        CustomConnector::new(c)
    }

    fn payload() -> Payload {
        Payload {
            title: "My \"App\"".into(),
            image: Some(Attachment {
                name: "shot.png".into(),
                mime: "image/png".into(),
                data: vec![1, 2, 3],
            }),
            still: None,
            files_zip: Some(Attachment {
                name: "files.zip".into(),
                mime: "application/zip".into(),
                data: vec![9, 9],
            }),
            local: None,
            inline_files: Vec::new(),
            prompt: Some("line1\nline2 \"q\"".into()),
            fields: vec![Field::text("channel", "dev ops"), Field::text("empty", "")],
            vars: [("rule.version".to_string(), "1.2.3".to_string())].into(),
        }
    }

    #[test]
    fn json_paths_walk_objects_and_arrays() {
        let v = json!({"data":{"items":[{"id":7,"name":"a"}]},"ok":true});
        assert_eq!(json_path(&v, "data.items.0.id"), Some(&json!(7)));
        assert_eq!(json_path(&v, "data.items[0].name"), Some(&json!("a")));
        assert_eq!(json_path(&v, ""), Some(&v));
        assert_eq!(json_path(&v, "data.nope"), None);
        assert_eq!(json_path(&v, "data.items.x"), None);
        assert_eq!(json_path(&v, "ok.deeper"), None);
        assert_eq!(scalar(&json!(7)).as_deref(), Some("7"));
        assert_eq!(scalar(&json!(null)), None);
    }

    #[test]
    fn json_bodies_are_escaped_and_must_stay_valid_json() {
        let server = MockServer::start(|_| {
            MockResponse::json(201, json!({"data":{"url":"https://x.test/p/1"}}))
        });
        let w = World::new();
        let c = connector(&server, |c| {
            c.send = CustomRequest {
                method: SendMethod::Post,
                path: "/post/{{channel}}".into(),
                headers: vec![Pair {
                    name: "X-App".into(),
                    value: "{{rule.version}}".into(),
                }],
                body: CustomBody::Json {
                    template: r#"{"text":"{{prompt}}","app":"{{title}}","n":"{{empty}}"}"#.into(),
                },
            };
            c.result_url_path = "data.url".into();
        });
        let out = w.with("c", |ctx| c.send(ctx, &payload())).unwrap();
        assert_eq!(out.url.as_deref(), Some("https://x.test/p/1"));
        let req = &server.requests()[0];
        assert_eq!(req.path(), "/post/dev%20ops");
        assert_eq!(req.header("x-app"), Some("1.2.3"));
        assert_eq!(req.header("content-type"), Some("application/json"));
        assert_eq!(
            req.json(),
            json!({"text":"line1\nline2 \"q\"","app":"My \"App\"","n":""})
        );
    }

    #[test]
    fn a_placeholder_nothing_fills_stops_the_send() {
        let server = MockServer::start(|_| MockResponse::ok());
        let w = World::new();
        let c = connector(&server, |c| {
            c.send.body = CustomBody::Json {
                template: r#"{"a":"{{missing}}"}"#.into(),
            };
        });
        let err = w.with("c", |ctx| c.send(ctx, &payload())).unwrap_err();
        assert!(err.message.contains("{{missing}}"), "{}", err.message);
        assert!(!err.kind.is_retryable());
        assert!(server.requests().is_empty());

        let c = connector(&server, |c| {
            c.send.body = CustomBody::Json {
                template: "{not json {{channel}}".into(),
            }
        });
        let err = w.with("c", |ctx| c.send(ctx, &payload())).unwrap_err();
        assert!(err.message.contains("not valid"), "{}", err.message);
    }

    #[test]
    fn form_and_multipart_bodies() {
        let server = MockServer::start(|_| MockResponse::ok());
        let w = World::new();
        let c = connector(&server, |c| {
            c.send.method = SendMethod::Put;
            c.send.path = "up".into();
            c.send.body = CustomBody::Form {
                fields: vec![Pair {
                    name: "msg".into(),
                    value: "{{prompt}} & {{channel}}".into(),
                }],
            };
        });
        w.with("c", |ctx| c.send(ctx, &payload())).unwrap();
        let r = &server.requests()[0];
        assert_eq!((r.method.as_str(), r.path()), ("PUT", "/up"));
        assert_eq!(
            r.form_value("msg").as_deref(),
            Some("line1\nline2 \"q\" & dev ops")
        );

        let server = MockServer::start(|_| MockResponse::ok());
        let c = connector(&server, |c| {
            c.send.path = "/upload".into();
            c.send.body = CustomBody::Multipart {
                fields: vec![Pair {
                    name: "caption".into(),
                    value: "{{title}}".into(),
                }],
                image_field: "image".into(),
                files_field: "archive".into(),
            };
        });
        w.with("c", |ctx| c.send(ctx, &payload())).unwrap();
        let r = &server.requests()[0];
        let text = r.text();
        assert!(r
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data"));
        assert!(text.contains("name=\"caption\"\r\n\r\nMy \"App\""));
        assert!(text.contains("name=\"image\"; filename=\"shot.png\""));
        assert!(text.contains("name=\"archive\"; filename=\"files.zip\""));
    }

    #[test]
    fn each_auth_style_carries_the_key_where_it_belongs() {
        let w = World::new().secret("c", "secret", "S3CRET");
        let try_auth = |auth: CustomAuth| {
            let server = MockServer::start(|_| MockResponse::ok());
            let c = connector(&server, |c| {
                c.auth = auth;
                c.test_path = "/ping?x=1".into();
            });
            w.with("c", |ctx| c.test(ctx)).unwrap();
            let r = server.requests().remove(0);
            (
                r.header("authorization").map(String::from),
                r.header("x-key").map(String::from),
                r.target.clone(),
            )
        };
        assert_eq!(
            try_auth(CustomAuth::Bearer).0.as_deref(),
            Some("Bearer S3CRET")
        );
        assert_eq!(
            try_auth(CustomAuth::Header {
                name: "X-Key".into()
            })
            .1
            .as_deref(),
            Some("S3CRET")
        );
        assert_eq!(
            try_auth(CustomAuth::Basic { user: "bob".into() }).0,
            Some(format!("Basic {}", STANDARD.encode("bob:S3CRET")))
        );
        assert_eq!(
            try_auth(CustomAuth::Query {
                param: "api key".into()
            })
            .2,
            "/ping?x=1&api%20key=S3CRET"
        );
        assert_eq!(
            try_auth(CustomAuth::None),
            (None, None, "/ping?x=1".to_string())
        );
    }

    #[test]
    fn a_missing_key_is_reported_and_none_needs_no_key() {
        let server = MockServer::start(|_| MockResponse::ok());
        let w = World::new();
        let c = connector(&server, |c| c.auth = CustomAuth::Bearer);
        assert_eq!(w.with("c", |ctx| c.status(ctx)), Status::Disconnected);
        let err = w.with("c", |ctx| c.send(ctx, &payload())).unwrap_err();
        assert_eq!(err.kind, FailureKind::ConnectorUnavailable);
        assert!(server.requests().is_empty());
        let c = connector(&server, |_| {});
        assert_eq!(w.with("c", |ctx| c.status(ctx)), Status::Connected);
    }

    #[test]
    fn lists_are_read_from_the_answer() {
        let server = MockServer::start(|req| match req.path() {
            "/channels" => MockResponse::json(
                200,
                json!({"data":[{"id":1,"name":"dev"},{"id":2},{"name":"no id"}]}),
            ),
            "/plain" => MockResponse::json(200, json!(["a", "b"])),
            _ => MockResponse::new(500),
        });
        let w = World::new();
        let c = connector(&server, |c| {
            c.lists = vec![
                CustomList {
                    id: "channels".into(),
                    path: "channels".into(),
                    items_path: "data".into(),
                    value_path: "id".into(),
                    label_path: "name".into(),
                    ..Default::default()
                },
                CustomList {
                    id: "plain".into(),
                    path: "/plain".into(),
                    items_path: "".into(),
                    value_path: "".into(),
                    label_path: "".into(),
                    ..Default::default()
                },
                CustomList {
                    id: "bad".into(),
                    path: "/channels".into(),
                    items_path: "nope".into(),
                    value_path: "id".into(),
                    ..Default::default()
                },
            ];
        });
        let got: Vec<(String, String)> = w
            .with("c", |ctx| c.options(ctx, "channels"))
            .unwrap()
            .into_iter()
            .map(|o| (o.value, o.label))
            .collect();
        assert_eq!(
            got,
            vec![("1".into(), "dev".into()), ("2".into(), "2".into())]
        );
        let plain = w.with("c", |ctx| c.options(ctx, "plain")).unwrap();
        assert_eq!(
            plain.iter().map(|o| o.value.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert!(w
            .with("c", |ctx| c.options(ctx, "bad"))
            .unwrap_err()
            .message
            .contains("nope"));
        assert!(w
            .with("c", |ctx| c.options(ctx, "missing"))
            .unwrap_err()
            .message
            .contains("no list called"));
        assert_eq!(c.describe().lists.len(), 3);
    }

    #[test]
    fn errors_keep_their_class_and_unsafe_result_links_are_dropped() {
        let w = World::new();
        let server =
            MockServer::start(|_| MockResponse::json(503, json!({"message":"maintenance"})));
        let c = connector(&server, |_| {});
        let err = w.with("c", |ctx| c.send(ctx, &payload())).unwrap_err();
        assert_eq!(err.kind, FailureKind::Server(503));
        assert!(err.message.contains("maintenance"));

        let server =
            MockServer::start(|_| MockResponse::json(200, json!({"url":"javascript:alert(1)"})));
        let c = connector(&server, |c| c.result_url_path = "url".into());
        assert_eq!(
            w.with("c", |ctx| c.send(ctx, &payload())).unwrap().url,
            None
        );
    }

    #[test]
    fn the_key_is_never_forwarded_by_a_redirect() {
        let other = MockServer::start(|_| MockResponse::ok());
        let target = other.url("/x");
        let first = MockServer::start(move |_| MockResponse::new(307).header("Location", &target));
        let w = World::new().secret("c", "secret", "S3CRET");
        let c = connector(&first, |c| {
            c.auth = CustomAuth::Header {
                name: "X-Key".into(),
            };
        });
        let err = w.with("c", |ctx| c.send(ctx, &payload())).unwrap_err();
        assert!(
            err.message.contains("redirected"),
            "{err:?}; the server saw {} request(s)",
            first.requests().len()
        );
        assert!(other.requests().is_empty());
    }

    #[test]
    fn the_test_button_is_honest_about_having_nothing_to_test() {
        let server = MockServer::start(|_| MockResponse::new(404));
        let w = World::new();
        let c = connector(&server, |_| {});
        assert!(w
            .with("c", |ctx| c.test(ctx))
            .unwrap()
            .contains("Nothing to test"));
        let c = connector(&server, |c| c.test_path = "/health".into());
        assert!(w.with("c", |ctx| c.test(ctx)).is_err());
    }
}
