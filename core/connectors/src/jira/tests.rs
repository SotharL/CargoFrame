use super::*;
use crate::model::Jira as JiraSettings;
use crate::payload::Attachment;
use crate::testkit::World;
use cargoframe_net::mock::{MockResponse, MockServer};
use cargoframe_safe::FailureKind;
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn cloud(server: &MockServer) -> JiraConnector {
    JiraConnector::new(JiraSettings {
        site_url: server.base(),
        auth: JiraAuth::ApiToken,
        email: "me@acme.com".into(),
    })
}

fn server_connector(server: &MockServer) -> JiraConnector {
    JiraConnector::new(JiraSettings {
        site_url: server.base(),
        auth: JiraAuth::Pat,
        email: String::new(),
    })
}

fn world() -> World {
    World::new().secret("jira", "token", "TOKEN")
}

fn payload() -> Payload {
    Payload {
        title: "Editor - main.rs".into(),
        image: Some(Attachment {
            name: "screenshot.png".into(),
            mime: "image/png".into(),
            data: vec![0x89, b'P', b'N', b'G', 1, 2, 3],
        }),
        still: None,
        files_zip: Some(Attachment {
            name: "files.zip".into(),
            mime: "application/zip".into(),
            data: vec![b'P', b'K', 3, 4],
        }),
        local: None,
        inline_files: Vec::new(),
        prompt: None,
        fields: vec![
            Field::choice("project", "PROJ", true),
            Field::choice("issuetype", "10001", true),
            Field::text("summary", "It crashes"),
        ],
        vars: Default::default(),
    }
}

/// A Jira that creates PROJ-1 and accepts attachments and comments.
fn happy() -> MockServer {
    MockServer::start(|req| match (req.method.as_str(), req.path()) {
        ("POST", "/rest/api/2/issue") => {
            MockResponse::json(201, json!({"id":"10","key":"PROJ-1","self":"x"}))
        }
        ("POST", p) if p.ends_with("/attachments") => MockResponse::json(200, json!([{"id":"1"}])),
        ("POST", p) if p.ends_with("/comment") => MockResponse::json(201, json!({"id":"5"})),
        ("GET", "/rest/api/2/myself") => MockResponse::json(
            200,
            json!({"displayName":"Jane Doe","emailAddress":"me@acme.com"}),
        ),
        _ => MockResponse::new(404),
    })
}

#[test]
fn field_shapes_follow_jiras_api() {
    let f = |key: &str, value: &str, kind: FieldKind, from_list: bool| Field {
        key: key.into(),
        value: value.into(),
        kind,
        from_list,
    };
    let s = |x: &Field, cloud| shape(x, cloud).unwrap();
    assert_eq!(
        s(&f("project", "PROJ", FieldKind::Choice, true), true),
        json!({"key":"PROJ"})
    );
    assert_eq!(
        s(&f("project", "10000", FieldKind::Choice, true), true),
        json!({"id":"10000"})
    );
    assert_eq!(
        s(&f("issuetype", "10001", FieldKind::Choice, true), true),
        json!({"id":"10001"})
    );
    assert_eq!(
        s(&f("issuetype", "Bug", FieldKind::Text, false), true),
        json!({"name":"Bug"})
    );
    assert_eq!(
        s(&f("priority", "2", FieldKind::Choice, true), true),
        json!({"id":"2"})
    );
    assert_eq!(
        s(
            &f("labels", "ui bug, crash,  ", FieldKind::MultiChoice, false),
            true
        ),
        json!(["ui-bug", "crash"])
    );
    assert_eq!(
        s(
            &f("components", "10,Backend", FieldKind::MultiChoice, true),
            true
        ),
        json!([{"id":"10"},{"name":"Backend"}])
    );
    assert_eq!(
        s(&f("assignee", "abc", FieldKind::Choice, true), true),
        json!({"accountId":"abc"})
    );
    assert_eq!(
        s(&f("assignee", "jdoe", FieldKind::Choice, true), false),
        json!({"name":"jdoe"})
    );
    assert_eq!(
        s(&f("parent", "PROJ-9", FieldKind::Text, false), true),
        json!({"key":"PROJ-9"})
    );
    assert_eq!(
        s(&f("summary", " a\n b \n", FieldKind::Text, false), true),
        json!("a b")
    );
    assert_eq!(
        s(
            &f("description", "line1\nline2\n\n", FieldKind::Text, false),
            true
        ),
        json!("line1\nline2")
    );
    // custom fields follow the info type
    assert_eq!(
        s(&f("customfield_1", "x", FieldKind::Text, false), true),
        json!("x")
    );
    assert_eq!(
        s(&f("customfield_2", "3,5", FieldKind::Number, false), true),
        json!(3.5)
    );
    assert_eq!(
        s(&f("customfield_3", "yes", FieldKind::Flag, false), true),
        json!(true)
    );
    assert_eq!(
        s(&f("customfield_3", "no", FieldKind::Flag, false), true),
        json!(false)
    );
    assert_eq!(
        s(&f("customfield_4", "Red", FieldKind::Choice, false), true),
        json!({"value":"Red"})
    );
    assert_eq!(
        s(&f("customfield_4", "42", FieldKind::Choice, true), true),
        json!({"id":"42"})
    );
    assert_eq!(
        s(
            &f("customfield_5", "A, B", FieldKind::MultiChoice, false),
            true
        ),
        json!([{"value":"A"},{"value":"B"}])
    );
    assert!(shape(&f("customfield_2", "abc", FieldKind::Number, false), true).is_err());
}

