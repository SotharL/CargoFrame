use super::*;
use crate::oauth::Tokens;
use crate::payload::{Attachment, Field};
use crate::secrets::SecretStore;
use crate::testkit::{ApproveBrowser, World};
use cargoframe_net::mock::{MockResponse, MockServer};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

fn connector(server: &MockServer) -> RedditConnector {
    RedditConnector::new(model::Reddit {
        client_id: "cid".into(),
        redirect_port: 0,
    })
    .with_endpoints(
        Endpoints {
            auth: "https://www.reddit.example/authorize".into(),
            token: server.url("/token"),
            revoke: server.url("/revoke"),
            api: server.base(),
        },
        Duration::from_secs(5),
    )
}

/// A person who has already signed in.
fn signed_in() -> World {
    let w = World::new();
    w.secrets
        .set(
            "reddit/oauth",
            &serde_json::to_string(&Tokens {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                expires_at: 9_999_999,
            })
            .unwrap(),
        )
        .unwrap();
    w
}

fn image() -> Attachment {
    Attachment {
        name: "screenshot.png".into(),
        mime: "image/png".into(),
        data: vec![0x89, b'P', b'N', b'G', 7, 7],
    }
}

fn base_payload() -> Payload {
    Payload {
        title: "My App - crash".into(),
        fields: vec![Field::text("subreddit", "r/rust")],
        ..Payload::default()
    }
}

fn ok_post() -> MockResponse {
    MockResponse::json(
        200,
        json!({"json":{"errors":[],"data":{"url":"https://www.reddit.com/r/rust/comments/abc/x/","id":"abc"}}}),
    )
}

#[test]
fn subreddit_names_are_cleaned_and_checked() {
    for (raw, want) in [
        ("rust", "rust"),
        ("r/rust", "rust"),
        ("/r/rust/", "rust"),
        (" R/Rust_2 ", "Rust_2"),
        ("u_jane", "u_jane"),
    ] {
        assert_eq!(clean_subreddit(raw).unwrap(), want, "{raw}");
    }
    for bad in [
        "",
        "r",
        "a",
        "has space",
        "../x",
        "r/ru st",
        &"x".repeat(30),
        "ru-st",
    ] {
        assert!(clean_subreddit(bad).is_err(), "{bad}");
    }
}

#[test]
fn rate_limit_waits_are_read_from_the_message() {
    assert_eq!(
        parse_wait("you are doing that too much. try again in 9 minutes."),
        Some(Duration::from_secs(540))
    );
    assert_eq!(
        parse_wait("try again in 1 minute."),
        Some(Duration::from_secs(60))
    );
    assert_eq!(
        parse_wait("Try again in 45 seconds"),
        Some(Duration::from_secs(45))
    );
    assert_eq!(parse_wait("slow down"), None);
}

#[test]
fn a_text_post_has_the_form_reddit_expects() {
    let server = MockServer::start(|req| {
        if req.path() == "/api/submit" {
            ok_post()
        } else {
            MockResponse::new(404)
        }
    });
    let w = signed_in();
    let mut p = base_payload();
    p.prompt = Some("It crashes\non save".into());
    p.fields.push(Field::text("flair_id", "FL1"));
    p.fields.push(Field::text("nsfw", "no"));
    p.fields.push(Field::text("spoiler", "yes"));
    let out = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert_eq!(out.summary, "Posted to r/rust");
    assert_eq!(
        out.url.as_deref(),
        Some("https://www.reddit.com/r/rust/comments/abc/x/")
    );

    let req = &server.requests()[0];
    assert_eq!(req.header("authorization"), Some("bearer AT"));
    let f = |k: &str| req.form_value(k);
    assert_eq!(f("kind").as_deref(), Some("self"));
    assert_eq!(f("sr").as_deref(), Some("rust"));
    assert_eq!(f("title").as_deref(), Some("My App - crash"));
    assert_eq!(f("text").as_deref(), Some("It crashes\non save"));
    assert_eq!(f("api_type").as_deref(), Some("json"));
    assert_eq!(f("flair_id").as_deref(), Some("FL1"));
    assert_eq!(f("spoiler").as_deref(), Some("true"));
    assert_eq!(f("nsfw"), None);
}

