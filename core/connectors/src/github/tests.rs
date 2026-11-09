use super::*;
use crate::model::{Github as GithubSettings, GithubFiles};
use crate::payload::{Attachment, Field};
use crate::testkit::World;
use cargoframe_net::mock::{MockResponse, MockServer};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn connector(server: &MockServer, files: GithubFiles) -> GithubConnector {
    GithubConnector::new(GithubSettings {
        repository: "octo/app".into(),
        api_url: server.base(),
        files,
        branch: "cargoframe-attachments".into(),
    })
}

fn world() -> World {
    World::new().secret("gh", "token", "TOKEN")
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
            Field::text("title", "It crashes\non save"),
            Field::text("body", "Seen in **Maya**\n- twice"),
            Field::text("labels", "bug, crash"),
        ],
        vars: Default::default(),
    }
}

/// A GitHub whose attachments branch is missing until it is made.
fn happy(branch_made: Arc<AtomicU32>) -> MockServer {
    MockServer::start(move |req| match (req.method.as_str(), req.path()) {
        ("GET", "/repos/octo/app/git/ref/heads/cargoframe-attachments") => {
            if branch_made.load(Ordering::SeqCst) > 0 {
                MockResponse::json(200, json!({"object": {"sha": "b1"}}))
            } else {
                MockResponse::json(404, json!({"message": "Not Found"}))
            }
        }
        ("GET", "/repos/octo/app") => {
            MockResponse::json(200, json!({"default_branch": "main", "has_issues": true}))
        }
        ("GET", "/repos/octo/app/git/ref/heads/main") => {
            MockResponse::json(200, json!({"object": {"sha": "abc"}}))
        }
        ("POST", "/repos/octo/app/git/refs") => {
            branch_made.fetch_add(1, Ordering::SeqCst);
            MockResponse::json(201, json!({"ref": "refs/heads/cargoframe-attachments"}))
        }
        ("PUT", p) if p.starts_with("/repos/octo/app/contents/cargoframe/") => {
            let name = p.rsplit('/').next().unwrap_or("").to_string();
            MockResponse::json(
                201,
                json!({"content": {"html_url": format!("https://github.com/octo/app/blob/cargoframe-attachments/{name}")}}),
            )
        }
        ("POST", "/repos/octo/app/issues") => MockResponse::json(
            201,
            json!({"number": 7, "html_url": "https://github.com/octo/app/issues/7"}),
        ),
        ("POST", "/repos/octo/app/issues/12/comments") => MockResponse::json(201, json!({"id": 1})),
        ("POST", "/repos/octo/app/issues/12/labels") => MockResponse::json(200, json!([])),
        ("GET", "/user") => MockResponse::json(200, json!({"login": "jane"})),
        _ => MockResponse::new(404),
    })
}

#[test]
fn a_share_puts_the_files_in_their_branch_then_opens_an_issue_that_shows_them() {
    let server = happy(Arc::new(AtomicU32::new(0)));
    let c = connector(&server, GithubFiles::Branch);
    let out = world().with("gh", |ctx| c.send(ctx, &payload())).unwrap();
    assert_eq!(out.summary, "Opened #7 in octo/app");
    assert_eq!(
        out.url.as_deref(),
        Some("https://github.com/octo/app/issues/7")
    );

    // the branch is made from the default branch
    let refs = server.requests_to("/repos/octo/app/git/refs");
    assert_eq!(
        refs[0].json(),
        json!({"ref": "refs/heads/cargoframe-attachments", "sha": "abc"})
    );
    // each file goes once, in the branch, under the day's folder
    let puts: Vec<_> = server
        .requests()
        .into_iter()
        .filter(|r| r.method == "PUT")
        .collect();
    assert_eq!(puts.len(), 2);
    assert_eq!(puts[0].json()["branch"], "cargoframe-attachments");
    assert!(
        puts[0].path().ends_with("-screenshot.png"),
        "{}",
        puts[0].path()
    );
    assert_eq!(puts[0].header("Authorization"), Some("Bearer TOKEN"));

    let issue = server.requests_to("/repos/octo/app/issues")[0].json();
    assert_eq!(issue["title"], "It crashes on save");
    assert_eq!(issue["labels"], json!(["bug", "crash"]));
    let body = issue["body"].as_str().unwrap();
    assert!(
        body.starts_with("Seen in **Maya**\n\n- twice\n\n"),
        "{body}"
    );
    assert!(
        body.contains("![screenshot.png](https://github.com/octo/app/blob/cargoframe-attachments/"),
        "{body}"
    );
    assert!(body.contains("-screenshot.png?raw=true)"), "{body}");
    assert!(
        body.contains("[files.zip](https://github.com/octo/app/blob/"),
        "{body}"
    );
}

#[test]
fn a_retry_after_the_issue_was_opened_does_not_open_another_or_put_files_twice() {
    let made = Arc::new(AtomicU32::new(0));
    let server = happy(made);
    let c = connector(&server, GithubFiles::Branch);
    let w = world();
    w.with("gh", |ctx| c.send(ctx, &payload())).unwrap();
    let again = w.with("gh", |ctx| c.send(ctx, &payload())).unwrap();
    assert_eq!(again.summary, "Opened #7 in octo/app");
    assert_eq!(server.requests_to("/repos/octo/app/issues").len(), 1);
    assert_eq!(
        server
            .requests()
            .into_iter()
            .filter(|r| r.method == "PUT")
            .count(),
        2
    );
}