#[test]
fn a_long_multi_line_summary_is_made_acceptable() {
    let long = format!("first line\n{}", "x".repeat(400));
    let c = clean_summary(&long);
    assert!(!c.contains('\n'));
    assert_eq!(c.chars().count(), 255);
    assert!(c.ends_with('…'));
}

#[test]
fn creating_needs_a_project_and_an_issue_type_and_defaults_the_rest() {
    let mut p = payload();
    p.fields.retain(|f| f.key != "summary");
    p.prompt = Some("Steps:\n1. Click".into());
    let fields = build_fields(&p, true).unwrap();
    assert_eq!(fields["summary"], "Editor - main.rs");
    assert_eq!(fields["description"], "Steps:\n1. Click");

    let mut p = payload();
    p.fields.retain(|f| f.key != "issuetype");
    let err = build_fields(&p, true).unwrap_err();
    assert!(
        err.message.contains("project and an issue type"),
        "{}",
        err.message
    );
    assert!(!err.kind.is_retryable());

    // empty values are left out
    let mut p = payload();
    p.fields.push(Field::text("environment", "  "));
    assert!(!build_fields(&p, true).unwrap().contains_key("environment"));
}

#[test]
fn issue_keys_are_checked() {
    for ok in ["PROJ-1", "AB_C-42", "x1-100"] {
        assert!(issue_key_ok(ok), "{ok}");
    }
    for bad in ["PROJ", "-1", "PROJ-", "PROJ-1a", "../x-1", "A B-1", ""] {
        assert!(!issue_key_ok(bad), "{bad}");
    }
}

#[test]
fn cloud_signs_in_with_basic_and_server_with_bearer() {
    let server = happy();
    let w = world();
    let msg = w.with("jira", |ctx| cloud(&server).test(ctx)).unwrap();
    assert!(msg.contains("Jane Doe"), "{msg}");
    let req = &server.requests()[0];
    assert_eq!(
        req.header("authorization"),
        Some(format!("Basic {}", STANDARD.encode("me@acme.com:TOKEN")).as_str())
    );
    assert_eq!(req.header("accept"), Some("application/json"));

    let server = happy();
    w.with("jira", |ctx| server_connector(&server).test(ctx))
        .unwrap();
    assert_eq!(
        server.requests()[0].header("authorization"),
        Some("Bearer TOKEN")
    );
}