#[test]
fn a_link_post_validates_the_link() {
    let server = MockServer::start(|_| ok_post());
    let w = signed_in();
    let mut p = base_payload();
    p.fields.push(Field::text("url", "https://example.com/bug"));
    w.with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    let req = &server.requests()[0];
    assert_eq!(req.form_value("kind").as_deref(), Some("link"));
    assert_eq!(
        req.form_value("url").as_deref(),
        Some("https://example.com/bug")
    );

    p.fields.retain(|f| f.key != "url");
    p.fields.push(Field::text("url", "javascript:alert(1)"));
    let w = signed_in();
    assert!(w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .is_err());
}

fn image_server() -> MockServer {
    MockServer::start(|req| match req.path() {
        "/api/media/asset.json" => {
            // the upload address is on the same mock server
            let host = req.header("host").unwrap_or("").to_string();
            MockResponse::json(
                200,
                json!({"args":{"action":format!("http://{host}/s3"),"fields":[
                    {"name":"key","value":"rte_images/abc123"},{"name":"policy","value":"POL"}]},
                    "asset":{"asset_id":"A1"}}),
            )
        }
        "/s3" => MockResponse::new(201).body(b"<PostResponse/>".to_vec()),
        "/api/submit" => MockResponse::json(
            200,
            json!({"json":{"errors":[],"data":{"user_submitted_page":"https://www.reddit.com/user/jane/submitted/","websocket_url":"wss://x"}}}),
        ),
        _ => MockResponse::new(404),
    })
}

#[test]
fn an_image_post_uploads_the_picture_then_submits_its_address() {
    let server = image_server();
    let w = signed_in();
    let mut p = base_payload();
    p.image = Some(image());
    p.files_zip = Some(Attachment {
        name: "f.zip".into(),
        mime: "application/zip".into(),
        data: vec![1],
    });
    let out = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert!(out.summary.contains("still processing"), "{}", out.summary);
    assert_eq!(
        out.url.as_deref(),
        Some("https://www.reddit.com/user/jane/submitted/")
    );
    assert!(out.notes.iter().any(|n| n.contains("cannot take files")));

    let lease = &server.requests_to("/api/media/asset.json")[0];
    assert_eq!(
        lease.form_value("filepath").as_deref(),
        Some("screenshot.png")
    );
    assert_eq!(lease.form_value("mimetype").as_deref(), Some("image/png"));

    let up = &server.requests_to("/s3")[0];
    // the storage service wants its fields first and the file last; no sign-in header goes there
    assert_eq!(up.header("authorization"), None);
    let text = String::from_utf8_lossy(&up.body).into_owned();
    let key_at = text.find("name=\"key\"").unwrap();
    let policy_at = text.find("name=\"policy\"").unwrap();
    let file_at = text.find("name=\"file\"").unwrap();
    assert!(key_at < policy_at && policy_at < file_at);

    let submit = &server.requests_to("/api/submit")[0];
    assert_eq!(submit.form_value("kind").as_deref(), Some("image"));
    assert_eq!(
        submit.form_value("url"),
        Some(format!("{}/s3/rte_images/abc123", server.base()))
    );
}