#[test]
fn an_issue_number_adds_a_comment_and_the_labels_instead() {
    let server = happy(Arc::new(AtomicU32::new(1)));
    let c = connector(&server, GithubFiles::None);
    let mut p = payload();
    p.fields.push(Field::text("issue", "#12"));
    let out = world().with("gh", |ctx| c.send(ctx, &p)).unwrap();
    assert_eq!(out.summary, "Added to #12 in octo/app");
    assert_eq!(
        out.url.as_deref(),
        Some(&*format!("{}/octo/app/issues/12", server.base()))
    );
    // text only: nothing is put in the repository, and the person is told
    assert!(server.requests().iter().all(|r| r.method != "PUT"));
    assert!(out.notes[0].contains("text only"), "{:?}", out.notes);
    let comment = server.requests_to("/repos/octo/app/issues/12/comments")[0].json();
    assert_eq!(comment["body"], "Seen in **Maya**\n\n- twice");
    assert_eq!(
        server.requests_to("/repos/octo/app/issues/12/labels")[0].json(),
        json!({"labels": ["bug", "crash"]})
    );
    assert!(server.requests_to("/repos/octo/app/issues").is_empty());
}

#[test]
fn mistakes_are_said_before_anything_is_sent() {
    let server = happy(Arc::new(AtomicU32::new(1)));
    let c = connector(&server, GithubFiles::None);
    let mut p = payload();
    p.fields.push(Field::text("repo", "not a repo"));
    let e = world().with("gh", |ctx| c.send(ctx, &p)).unwrap_err();
    assert!(e.message.contains("owner/name"), "{}", e.message);
    let mut p = payload();
    p.fields.push(Field::text("issue", "PROJ-1"));
    let e = world().with("gh", |ctx| c.send(ctx, &p)).unwrap_err();
    assert!(e.message.contains("issue number"), "{}", e.message);
    let e = World::new()
        .with("gh", |ctx| c.send(ctx, &payload()))
        .unwrap_err();
    assert!(e.message.contains("no token"), "{}", e.message);
    assert!(server.requests().is_empty());
}

#[test]
fn the_test_names_the_account_and_the_lists_come_from_the_repository() {
    let server = MockServer::start(|req| match req.path() {
        "/user" => MockResponse::json(200, json!({"login": "jane"})),
        "/repos/octo/app" => MockResponse::json(200, json!({"has_issues": true})),
        "/repos/octo/app/labels" => {
            MockResponse::json(200, json!([{"name": "bug"}, {"name": "ui"}]))
        }
        "/repos/other/lib/labels" => MockResponse::json(200, json!([{"name": "docs"}])),
        "/repos/octo/app/milestones" => {
            MockResponse::json(200, json!([{"number": 3, "title": "v1"}]))
        }
        "/repos/octo/app/issues" => MockResponse::json(
            200,
            json!([{"number": 5, "title": "Crash"}, {"number": 6, "title": "A PR", "pull_request": {}}]),
        ),
        "/repos/octo/app/assignees" => MockResponse::json(200, json!([{"login": "bob"}])),
        _ => MockResponse::new(404),
    });
    let c = connector(&server, GithubFiles::Branch);
    let w = world();
    assert_eq!(
        w.with("gh", |ctx| c.test(ctx)).unwrap(),
        "Signed in as @jane, opening issues in octo/app"
    );
    let names = |list: &str| -> Vec<(String, String)> {
        w.with("gh", |ctx| c.options(ctx, list))
            .unwrap()
            .into_iter()
            .map(|o| (o.value, o.label))
            .collect()
    };
    assert_eq!(
        names("labels"),
        [("bug".into(), "bug".into()), ("ui".into(), "ui".into())]
    );
    assert_eq!(
        names("labels:other/lib"),
        [("docs".to_string(), "docs".to_string())]
    );
    assert_eq!(names("milestones"), [("3".to_string(), "v1".to_string())]);
    assert_eq!(
        names("recent_issues"),
        [("5".to_string(), "#5  Crash".to_string())]
    );
    assert_eq!(
        names("assignees"),
        [("bob".to_string(), "@bob".to_string())]
    );
}

#[test]
fn a_private_repository_it_cannot_see_says_the_token_may_lack_access() {
    let server = MockServer::start(|_| MockResponse::json(404, json!({"message": "Not Found"})));
    let c = connector(&server, GithubFiles::None);
    let e = world()
        .with("gh", |ctx| c.send(ctx, &payload()))
        .unwrap_err();
    assert!(e.message.contains("may not have access"), "{}", e.message);
}

#[test]
fn days_and_file_names_for_the_branch() {
    assert_eq!(day(0), "1970-01-01");
    assert_eq!(day(1_794_182_400), "2026-11-09");
    assert_eq!(safe_name("Screen shot (1).png"), "Screen-shot--1-.png");
    assert_eq!(safe_name("../x"), "x");
    assert_eq!(issue_number("#12"), Some(12));
    assert_eq!(issue_number("0"), None);
    assert_eq!(clean_title(" a\n b "), "a b");
}

#[test]
fn the_screenshot_shows_where_the_text_puts_it_and_is_not_added_again_at_the_end() {
    let server = happy(Arc::new(AtomicU32::new(1)));
    let c = connector(&server, GithubFiles::Branch);
    let mut p = payload();
    p.fields.retain(|f| f.key != "body");
    p.fields.push(Field::text(
        "body",
        "| Shot |\n| --- |\n| {{capture.screenshot}} |",
    ));
    world().with("gh", |ctx| c.send(ctx, &p)).unwrap();
    let body = server.requests_to("/repos/octo/app/issues")[0].json()["body"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(body.starts_with("| Shot |\n| --- |\n| ![screenshot.png](https://github.com/octo/app/blob/cargoframe-attachments/"), "{body}");
    // the picture once, in its cell; the zip at the end
    assert_eq!(body.matches("screenshot.png](").count(), 1, "{body}");
    assert!(body.trim_end().ends_with("files.zip?raw=true)"), "{body}");
}