#[test]
fn without_a_token_nothing_is_sent() {
    let server = happy();
    let w = World::new();
    assert_eq!(
        w.with("jira", |ctx| cloud(&server).status(ctx)),
        Status::Disconnected
    );
    let err = w.with("jira", |ctx| cloud(&server).test(ctx)).unwrap_err();
    assert_eq!(err.kind, FailureKind::ConnectorUnavailable);
    assert!(server.requests().is_empty());
    let w = world();
    assert_eq!(
        w.with("jira", |ctx| cloud(&server).status(ctx)),
        Status::Connected
    );
    w.with("jira", |ctx| cloud(&server).disconnect(ctx));
    assert_eq!(
        w.with("jira", |ctx| cloud(&server).status(ctx)),
        Status::Disconnected
    );
}

#[test]
fn a_share_creates_the_issue_then_attaches_the_screenshot_and_the_zip() {
    let server = happy();
    let w = world();
    let out = w
        .with("jira", |ctx| cloud(&server).send(ctx, &payload()))
        .unwrap();
    assert_eq!(out.summary, "Created PROJ-1");
    assert_eq!(
        out.url.as_deref(),
        Some(format!("{}/browse/PROJ-1", server.base()).as_str())
    );

    let create = &server.requests_to("/rest/api/2/issue")[0];
    assert_eq!(
        create.json(),
        json!({"fields":{"project":{"key":"PROJ"},"issuetype":{"id":"10001"},"summary":"It crashes"}})
    );
    let attach = server.requests_to("/rest/api/2/issue/PROJ-1/attachments");
    assert_eq!(attach.len(), 2);
    for a in &attach {
        assert_eq!(a.header("x-atlassian-token"), Some("no-check"));
        assert!(a
            .header("content-type")
            .unwrap()
            .starts_with("multipart/form-data"));
        assert!(a.text().contains("name=\"file\""));
    }
    assert!(attach[0].text().contains("filename=\"screenshot.png\""));
    assert!(attach[0]
        .body
        .windows(7)
        .any(|w| w == [0x89, b'P', b'N', b'G', 1, 2, 3]));
    assert!(attach[1].text().contains("filename=\"files.zip\""));
}

#[test]
fn a_picture_taken_from_a_recording_is_attached_beside_it() {
    let server = happy();
    let w = world();
    let mut p = payload();
    p.still = p.image.take();
    p.image = Some(Attachment {
        name: "recording.mp4".into(),
        mime: "video/mp4".into(),
        data: vec![0, 0, 0, 24],
    });
    w.with("jira", |ctx| cloud(&server).send(ctx, &p)).unwrap();
    let attach = server.requests_to("/rest/api/2/issue/PROJ-1/attachments");
    let names: Vec<bool> = ["recording.mp4", "screenshot.png", "files.zip"]
        .iter()
        .enumerate()
        .map(|(i, n)| attach[i].text().contains(&format!("filename=\"{n}\"")))
        .collect();
    assert_eq!(attach.len(), 3);
    assert_eq!(names, [true, true, true]);
}

#[test]
fn a_retry_after_a_failed_upload_does_not_create_a_second_issue() {
    let attempts = Arc::new(AtomicU32::new(0));
    let a2 = attempts.clone();
    let server = MockServer::start(move |req| match (req.method.as_str(), req.path()) {
        ("POST", "/rest/api/2/issue") => MockResponse::json(201, json!({"key":"PROJ-7"})),
        ("POST", p) if p.ends_with("/attachments") => {
            // the second upload (the zip) fails once
            if req.text().contains("files.zip") && a2.fetch_add(1, Ordering::SeqCst) == 0 {
                MockResponse::new(503)
            } else {
                MockResponse::json(200, json!([]))
            }
        }
        _ => MockResponse::new(404),
    });
    let w = world();
    let err = w
        .with("jira", |ctx| cloud(&server).send(ctx, &payload()))
        .unwrap_err();
    assert_eq!(err.kind, FailureKind::Server(503));
    assert!(
        err.message
            .starts_with("The issue PROJ-7 was created, but could not attach files.zip"),
        "{}",
        err.message
    );

    // same share again with the same progress: continues, creates nothing
    let out = w
        .with("jira", |ctx| cloud(&server).send(ctx, &payload()))
        .unwrap();
    assert_eq!(out.summary, "Created PROJ-7");
    assert_eq!(server.requests_to("/rest/api/2/issue").len(), 1);
    // the screenshot was uploaded once, the zip twice (failed, then done)
    let uploads = server.requests_to("/rest/api/2/issue/PROJ-7/attachments");
    let shots = uploads
        .iter()
        .filter(|r| r.text().contains("screenshot.png"))
        .count();
    let zips = uploads
        .iter()
        .filter(|r| r.text().contains("files.zip"))
        .count();
    assert_eq!((shots, zips), (1, 2));
}