#[test]
fn a_rate_limit_is_retryable_and_a_retry_does_not_upload_the_picture_again() {
    let hits = Arc::new(AtomicU32::new(0));
    let h2 = hits.clone();
    let server = MockServer::start(move |req| {
        if req.path() == "/api/submit" && h2.fetch_add(1, Ordering::SeqCst) == 0 {
            return MockResponse::json(
                200,
                json!({"json":{"errors":[["RATELIMIT","you are doing that too much. try again in 2 minutes.","ratelimit"]]}}),
            );
        }
        match req.path() {
            "/api/media/asset.json" => MockResponse::json(
                200,
                json!({"args":{"action":format!("http://{}/s3", req.header("host").unwrap()),"fields":[{"name":"key","value":"k1"}]}}),
            ),
            "/s3" => MockResponse::new(201),
            "/api/submit" => MockResponse::json(
                200,
                json!({"json":{"errors":[],"data":{"url":"https://www.reddit.com/r/rust/comments/z/"}}}),
            ),
            _ => MockResponse::new(404),
        }
    });
    let w = signed_in();
    let mut p = base_payload();
    p.image = Some(image());
    let err = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert_eq!(err.kind, FailureKind::Throttled(429));
    assert_eq!(err.retry_after, Some(Duration::from_secs(120)));

    let out = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap();
    assert!(out.summary.starts_with("Posted to r/rust"));
    assert_eq!(server.requests_to("/api/media/asset.json").len(), 1);
    assert_eq!(server.requests_to("/s3").len(), 1);
    assert_eq!(server.requests_to("/api/submit").len(), 2);
}

#[test]
fn refusals_in_the_body_are_not_retried() {
    let server = MockServer::start(|_| {
        MockResponse::json(
            200,
            json!({"json":{"errors":[["SUBREDDIT_NOEXIST","that subreddit doesn't exist","sr"]]}}),
        )
    });
    let w = signed_in();
    let mut p = base_payload();
    p.prompt = Some("x".into());
    let err = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(!err.kind.is_retryable());
    assert!(err.message.contains("SUBREDDIT_NOEXIST"), "{}", err.message);
    // nothing was posted, so a later attempt is allowed
    assert!(!w.progress.done("reddit.submitted"));
}

#[test]
fn when_it_is_unknown_whether_the_post_went_through_it_is_not_repeated() {
    let w = signed_in().timeout(Duration::from_millis(300));
    let server = MockServer::start(|_| ok_post().delay(Duration::from_secs(2)));
    let mut p = base_payload();
    p.prompt = Some("hello".into());
    let err = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(!err.kind.is_retryable(), "{err:?}");
    assert!(
        err.message.contains("may or may not have posted"),
        "{}",
        err.message
    );

    // a second attempt with the same progress refuses instead of posting again
    let err = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(
        err.message.contains("already have posted"),
        "{}",
        err.message
    );
    std::thread::sleep(Duration::from_millis(2100));
    assert_eq!(server.requests_to("/api/submit").len(), 1);

    // a server error is unknown too
    let w = signed_in();
    let server = MockServer::start(|_| MockResponse::new(503));
    let err = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert!(!err.kind.is_retryable());
    assert!(err.message.contains("may or may not"));
}

#[test]
fn a_throttled_status_is_a_definite_refusal_so_it_may_be_retried() {
    let server = MockServer::start(|_| MockResponse::new(429).header("Retry-After", "3"));
    let w = signed_in();
    let mut p = base_payload();
    p.prompt = Some("hello".into());
    let err = w
        .with("reddit", |ctx| connector(&server).send(ctx, &p))
        .unwrap_err();
    assert_eq!(err.kind, FailureKind::Throttled(429));
    assert!(!w.progress.done("reddit.submitted"));
}