#[test]
fn naming_an_existing_issue_adds_a_comment_and_attachments_only() {
    let server = happy();
    let w = world();
    let mut p = payload();
    p.fields = vec![
        Field::text("issue", "PROJ-99"),
        Field::text("description", "See screenshot"),
    ];
    let out = w.with("jira", |ctx| cloud(&server).send(ctx, &p)).unwrap();
    assert_eq!(out.summary, "Added to PROJ-99");
    assert!(server.requests_to("/rest/api/2/issue").is_empty());
    let comment = &server.requests_to("/rest/api/2/issue/PROJ-99/comment")[0];
    assert_eq!(comment.json(), json!({"body":"See screenshot"}));
    assert_eq!(
        server
            .requests_to("/rest/api/2/issue/PROJ-99/attachments")
            .len(),
        2
    );

    // a bad key is refused before anything is sent
    let server = happy();
    p.fields = vec![Field::text("issue", "../../x")];
    let err = w
        .with("jira", |ctx| cloud(&server).send(ctx, &p))
        .unwrap_err();
    assert!(err.message.contains("not an issue key"));
    assert!(server.requests().is_empty());
}

#[test]
fn service_errors_become_failures_with_the_jira_message() {
    let w = world();
    let server = MockServer::start(|_| {
        MockResponse::json(
            400,
            json!({"errorMessages":[],"errors":{"summary":"You must specify a summary of the issue."}}),
        )
    });
    let err = w
        .with("jira", |ctx| cloud(&server).send(ctx, &payload()))
        .unwrap_err();
    assert_eq!(err.kind, FailureKind::Rejected(400));
    assert!(
        err.message.contains("You must specify a summary"),
        "{}",
        err.message
    );
    assert!(w.progress.get("jira.issue").is_none());

    let server = MockServer::start(|_| MockResponse::new(401));
    let err = w
        .with("jira", |ctx| cloud(&server).send(ctx, &payload()))
        .unwrap_err();
    assert_eq!(err.kind, FailureKind::AuthExpired);

    let server = MockServer::start(|_| MockResponse::new(429).header("Retry-After", "9"));
    let err = w.with("jira", |ctx| cloud(&server).test(ctx)).unwrap_err();
    assert_eq!(err.kind, FailureKind::Throttled(429));
    assert_eq!(err.retry_after, Some(std::time::Duration::from_secs(9)));
}