#[test]
fn missing_or_bad_input_is_refused_before_anything_is_sent() {
    let server = MockServer::start(|_| ok_post());
    let w = signed_in();
    let c = connector(&server);
    let mut p = base_payload();
    p.fields.clear();
    assert!(w
        .with("reddit", |ctx| c.send(ctx, &p))
        .unwrap_err()
        .message
        .contains("needs a subreddit"));
    let mut p = base_payload();
    p.fields = vec![Field::text("subreddit", "no good!")];
    assert!(w
        .with("reddit", |ctx| c.send(ctx, &p))
        .unwrap_err()
        .message
        .contains("not a subreddit"));
    let p = base_payload(); // no image, no link, no text
    assert!(w
        .with("reddit", |ctx| c.send(ctx, &p))
        .unwrap_err()
        .message
        .contains("nothing to post"));
    let mut p = base_payload();
    p.prompt = Some("x".into());
    p.fields.push(Field::text("title", &"t".repeat(301)));
    assert!(w
        .with("reddit", |ctx| c.send(ctx, &p))
        .unwrap_err()
        .message
        .contains("300"));
    assert!(server.requests().is_empty());
}

#[test]
fn the_lists_are_read_from_reddit() {
    let server = MockServer::start(|req| match req.path() {
        "/subreddits/mine/subscriber" => MockResponse::json(
            200,
            json!({"data":{"children":[
                {"data":{"display_name":"rust","title":"The Rust language"}},
                {"data":{"display_name":"Android"}}]}}),
        ),
        "/r/rust/api/link_flair_v2" => {
            MockResponse::json(200, json!([{"id":"F1","text":"Help"},{"id":"F2"}]))
        }
        "/r/noflair/api/link_flair_v2" => MockResponse::new(403),
        _ => MockResponse::new(404),
    });
    let w = signed_in();
    let c = connector(&server);
    let subs = w
        .with("reddit", |ctx| c.options(ctx, "subscribed"))
        .unwrap();
    assert_eq!(subs[0].value, "Android");
    assert_eq!(subs[1].label, "r/rust  The Rust language");
    let flairs = w
        .with("reddit", |ctx| c.options(ctx, "flairs:rust"))
        .unwrap();
    assert_eq!(
        flairs
            .iter()
            .map(|f| (f.value.as_str(), f.label.as_str()))
            .collect::<Vec<_>>(),
        vec![("F1", "Help"), ("F2", "(no text)")]
    );
    assert!(w
        .with("reddit", |ctx| c.options(ctx, "flairs:noflair"))
        .unwrap()
        .is_empty());
    assert!(w
        .with("reddit", |ctx| c.options(ctx, "flairs:bad name"))
        .is_err());
    assert!(w.with("reddit", |ctx| c.options(ctx, "nope")).is_err());
}

#[test]
fn connecting_signs_in_through_the_browser_and_names_the_account() {
    let server = MockServer::start(|req| match req.path() {
        "/token" => MockResponse::json(
            200,
            json!({"access_token":"AT9","refresh_token":"RT9","expires_in":86400}),
        ),
        "/api/v1/me" => MockResponse::json(200, json!({"name":"jane"})),
        "/revoke" => MockResponse::ok(),
        _ => MockResponse::new(404),
    });
    let w = World::new();
    let c = connector(&server);
    assert_eq!(w.with("reddit", |ctx| c.status(ctx)), Status::Disconnected);
    let browser = ApproveBrowser::new();
    let msg = w
        .with_browser("reddit", &browser, |ctx| c.connect(ctx))
        .unwrap();
    assert_eq!(msg, "Signed in to Reddit as u/jane");
    assert_eq!(w.with("reddit", |ctx| c.status(ctx)), Status::Connected);

    // Reddit's own conventions: basic client auth with an empty secret, permanent duration
    let token = &server.requests_to("/token")[0];
    assert!(token.header("authorization").unwrap().starts_with("Basic "));
    let url = browser.opened.lock().unwrap()[0].clone();
    assert!(url.contains("duration=permanent"));
    assert!(url.contains("scope=identity%20submit%20mysubscriptions%20read%20flair"));
    assert!(!url.contains("code_challenge"));

    w.with("reddit", |ctx| c.disconnect(ctx));
    assert_eq!(w.with("reddit", |ctx| c.status(ctx)), Status::Disconnected);
    assert_eq!(server.requests_to("/revoke").len(), 1);
}