#[test]
fn the_dropdown_lists_are_read_from_the_right_endpoints() {
    let server = MockServer::start(|req| match req.path() {
        "/rest/api/2/project/search" => MockResponse::json(
            200,
            json!({"values":[{"key":"PROJ","name":"Project"},{"key":"OPS","name":"Operations"}]}),
        ),
        "/rest/api/2/issuetype" => MockResponse::json(
            200,
            json!([{"id":"1","name":"Bug"},{"id":"2","name":"Task"},{"id":"3","name":"Sub-task","subtask":true}]),
        ),
        "/rest/api/2/priority" => MockResponse::json(200, json!([{"id":"1","name":"High"}])),
        "/rest/api/2/label" => MockResponse::json(200, json!({"values":["ui","crash"]})),
        "/rest/api/2/project/PROJ/components" => {
            MockResponse::json(200, json!([{"id":"10","name":"Backend"}]))
        }
        "/rest/api/2/project/PROJ/versions" => MockResponse::json(
            200,
            json!([{"id":"7","name":"1.0"},{"id":"8","name":"0.9","archived":true}]),
        ),
        "/rest/api/2/user/assignable/search" => MockResponse::json(
            200,
            json!([{"accountId":"a1","displayName":"Ann","active":true},{"accountId":"a2","displayName":"Gone","active":false}]),
        ),
        "/rest/api/2/search/jql" => MockResponse::json(
            200,
            json!({"issues":[{"key":"PROJ-5","fields":{"summary":"Crash on save"}}]}),
        ),
        _ => MockResponse::new(404),
    });
    let w = world();
    let list = |name: &str| {
        w.with("jira", |ctx| cloud(&server).options(ctx, name))
            .unwrap()
    };
    let pairs = |v: Vec<OptionItem>| {
        v.into_iter()
            .map(|o| (o.value, o.label))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        pairs(list("projects")),
        vec![
            ("PROJ".into(), "Project (PROJ)".into()),
            ("OPS".into(), "Operations (OPS)".into())
        ]
    );
    assert_eq!(
        pairs(list("issue_types")),
        vec![("1".into(), "Bug".into()), ("2".into(), "Task".into())]
    );
    assert_eq!(pairs(list("priorities")), vec![("1".into(), "High".into())]);
    assert_eq!(
        pairs(list("labels")),
        vec![("ui".into(), "ui".into()), ("crash".into(), "crash".into())]
    );
    assert_eq!(
        pairs(list("components:PROJ")),
        vec![("10".into(), "Backend".into())]
    );
    assert_eq!(
        pairs(list("versions:PROJ")),
        vec![("7".into(), "1.0".into())]
    );
    assert_eq!(
        pairs(list("assignable_users:PROJ")),
        vec![("a1".into(), "Ann".into())]
    );
    assert_eq!(
        pairs(list("recent_issues")),
        vec![("PROJ-5".into(), "PROJ-5  Crash on save".into())]
    );

    // a list that needs a project says so; unknown lists are named
    let err = w
        .with("jira", |ctx| cloud(&server).options(ctx, "components"))
        .unwrap_err();
    assert!(err.message.contains("components:PROJ"));
    let err = w
        .with("jira", |ctx| cloud(&server).options(ctx, "nope"))
        .unwrap_err();
    assert!(err.message.contains("no list called"));
    // the project key cannot break out of the path
    let err = w
        .with("jira", |ctx| {
            cloud(&server).options(ctx, "components:../../x")
        })
        .unwrap_err();
    assert!(err.message.contains("not a project key"), "{}", err.message);
    assert!(server.requests().iter().all(|r| !r.path().contains("..")));
}

#[test]
fn server_uses_the_old_project_list_search_and_user_names() {
    let server = MockServer::start(|req| match req.path() {
        "/rest/api/2/project/search" => MockResponse::new(404),
        "/rest/api/2/project" => MockResponse::json(200, json!([{"key":"OLD","name":"Old"}])),
        "/rest/api/2/user/assignable/search" => {
            MockResponse::json(200, json!([{"name":"jdoe","displayName":"J Doe"}]))
        }
        "/rest/api/2/search" => MockResponse::json(200, json!({"issues":[]})),
        _ => MockResponse::new(404),
    });
    let w = world();
    let c = server_connector(&server);
    let projects = w.with("jira", |ctx| c.options(ctx, "projects")).unwrap();
    assert_eq!(projects[0].value, "OLD");
    let users = w
        .with("jira", |ctx| c.options(ctx, "assignable_users:OLD"))
        .unwrap();
    assert_eq!(users[0].value, "jdoe");
    assert!(w
        .with("jira", |ctx| c.options(ctx, "recent_issues"))
        .unwrap()
        .is_empty());
    assert!(!server.requests_to("/rest/api/2/search").is_empty());
}

#[test]
fn the_description_names_every_field_and_list() {
    let d = JiraConnector::new(JiraSettings::default()).describe();
    assert!(d.fields.iter().any(|f| f.key == "issue"));
    // every field's suggested list exists
    for f in &d.fields {
        if let Some(l) = f.list {
            assert!(d.lists.iter().any(|x| x.id == l), "{} -> {l}", f.key);
        }
    }
}
